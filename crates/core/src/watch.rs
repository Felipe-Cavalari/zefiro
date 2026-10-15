//! Observa pastas e avisa uma vez por rajada de gravações (editores gravam em mais de um passo).

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

pub struct FileWatcher {
    _watcher: RecommendedWatcher,
}

/// Quando o `FileWatcher` é descartado, o `notify` larga o canal; o `recv` da thread falha e ela termina.
pub fn watch_paths(
    dirs: &[PathBuf],
    debounce: Duration,
    on_change: impl Fn(Vec<PathBuf>) + Send + 'static,
) -> notify::Result<FileWatcher> {
    let (tx, rx) = mpsc::channel::<PathBuf>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res
            && (event.kind.is_create() || event.kind.is_modify() || event.kind.is_remove())
        {
            for p in event.paths {
                let _ = tx.send(p);
            }
        }
    })?;
    for d in dirs {
        std::fs::create_dir_all(d).ok();
        watcher.watch(d, RecursiveMode::NonRecursive)?;
    }
    std::thread::Builder::new()
        .name("zefiro-watch".into())
        .spawn(move || {
            while let Ok(first) = rx.recv() {
                let mut batch = vec![first];
                while let Ok(p) = rx.recv_timeout(debounce) {
                    batch.push(p);
                }
                batch.sort();
                batch.dedup();
                on_change(batch);
            }
        })
        .map_err(|e| notify::Error::generic(&e.to_string()))?;
    Ok(FileWatcher { _watcher: watcher })
}
