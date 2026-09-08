//! `OtelSink`: shells out to the existing `telemetry_otel.py span` -- no `opentelemetry` crate.
//! Never re-implements the OTel wire protocol in Rust.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::Value;

use super::otel_payload::payload;
use crate::event::StreamEvent;
use crate::sink::{Sink, SinkError};

pub struct OtelSink {
    script_path: PathBuf,
}

impl OtelSink {
    pub fn new(script_path: impl Into<PathBuf>) -> Self {
        Self { script_path: script_path.into() }
    }
}

impl Sink for OtelSink {
    fn id(&self) -> &'static str {
        "otel"
    }

    fn accepts(&self, _event: &StreamEvent) -> bool {
        true
    }

    fn deliver(&mut self, event: &StreamEvent) -> Result<(), SinkError> {
        let mut child = Command::new("python3")
            .arg(&self.script_path)
            .arg("span")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| transient(err.to_string()))?;
        let stdin = child.stdin.take().expect("piped stdin");
        write_stdin(stdin, &payload(event))?;
        let output = child.wait_with_output().map_err(|err| transient(err.to_string()))?;
        match output.status.code() {
            Some(2) => Err(SinkError::Permanent {
                sink: "otel",
                seq: event.seq(),
                reason: "telemetry_otel.py: unknown command".into(),
            }),
            Some(0) if serde_json::from_slice::<Value>(&output.stdout).is_ok() => Ok(()),
            _ => Err(transient(String::from_utf8_lossy(&output.stderr).into_owned())),
        }
    }
}

fn write_stdin(mut stdin: impl Write, payload: &Value) -> Result<(), SinkError> {
    serde_json::to_writer(&mut stdin, payload).map_err(|err| transient(err.to_string()))
}

fn transient(reason: String) -> SinkError {
    SinkError::Transient { sink: "otel", reason }
}
