//! `GithubAdapter` -- polls GitHub's REST events API (never an inbound webhook listener, per
//! BLUEPRINT.md's divergence note: an inbound listener would itself be a resident daemon).

use super::github_map::map_kind;
use crate::adapter::Adapter;
use crate::clock::Clock;
use crate::envelope::EventEnvelope;
use crate::ids::EventId;
use crate::source_kind::SourceKind;
use serde_json::Value;
use std::time::Duration;

const POLL_TIMEOUT: Duration = Duration::from_secs(10);

/// A failure polling GitHub's REST API.
#[derive(Debug, thiserror::Error)]
pub enum GithubAdapterError {
    #[error("github poll request failed: {0}")]
    Request(#[from] reqwest::Error),
}

/// Polls `events_url` (a GitHub repo/org events endpoint) using an `ETag`-based cursor.
pub struct GithubAdapter {
    events_url: String,
    token: String,
    client: reqwest::blocking::Client,
    last_etag: Option<String>,
}

impl GithubAdapter {
    pub fn new(events_url: String, token: String) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(POLL_TIMEOUT)
            .build()
            .unwrap_or_default();
        Self { events_url, token, client, last_etag: None }
    }
}

impl Adapter for GithubAdapter {
    type Error = GithubAdapterError;

    fn source(&self) -> SourceKind {
        SourceKind::GithubWebhook
    }

    fn pull(&mut self, clock: &dyn Clock) -> Result<Vec<EventEnvelope>, Self::Error> {
        let mut req = self.client.get(&self.events_url).bearer_auth(&self.token);
        if let Some(etag) = &self.last_etag {
            req = req.header("If-None-Match", etag.as_str());
        }
        let resp = req.send()?;
        if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(vec![]);
        }
        if let Some(etag) = resp.headers().get("etag") {
            if let Ok(etag) = etag.to_str() {
                self.last_etag = Some(etag.to_string());
            }
        }
        let events: Vec<Value> = resp.json()?;
        let mut envelopes = Vec::new();
        for event in events {
            let Some(type_name) = event.get("type").and_then(Value::as_str) else { continue };
            let Some(kind) = map_kind(type_name) else { continue };
            let Some(external_id) = event.get("id").and_then(Value::as_str) else { continue };
            let id = EventId::derive(SourceKind::GithubWebhook, kind, external_id);
            envelopes.push(EventEnvelope::new(
                id,
                SourceKind::GithubWebhook,
                kind,
                clock.now_rfc3339(),
                event,
            ));
        }
        Ok(envelopes)
    }
}
