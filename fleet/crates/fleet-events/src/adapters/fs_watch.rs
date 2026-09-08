//! `FsAdapter` -- filesystem ingress via `notify`, bounded-timeout poll mode. Never a resident
//! indefinite watch: each `pull` drains events for a fixed `Duration`, then returns.

use crate::adapter::Adapter;
use crate::clock::Clock;
use crate::envelope::EventEnvelope;
use crate::event_kind::EventKind;
use crate::source_kind::SourceKind;
use crate::ids::EventId;
use notify::{Config, Event, EventKind as NotifyKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::json;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

/// A failure watching or reading from the filesystem.
#[derive(Debug, thiserror::Error)]
pub enum FsAdapterError {
    #[error("filesystem watch failed: {0}")]
    Watch(#[from] notify::Error),
}

/// Watches `root` and drains whatever fs events arrive within `poll_timeout` on each `pull`.
pub struct FsAdapter {
    root: PathBuf,
    poll_timeout: Duration,
    generation: u64,
}

impl FsAdapter {
    pub fn new(root: PathBuf, poll_timeout: Duration) -> Self {
        Self { root, poll_timeout, generation: 0 }
    }

    fn kind_of(event: &Event) -> Option<EventKind> {
        match event.kind {
            NotifyKind::Create(_) => Some(EventKind::FsFileCreated),
            NotifyKind::Modify(_) => Some(EventKind::FsFileModified),
            NotifyKind::Remove(_) => Some(EventKind::FsFileDeleted),
            _ => None,
        }
    }
}

impl Adapter for FsAdapter {
    type Error = FsAdapterError;

    fn source(&self) -> SourceKind {
        SourceKind::FsWatch
    }

    fn pull(&mut self, clock: &dyn Clock) -> Result<Vec<EventEnvelope>, Self::Error> {
        let (tx, rx): (_, Receiver<notify::Result<Event>>) = channel();
        let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
        watcher.watch(&self.root, RecursiveMode::Recursive)?;
        self.generation += 1;

        let mut envelopes = Vec::new();
        while let Ok(Ok(event)) = rx.recv_timeout(self.poll_timeout) {
            let Some(kind) = Self::kind_of(&event) else { continue };
            for path in &event.paths {
                let external_id = format!("{}#{}", path.display(), self.generation);
                let id = EventId::derive(SourceKind::FsWatch, kind, &external_id);
                let payload = json!({ "path": path.display().to_string() });
                envelopes.push(EventEnvelope::new(
                    id,
                    SourceKind::FsWatch,
                    kind,
                    clock.now_rfc3339(),
                    payload,
                ));
            }
        }
        Ok(envelopes)
    }
}
