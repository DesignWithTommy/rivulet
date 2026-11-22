use anyhow::{Context, Result, bail};
use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse, Session};
use std::{path::PathBuf, time::Duration};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        bail!("usage: torrent-dl <file.torrent|magnet:...|http url> [out_dir]");
    }
    let src = &args[1];
    let out = PathBuf::from(args.get(2).map(String::as_str).unwrap_or("."));
    std::fs::create_dir_all(&out)?;

    let add = if src.starts_with("magnet:") || src.starts_with("http") {
        AddTorrent::from_url(src.clone())
    } else {
        AddTorrent::from_bytes(std::fs::read(src).with_context(|| format!("read {src}"))?)
    };

    let session = Session::new(out.clone()).await?;
    let handle = match session
        .add_torrent(add, Some(AddTorrentOptions { overwrite: true, ..Default::default() }))
        .await?
    {
        AddTorrentResponse::Added(_, h) | AddTorrentResponse::AlreadyManaged(_, h) => h,
        AddTorrentResponse::ListOnly(_) => bail!("list only"),
    };

    let h2 = handle.clone();
    let progress = tokio::spawn(async move {
        loop {
            let s = h2.stats();
            let pct = if s.total_bytes > 0 { s.progress_bytes as f64 * 100.0 / s.total_bytes as f64 } else { 0.0 };
            let (speed, peers) = s.live.as_ref()
                .map(|l| (l.download_speed.mbps, l.snapshot.peer_stats.live))
                .unwrap_or((0.0, 0));
            eprint!("\r{pct:6.2}%  {:.1}/{:.1} MiB  {speed:.2} MiB/s  peers {peers}   ",
                s.progress_bytes as f64 / 1048576.0, s.total_bytes as f64 / 1048576.0);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    handle.wait_until_completed().await?;
    progress.abort();
    eprintln!("\ndone -> {}", out.display());
    Ok(())
}
