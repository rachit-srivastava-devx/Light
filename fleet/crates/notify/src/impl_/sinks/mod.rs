//! The six concrete `Sink` implementations: `OrbSink`, `DashboardSink`, `MeterSink`,
//! `WebhookSink`, `OtelSink`, `FileSink`.

mod dashboard;
mod file;
mod meter;
mod meter_types;
mod orb;
mod orb_project;
mod otel;
mod otel_payload;
mod webhook;

pub use dashboard::DashboardSink;
pub use file::FileSink;
pub use meter::MeterSink;
pub use meter_types::{MeterSample, MeterSnapshot};
pub use orb::OrbSink;
pub use otel::OtelSink;
pub use webhook::{WebhookConfig, WebhookSink};
