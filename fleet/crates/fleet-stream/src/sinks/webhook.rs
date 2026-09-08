//! `WebhookSink`: POSTs the raw `Receipt` as JSON to a configured URL, filtered by an
//! allow-list of `ReceiptEvent` variants.

use fleet_types::ReceiptEvent;

use crate::event::StreamEvent;
use crate::sink::{Sink, SinkError};

/// Configuration for `WebhookSink`. `allowed` defaults to every `ReceiptEvent` variant.
pub struct WebhookConfig {
    pub url: String,
    pub allowed: Vec<ReceiptEvent>,
    pub bearer_token: Option<String>,
}

pub struct WebhookSink {
    config: WebhookConfig,
    client: reqwest::blocking::Client,
}

impl WebhookSink {
    pub fn new(config: WebhookConfig) -> Self {
        Self {
            config,
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl Sink for WebhookSink {
    fn id(&self) -> &'static str {
        "webhook"
    }

    fn accepts(&self, event: &StreamEvent) -> bool {
        self.config.allowed.is_empty() || self.config.allowed.contains(&event.0.event)
    }

    fn deliver(&mut self, event: &StreamEvent) -> Result<(), SinkError> {
        let mut request = self.client.post(&self.config.url).json(&event.0);
        if let Some(token) = &self.config.bearer_token {
            request = request.bearer_auth(token);
        }
        let response = request.send().map_err(|err| SinkError::Transient {
            sink: "webhook",
            reason: err.to_string(),
        })?;
        let status = response.status();
        if status.is_success() {
            Ok(())
        } else if status.is_client_error() {
            Err(SinkError::Permanent {
                sink: "webhook",
                seq: event.seq(),
                reason: format!("webhook rejected with {status}"),
            })
        } else {
            Err(SinkError::Transient {
                sink: "webhook",
                reason: format!("webhook responded {status}"),
            })
        }
    }
}
