#![windows_subsystem = "windows"]
use anyhow::{Result, bail};
use eframe::egui;
use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse, ManagedTorrent, Session};
use std::{path::PathBuf, sync::{Arc, Mutex}};

type Handle = Arc<ManagedTorrent>;

struct App {
    rt: tokio::runtime::Runtime,
    session: Arc<Session>,
    input: String,
    out_dir: String,
    torrents: Arc<Mutex<Vec<Handle>>>,
    status: Arc<Mutex<String>>,
}

fn default_dir() -> String {
    std::env::var("USERPROFILE").map(|h| format!("{h}\\Downloads")).unwrap_or_else(|_| ".".into())
}

/// Windows-unsafe names librqbit does not reject: drive/stream colons, reserved devices,
/// trailing dot/space (aliasing), control chars.
fn check_component(c: &str) -> Result<()> {
    let bad = c.is_empty() || c == "." || c == ".."
        || c.chars().any(|ch| ch.is_control() || "<>:\"/\\|?*".contains(ch))
        || c.ends_with('.') || c.ends_with(' ');
    let stem = c.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
        || ((stem.starts_with("COM") || stem.starts_with("LPT")) && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit());
    if bad || reserved { bail!("refused: unsafe file name in torrent: {c:?}"); }
    Ok(())
}

impl App {
    fn add(&self, src: String) {
        let (session, list, status, out) =
            (self.session.clone(), self.torrents.clone(), self.status.clone(), self.out_dir.clone());
        *status.lock().unwrap() = "adding...".into();
        self.rt.spawn(async move {
            let r: Result<Handle> = async {
                let add = if src.starts_with("magnet:") || src.starts_with("http") {
                    AddTorrent::from_url(src)
                } else {
                    AddTorrent::from_bytes(std::fs::read(&src)?)
                };
                // pass 1: metadata only, vet every filename before anything touches disk
                let peek = AddTorrentOptions { list_only: true, output_folder: Some(out.clone()), ..Default::default() };
                let bytes = match session.add_torrent(add, Some(peek)).await? {
                    AddTorrentResponse::AlreadyManaged(_, h) => return Ok(h),
                    AddTorrentResponse::Added(..) => bail!("unexpected add on list_only"),
                    AddTorrentResponse::ListOnly(r) => {
                        for fd in r.info.iter_file_details() {
                            if fd.attrs().symlink { bail!("refused: torrent contains a symlink"); }
                            for c in fd.filename.to_vec() { check_component(&c)?; }
                        }
                        r.torrent_bytes
                    }
                };
                let opts = AddTorrentOptions { overwrite: true, output_folder: Some(out), ..Default::default() };
                match session.add_torrent(AddTorrent::from_bytes(bytes), Some(opts)).await? {
                    AddTorrentResponse::Added(_, h) | AddTorrentResponse::AlreadyManaged(_, h) => Ok(h),
                    AddTorrentResponse::ListOnly(_) => bail!("list only"),
                }
            }
            .await;
            match r {
                Ok(h) => {
                    let mut l = list.lock().unwrap();
                    if !l.iter().any(|x| Arc::ptr_eq(x, &h)) { l.push(h); }
                    *status.lock().unwrap() = String::new();
                }
                Err(e) => *status.lock().unwrap() = format!("error: {e:#}"),
            }
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        // drag and drop .torrent files
        let dropped: Vec<_> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        for p in dropped { self.add(p.display().to_string()); }

        egui::CentralPanel::default().show(root, |ui| {
            ui.heading("Rivulet");
            ui.horizontal(|ui| {
                ui.label("Magnet / URL:");
                ui.add(egui::TextEdit::singleline(&mut self.input).desired_width(ui.available_width() - 60.0));
                if ui.button("Add").clicked() && !self.input.trim().is_empty() {
                    let s = self.input.trim().to_string();
                    self.input.clear();
                    self.add(s);
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Open .torrent file...").clicked() {
                    if let Some(p) = rfd::FileDialog::new().add_filter("torrent", &["torrent"]).pick_file() {
                        self.add(p.display().to_string());
                    }
                }
                ui.label("Save to:");
                ui.label(&self.out_dir);
                if ui.button("Change...").clicked() {
                    if let Some(p) = rfd::FileDialog::new().set_directory(&self.out_dir).pick_folder() {
                        self.out_dir = p.display().to_string();
                    }
                }
                if ui.button("Open folder").clicked() {
                    let _ = std::process::Command::new("explorer").arg(&self.out_dir).spawn();
                }
            });
            let st = self.status.lock().unwrap().clone();
            if !st.is_empty() { ui.colored_label(egui::Color32::YELLOW, st); }
            ui.label("Tip: drag .torrent files onto this window.");
            ui.separator();

            let list = self.torrents.lock().unwrap().clone();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for h in list {
                    let s = h.stats();
                    let frac = if s.total_bytes > 0 { s.progress_bytes as f32 / s.total_bytes as f32 } else { 0.0 };
                    let (speed, peers) = s.live.as_ref()
                        .map(|l| (l.download_speed.mbps, l.snapshot.peer_stats.live))
                        .unwrap_or((0.0, 0));
                    let name = h.name().unwrap_or_else(|| "(fetching metadata)".into());
                    ui.label(egui::RichText::new(name).strong());
                    let text = if s.finished { "done".to_string() } else {
                        format!("{:.1}%  {:.1}/{:.1} MiB  {speed:.2} MiB/s  peers {peers}",
                            frac * 100.0, s.progress_bytes as f64 / 1048576.0, s.total_bytes as f64 / 1048576.0)
                    };
                    ui.add(egui::ProgressBar::new(frac).text(text));
                    ui.add_space(6.0);
                }
            });
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }
}

fn main() -> eframe::Result<()> {
    let rt = tokio::runtime::Runtime::new().expect("tokio");
    let out_dir = default_dir();
    let trackers = [
        "udp://tracker.opentrackr.org:1337/announce",
        "udp://open.stealth.si:80/announce",
        "udp://tracker.torrent.eu.org:451/announce",
        "udp://exodus.desync.com:6969/announce",
        "udp://open.demonii.com:1337/announce",
        "udp://tracker.openbittorrent.com:6969/announce",
    ].iter().filter_map(|t| url::Url::parse(t).ok()).collect();
    let opts = librqbit::SessionOptions {
        // default is no listener: no incoming peers at all
        listen: Some(librqbit::ListenerOptions {
            mode: librqbit::ListenerMode::TcpOnly, // uTP pulls ringbuf 0.4.8 (RUSTSEC-2026-0293)
            listen_addr: (std::net::Ipv6Addr::UNSPECIFIED, 6881).into(),
            enable_upnp_port_forwarding: false, // never open router ports
            ..Default::default()
        }),
        peer_limit: Some(400), // default 128
        trackers,
        fastresume: true,
        persistence: Some(librqbit::SessionPersistenceConfig::Json { folder: None }),
        ..Default::default()
    };
    let session = rt.block_on(Session::new_with_opts(PathBuf::from(&out_dir), opts)).expect("session");
    // persisted torrents resume in the session; show them
    let restored: Vec<Handle> = session.with_torrents(|it| it.map(|(_, h)| h.clone()).collect());
    let app = App {
        rt, session, input: String::new(), out_dir,
        torrents: Arc::new(Mutex::new(restored)), status: Default::default(),
    };
    if let Some(arg) = std::env::args().nth(1) { app.add(arg); } // "Open with" / file association
    eframe::run_native(
        "Rivulet",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([720.0, 420.0]).with_drag_and_drop(true),
            ..Default::default()
        },
        Box::new(|_| Ok(Box::new(app))),
    )
}
