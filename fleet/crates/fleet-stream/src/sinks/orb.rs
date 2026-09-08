//! `OrbSink`: projects a `lane_status` receipt into `contracts/lane-status.v1.json`'s shape and
//! POSTs it to a configured local URL. Mirrors `console.rs`'s `load_lane_status` (see §5).

use fleet_types::ReceiptEvent;

use crate::event::StreamEvent;
use crate::sink::{Sink, SinkError};

use super::orb_project::{permanent, project};

pub struct OrbSink {
    url: String,
    client: reqwest::blocking::Client,
}

impl OrbSink {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl Sink for OrbSink {
    fn id(&self) -> &'static str {
        "orb"
    }

    fn accepts(&self, event: &StreamEvent) -> bool {
        event.0.event == ReceiptEvent::LaneStatus
    }

    fn deliver(&mut self, event: &StreamEvent) -> Result<(), SinkError> {
        let body = project(&event.0)?;
        let response = self
            .client
            .post(&self.url)
            .json(&body)
            .send()
            .map_err(|err| SinkError::Transient {
                sink: "orb",
                reason: err.to_string(),
            })?;
        let status = response.status();
        if status.is_success() {
            Ok(())
        } else if status.is_client_error() {
            Err(permanent(event.seq(), format!("orb rejected with {status}")))
        } else {
            Err(SinkError::Transient {
                sink: "orb",
                reason: format!("orb responded {status}"),
            })
        }
    }
}
