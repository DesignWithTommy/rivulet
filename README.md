<p align="center"><img src="assets/rivulet.png" width="96"></p>

# Rivulet

**Fast, lightweight torrent client for Windows, written in Rust.** Free and open-source BitTorrent downloader for magnet links and `.torrent` files: one portable exe, no installer, no ads, no bundled junk. Unaffiliated with any other client.

**Download:** [rivulet.exe](https://github.com/DesignWithTommy/rivulet/releases/latest)

## Use
- Paste a magnet link or URL and press **Add**, open a `.torrent` file, or drag one onto the window.
- Each torrent saves into its own folder under the save directory (default `Downloads`).
- Closing the window stops it; the next start resumes without re-checking files.

## What's inside
Built on a patched [librqbit](https://github.com/ikatson/rqbit) (vendored in `vendor/`), with:
- PropShare-style choker and rarest-first piece selection
- BEP 55 hole punching with DCUtR-style RTT-timed dials (off by default; toggle with the **Hole punching** button)
- Parallel file checking, IPv4 + IPv6, TCP + uTP, DHT, PEX, UPnP
- Filename vetting: Windows-unsafe names and symlinks in a torrent are refused

## Build
```
cargo build --release
```

## License
Apache-2.0. Vendored librqbit crates are Apache-2.0 by Igor Katson.
