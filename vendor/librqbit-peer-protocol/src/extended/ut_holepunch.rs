//! Rivulet: BEP 55 ut_holepunch. Binary payload:
//! msg_type u8 (0 rendezvous, 1 connect, 2 error), addr_type u8 (0 v4, 1 v6),
//! addr (4 or 16 bytes), port u16 BE, err_code u32 BE.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HolepunchType {
    Rendezvous,
    Connect,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UtHolepunch {
    pub kind: HolepunchType,
    pub addr: SocketAddr,
    /// 1 no such peer, 2 not connected, 3 no support, 4 no self
    pub err_code: u32,
}

impl UtHolepunch {
    pub fn serialize(&self, out: &mut impl std::io::Write) -> std::io::Result<()> {
        let t = match self.kind {
            HolepunchType::Rendezvous => 0u8,
            HolepunchType::Connect => 1,
            HolepunchType::Error => 2,
        };
        out.write_all(&[t])?;
        match self.addr.ip() {
            IpAddr::V4(ip) => {
                out.write_all(&[0])?;
                out.write_all(&ip.octets())?;
            }
            IpAddr::V6(ip) => {
                out.write_all(&[1])?;
                out.write_all(&ip.octets())?;
            }
        }
        out.write_all(&self.addr.port().to_be_bytes())?;
        out.write_all(&self.err_code.to_be_bytes())
    }

    pub fn deserialize(b: &[u8]) -> Option<Self> {
        let kind = match *b.first()? {
            0 => HolepunchType::Rendezvous,
            1 => HolepunchType::Connect,
            2 => HolepunchType::Error,
            _ => return None,
        };
        let (ip, rest): (IpAddr, &[u8]) = match *b.get(1)? {
            0 => {
                let o: [u8; 4] = b.get(2..6)?.try_into().ok()?;
                (Ipv4Addr::from(o).into(), b.get(6..)?)
            }
            1 => {
                let o: [u8; 16] = b.get(2..18)?.try_into().ok()?;
                (Ipv6Addr::from(o).into(), b.get(18..)?)
            }
            _ => return None,
        };
        let port = u16::from_be_bytes(rest.get(0..2)?.try_into().ok()?);
        let err_code = u32::from_be_bytes(rest.get(2..6)?.try_into().ok()?);
        Some(Self { kind, addr: SocketAddr::new(ip, port), err_code })
    }
}
