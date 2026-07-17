use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub struct WatcherHandle(pub Mutex<Option<RecommendedWatcher>>);

/// Watch the root recursively; on a quiet gap after a burst of FS events,
/// rescan and tell the frontend to refetch. Disk is truth — Finder changes
/// show up in Hangar within ~1s.
pub fn start(app: AppHandle, db_path: PathBuf, root: PathBuf) -> notify::Result<RecommendedWatcher> {
    let (tx, rx) = mpsc::channel::<()>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            use notify::EventKind::*;
            if matches!(event.kind, Create(_) | Modify(_) | Remove(_)) {
                let _ = tx.send(());
            }
        }
    })?;
    watcher.watch(&root, RecursiveMode::Recursive)?;
    tracing::info!("watching {}", root.display());

    std::thread::spawn(move || {
        loop {
            // Block until something changes…
            if rx.recv().is_err() {
                break; // watcher dropped
            }
            // …then absorb the burst until 700ms of quiet.
            while rx.recv_timeout(Duration::from_millis(700)).is_ok() {}
            match crate::db::open(&db_path) {
                Ok(mut conn) => match crate::scan::scan(&mut conn, &root) {
                    Ok(stats) => tracing::info!(
                        "watcher rescan: {} projects, {} files, {}ms",
                        stats.projects,
                        stats.files,
                        stats.elapsed_ms
                    ),
                    Err(e) => tracing::warn!("watcher rescan failed: {e}"),
                },
                Err(e) => tracing::warn!("watcher could not open db: {e}"),
            }
            let _ = app.emit("fs-changed", ());
        }
    });

    Ok(watcher)
}
