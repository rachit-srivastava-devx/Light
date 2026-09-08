//! `FileSink`: append-only NDJSON writer, zero server, zero network.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use fleet_types::ReceiptEvent;

use crate::event::StreamEvent;
use crate::sink::{Sink, SinkError};

pub struct FileSink {
    path: PathBuf,
    allowed: Vec<ReceiptEvent>,
    handle: Option<File>,
}

impl FileSink {
    pub fn new(path: impl Into<PathBuf>, allowed: Vec<ReceiptEvent>) -> Self {
        Self {
            path: path.into(),
            allowed,
            handle: None,
        }
    }

    fn open(&mut self) -> Result<&mut File, SinkError> {
        if self.handle.is_none() {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
                .map_err(|err| transient(err.to_string()))?;
            self.handle = Some(file);
        }
        Ok(self.handle.as_mut().expect("just set"))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn transient(reason: String) -> SinkError {
    SinkError::Transient { sink: "file", reason }
}

impl Sink for FileSink {
    fn id(&self) -> &'static str {
        "file"
    }

    fn accepts(&self, event: &StreamEvent) -> bool {
        self.allowed.is_empty() || self.allowed.contains(&event.0.event)
    }

    fn deliver(&mut self, event: &StreamEvent) -> Result<(), SinkError> {
        let mut line = serde_json::to_vec(&event.0).map_err(|err| transient(err.to_string()))?;
        line.push(b'\n');
        self.open()?
            .write_all(&line)
            .map_err(|err| transient(err.to_string()))
    }
}

// Never `Permanent` on its own -- a well-formed JSON write to an open handle does not
// "permanently reject" a specific event.
