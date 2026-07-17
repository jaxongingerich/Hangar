use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub struct WatcherHandle(pub Mutex<Option<RecommendedWatcher>>);
pub struct SweeperHandles(pub Mutex<Vec<RecommendedWatcher>>);

/// Watch an external folder (e.g. ~/Downloads); files matching the sweep
/// patterns get moved into the root's `_Inbox` with a toast on the frontend.
pub fn start_sweeper(
    app: AppHandle,
    watched_dir: PathBuf,
    inbox: PathBuf,
    patterns: String,
) -> notify::Result<RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else { return };
        if !matches!(event.kind, notify::EventKind::Create(_)) {
            return;
        }
        for path in &event.paths {
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            if !crate::rules::rule_matches("glob", &patterns, &name) {
                continue;
            }
            // Give the writing process a moment to finish.
            std::thread::sleep(std::time::Duration::from_millis(1500));
            if !path.exists() {
                continue;
            }
            let _ = std::fs::create_dir_all(&inbox);
            let final_name = crate::ops::dedupe_name(&inbox, &name);
            let dest = inbox.join(&final_name);
            let moved = std::fs::rename(path, &dest).or_else(|_| {
                // Cross-volume: copy then remove.
                std::fs::copy(path, &dest).and_then(|_| std::fs::remove_file(path))
            });
            if moved.is_ok() {
                tracing::info!("swept {} → _Inbox", final_name);
                let _ = app.emit("swept", final_name.clone());
            }
        }
    })?;
    watcher.watch(&watched_dir, RecursiveMode::NonRecursive)?;
    tracing::info!("sweeping {}", watched_dir.display());
    Ok(watcher)
}

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
