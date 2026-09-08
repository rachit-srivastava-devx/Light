//! `pump`: spawn one `run_sink` task per `(sink, config)` pair, run them concurrently.

use std::sync::Arc;

use crate::cursor::CursorStore;
use crate::log_source::{LogSource, LogSourceError};
use crate::sink::Sink;

use super::{worker::run_sink, PumpConfig, SinkStats};

/// This crate's one public "just run it" entry point; `src/` calls this once at process start
/// with the concrete `LogSource`/`CursorStore` it built from `fleet-store` and the sink list
/// assembled from config.
pub async fn pump(
    source: Arc<dyn LogSource + 'static>,
    cursors: Arc<dyn CursorStore + 'static>,
    sinks: Vec<(Box<dyn Sink + 'static>, PumpConfig)>,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Vec<Result<SinkStats, LogSourceError>> {
    let mut tasks = Vec::with_capacity(sinks.len());
    for (mut sink, config) in sinks {
        let source = Arc::clone(&source);
        let cursors = Arc::clone(&cursors);
        let mut shutdown = shutdown.clone();
        tasks.push(tokio::spawn(async move {
            run_sink(source.as_ref(), cursors.as_ref(), sink.as_mut(), &config, &mut shutdown).await
        }));
    }
    let mut results = Vec::with_capacity(tasks.len());
    for task in tasks {
        results.push(match task.await {
            Ok(result) => result,
            Err(_) => Err(LogSourceError::Unavailable("sink worker task panicked".into())),
        });
    }
    results
}
