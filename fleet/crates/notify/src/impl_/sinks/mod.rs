//! The six concrete `Sink` implementations: `OrbSink`, `DashboardSink`, `MeterSink`,
//! `WebhookSink`, `OtelSink`, `FileSink`.

mod dashboard;
mod file;
mod meter;
mod meter_types;
mod otel;
mod otel_payload;
mod orb;
mod orb_project;
mod webhook;

pub use dashboard::DashboardSink;
pub use file::FileSink;
pub use meter::MeterSink;
pub use meter_types::{MeterSample, MeterSnapshot};
pub use otel::OtelSink;
pub use orb::OrbSink;
pub use webhook::{WebhookConfig, WebhookSink};
