use anyhow::Result;
use grepdown_lib::MDDBProject;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc};

/// Start watching the project directory for .md file changes
pub async fn start_watch(project: Arc<Mutex<MDDBProject>>) -> Result<()> {
    let root = {
        let proj = project.lock().await;
        PathBuf::from(proj.get_root())
    };

    let (tx, mut rx) = mpsc::channel::<()>(100);

    // Spawn watcher thread
    let watch_root = root.clone();
    std::thread::spawn(move || {
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                // Only trigger on create/modify/remove events
                if matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                ) {
                    // Check if any .md files were affected
                    let has_md = event
                        .paths
                        .iter()
                        .any(|p| p.extension().map_or(false, |ext| ext == "md"));
                    if has_md {
                        let _ = tx.blocking_send(());
                    }
                }
            }
        })
        .expect("Failed to create file watcher");

        watcher
            .watch(&watch_root, RecursiveMode::Recursive)
            .expect("Failed to watch directory");

        // Keep watcher alive
        loop {
            std::thread::sleep(Duration::from_secs(3600));
        }
    });

    // Handle refresh requests
    let mut last_refresh = std::time::Instant::now();
    let refresh_interval = Duration::from_millis(500); // Debounce: at most every 500ms

    while let Some(()) = rx.recv().await {
        let now = std::time::Instant::now();
        if now.duration_since(last_refresh) >= refresh_interval {
            eprintln!("File change detected, refreshing index...");
            let mut proj = project.lock().await;
            if let Err(e) = proj.refresh() {
                eprintln!("Failed to refresh index: {}", e);
            } else {
                eprintln!("Index refreshed successfully");
            }
            last_refresh = now;
        }
    }

    Ok(())
}
