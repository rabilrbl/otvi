//! Hot-reload watcher for provider YAML configs.
//!
//! Spawns a background Tokio task that watches the `providers/` directory for
//! file-system events.  When a `.yaml` or `.yml` file is created, modified, or
//! deleted the in-memory provider map in [`AppState`] is updated without
//! restarting the server.
//!
//! # Usage
//!
//! Call [`spawn`] once after [`AppState`] is created:
//!
//! ```ignore
//! watcher::spawn(state.clone(), providers_dir.clone());
//! ```

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

use crate::state::AppState;

/// Spawn a background task that watches `dir` for YAML changes and
/// hot-reloads the provider map in `state`.
///
/// The task runs until the server exits; errors are logged but never fatal.
pub fn spawn(state: Arc<AppState>, dir: String) {
    tokio::spawn(async move {
        if let Err(e) = run(state, &dir).await {
            tracing::error!("Provider watcher exited with error: {e}");
        }
    });
}

async fn run(state: Arc<AppState>, dir: &str) -> anyhow::Result<()> {
    // notify is sync; bridge events into an async channel.
    let (tx, mut rx) = mpsc::channel::<notify::Result<Event>>(64);

    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
        // If the channel is full we drop the event; the next one will retrigger.
        let _ = tx.blocking_send(res);
    })?;

    let watch_path = Path::new(dir);
    if !watch_path.exists() {
        tracing::warn!(
            dir,
            "Provider hot-reload: directory does not exist, watcher not started"
        );
        return Ok(());
    }

    watcher.watch(watch_path, RecursiveMode::NonRecursive)?;
    tracing::info!(dir, "Provider hot-reload watcher started");

    // Debounce: wait a short interval after an event before re-reading disk so
    // that editors that write via tmp-rename don't trigger two reloads.
    let debounce = Duration::from_millis(300);

    loop {
        // Wait for the first event.
        let event = match rx.recv().await {
            Some(Ok(e)) => e,
            Some(Err(e)) => {
                tracing::warn!("Watcher error: {e}");
                continue;
            }
            None => break, // channel closed
        };

        if !is_yaml_event(&event) {
            continue;
        }

        // Drain any additional events that arrive within the debounce window.
        tokio::time::sleep(debounce).await;
        while let Ok(extra) = rx.try_recv() {
            let _ = extra; // discard
        }

        reload_providers(&state, dir);
    }

    Ok(())
}

/// Returns `true` when the event affects a `.yaml` / `.yml` file and is a
/// create, modify, or remove operation.
fn is_yaml_event(event: &Event) -> bool {
    let relevant = matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    );
    if !relevant {
        return false;
    }
    event.paths.iter().any(|p| {
        p.extension()
            .is_some_and(|ext| ext == "yaml" || ext == "yml")
    })
}

/// Re-scan `dir` and atomically replace the provider map in `state`.
fn reload_providers(state: &AppState, dir: &str) {
    let new_providers = match crate::state::load_provider_map(dir) {
        Ok(map) => map,
        Err(e) => {
            tracing::error!("Provider hot-reload: cannot reload providers from '{dir}': {e}");
            return;
        }
    };

    // Atomically swap the map using the write lock.
    match state.providers_rw.write() {
        Ok(mut guard) => {
            let added: Vec<_> = new_providers
                .keys()
                .filter(|id| !guard.contains_key(*id))
                .cloned()
                .collect();
            let removed: Vec<_> = guard
                .keys()
                .filter(|id| !new_providers.contains_key(*id))
                .cloned()
                .collect();

            *guard = new_providers;

            if !added.is_empty() {
                tracing::info!(providers = ?added, "Provider hot-reload: added");
            }
            if !removed.is_empty() {
                tracing::info!(providers = ?removed, "Provider hot-reload: removed");
            }
            tracing::info!("Provider map updated ({} provider(s))", guard.len());
        }
        Err(e) => {
            tracing::error!("Provider hot-reload: RwLock poisoned: {e}");
        }
    }
}
