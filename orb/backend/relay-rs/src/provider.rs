//! STT/TTS providers behind traits, so the session loop is testable without a network.
//!
//! Real implementations (Sarvam Saarika / Deepgram Nova-3 for STT; Fish Audio S2 / Sarvam Bulbul /
//! Cartesia for TTS — docs/BUILD-DIGEST.md §1) plug in here. `ORB_RELAY_PROVIDER` (fake | http |
//! external) has no implicit default — it must be set explicitly, including `fake` for local
//! dev/testing (`ProviderConfigError::MissingBackend` otherwise) — and production can use the HTTP
//! contract adapter without changing the socket/session layers.
//!
//! Every call takes a `&dyn CancelSignal`. Provider calls are the long poles on this connection —
//! a real Fish synthesis is seconds, not milliseconds — and a barge-in that cannot reach into one
//! of them can only ever cancel the NEXT turn (docs/REVIEW-2026-08-06-END-TO-END-FLOW.md, Fix 9).
//! The signal is polled from blocking code, so it must be cheap and non-blocking to read.

use serde_json::{json, Value};
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How long a provider socket read parks before the cancel flag is re-checked. This is the
/// worst-case contribution of a blocked provider call to barge-in yield latency, so it is sized
/// well under `latency_budget::BARGE_IN_YIELD_BUDGET_MS` (100ms) rather than at it.
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(10);
/// Bounded wait for the TCP handshake to the sidecar. Without this a black-holed sidecar host
/// parks the synthesis thread until the OS gives up (~75s on macOS).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Hard ceiling on a provider response. The TTS contract encodes audio as a JSON array of integers
/// (~4-5 bytes per PCM byte), so a 60s utterance is a few MB; anything past this is a runaway
/// provider, not a long sentence, and must not be accumulated into memory unbounded.
const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcript {
    pub text: String,
    pub is_final: bool,
}

#[derive(Debug)]
pub enum ProviderError {
    Unavailable(String),
    /// The caller cancelled while the request was in flight (barge-in, pause, teardown).
    ///
    /// Deliberately NOT a flavour of `Unavailable`: a cancel is a normal, user-initiated outcome
    /// and must never close the session or be reported as a provider failure. Conflating the two
    /// would make every barge-in look like an outage in dev-logs/ and drop the user's bed.
    Cancelled,
    /// The provider call ran past `main.rs`'s `PROVIDER_TIMEOUT` without returning.
    ///
    /// Deliberately NOT a flavour of `Unavailable`: a hung provider left its `spawn_blocking`
    /// thread still running (this variant is synthesised by the caller when the timeout elapses,
    /// not by the provider itself), and dev-logs/ needs to tell "the provider answered with an
    /// error" apart from "the provider never answered at all" when someone is diagnosing a stall.
    TimedOut,
}

/// Polled from blocking provider code between socket reads. Implementations must never block.
pub trait CancelSignal: Send + Sync {
    fn is_cancelled(&self) -> bool;
}

/// For call sites with nothing to cancel — unit tests, and any caller that owns no flag.
///
/// `allow(dead_code)`: this is part of the provider boundary (an out-of-tree adapter needs a
/// no-op signal to test against), but the binary itself always has a real `CancelFlag` to pass,
/// so nothing in `main.rs` constructs one.
#[allow(dead_code)]
pub struct NeverCancelled;

impl CancelSignal for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}

pub trait SttProvider: Send + Sync {
    /// Feeds one audio frame, returning a partial transcript if the provider emitted one.
    fn push_audio(
        &self,
        session_id: &str,
        frame: &[u8],
        cancel: &dyn CancelSignal,
    ) -> Result<Option<Transcript>, ProviderError>;
    /// Signals end-of-turn; returns the final transcript.
    fn end_of_turn(
        &self,
        session_id: &str,
        cancel: &dyn CancelSignal,
    ) -> Result<Transcript, ProviderError>;
}

pub trait TtsProvider: Send + Sync {
    /// Synthesises `text`, invoking `on_chunk` once per audio chunk **as it becomes available**,
    /// in playback order — not after the whole utterance has been produced. This is the fix for
    /// the "fully buffered before send" defect: the old signature (`-> Result<Vec<Vec<u8>>, _>`)
    /// forced every implementation, however capable the underlying provider, to hold the entire
    /// utterance in memory and return it in one shot before a single byte could reach the client.
    /// A push-style callback is the only shape that lets a genuinely streaming provider (or an
    /// HTTP response arriving as it's synthesised) forward its first chunk early.
    ///
    /// `on_chunk` returning `Err` (typically because the caller was cancelled, or its sink is
    /// gone) must stop synthesis and propagate that same error out of `synthesize`, rather than
    /// swallowing it and continuing to produce more audio nobody wants.
    ///
    /// Returns `ProviderError::Cancelled` if `cancel` fires before the response is complete. The
    /// request may still have reached the paid provider by then — see `SpeechOutcome::Cancelled`
    /// in `session.rs` for how those characters are billed.
    fn synthesize(
        &self,
        text: &str,
        voice_id: &str,
        emotion: &str,
        cancel: &dyn CancelSignal,
        on_chunk: &mut dyn FnMut(Vec<u8>) -> Result<(), ProviderError>,
    ) -> Result<(), ProviderError>;
}

/// Runtime provider boundary. Concrete adapters can replace the T0 fake without changing the
/// socket or session layers. The label is operational evidence, not a claim that a live provider
/// is bundled here.
pub trait ProviderAdapter: SttProvider + TtsProvider {
    fn label(&self) -> &'static str;
}

/// Provider backend selected by deployment configuration. `Http` is a real process boundary, not
/// a provider SDK: a sidecar owns paid credentials and exposes this small JSON contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderBackend {
    Fake,
    Http,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConfig {
    pub backend: ProviderBackend,
    pub http_base_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderConfigError {
    /// `ORB_RELAY_PROVIDER` was never set. There is deliberately no implicit default: a missing
    /// var used to silently resolve to the fake backend, so a deployment that forgot to configure
    /// a real provider would boot and produce synthetic audio with nothing distinguishing it from
    /// real speech. `fake` is still available for local dev/testing, but it must be requested
    /// explicitly (`ORB_RELAY_PROVIDER=fake`) so the choice is visible in config and startup logs.
    MissingBackend,
    UnknownBackend(String),
    MissingHttpBaseUrl,
    InvalidHttpBaseUrl(String),
    ExternalAdapterRequired,
}

impl ProviderConfig {
    pub fn from_env() -> Result<Self, ProviderConfigError> {
        Self::from_backend_var(
            std::env::var("ORB_RELAY_PROVIDER").ok(),
            std::env::var("ORB_RELAY_PROVIDER_URL").ok(),
        )
    }

    /// Pure parsing, independent of process env, so the fail-closed behaviour on a missing
    /// `ORB_RELAY_PROVIDER` can be unit tested without mutating global process state — see
    /// `from_env` for the actual boot-time entry point.
    fn from_backend_var(
        backend_var: Option<String>,
        http_base_url: Option<String>,
    ) -> Result<Self, ProviderConfigError> {
        let backend = backend_var.ok_or(ProviderConfigError::MissingBackend)?;
        let backend = match backend.trim().to_ascii_lowercase().as_str() {
            "fake" => ProviderBackend::Fake,
            "http" => ProviderBackend::Http,
            "external" => ProviderBackend::External,
            _ => return Err(ProviderConfigError::UnknownBackend(backend)),
        };
        Ok(Self {
            backend,
            http_base_url,
        })
    }
}

/// Builds the configured provider. An external adapter is supplied by the composition root so
/// this module remains free of provider SDKs and the socket/session layers remain unchanged.
pub fn provider_from_config(
    config: ProviderConfig,
    external: Option<Arc<dyn ProviderAdapter>>,
) -> Result<Arc<dyn ProviderAdapter>, ProviderConfigError> {
    match config.backend {
        ProviderBackend::Fake => Ok(Arc::new(FakeProvider::new())),
        ProviderBackend::Http => Ok(Arc::new(HttpContractProvider::new(
            config
                .http_base_url
                .as_deref()
                .ok_or(ProviderConfigError::MissingHttpBaseUrl)?,
        )?)),
        ProviderBackend::External => external.ok_or(ProviderConfigError::ExternalAdapterRequired),
    }
}

/// T0 default: deterministic, in-process fake. It is intentionally independent of environment
/// selection so the current relay remains runnable without credentials or paid infrastructure.
#[cfg(test)]
pub fn default_provider() -> Arc<dyn ProviderAdapter> {
    Arc::new(FakeProvider::new())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HttpBase {
    host: String,
    port: u16,
}

fn parse_http_base(base_url: &str) -> Result<HttpBase, ProviderConfigError> {
    let rest = base_url
        .strip_prefix("http://")
        .ok_or_else(|| ProviderConfigError::InvalidHttpBaseUrl(base_url.to_string()))?;
    let host_port = rest
        .trim_end_matches('/')
        .split('/')
        .next()
        .ok_or_else(|| ProviderConfigError::InvalidHttpBaseUrl(base_url.to_string()))?;
    let (host, port_text) = host_port
        .rsplit_once(':')
        .ok_or_else(|| ProviderConfigError::InvalidHttpBaseUrl(base_url.to_string()))?;
    if host.is_empty() {
        return Err(ProviderConfigError::InvalidHttpBaseUrl(
            base_url.to_string(),
        ));
    }
    let port = port_text
        .parse::<u16>()
        .map_err(|_| ProviderConfigError::InvalidHttpBaseUrl(base_url.to_string()))?;
    Ok(HttpBase {
        host: host.to_string(),
        port,
    })
}

/// HTTP JSON provider contract. A paid provider sidecar implements:
/// - `POST /v1/stt/push` -> `{ "text": string|null, "is_final": bool }`
/// - `POST /v1/stt/end` -> `{ "text": string, "is_final": true }`
/// - `POST /v1/tts/synthesize` -> `Transfer-Encoding: chunked` body, one HTTP chunk per audio
///   chunk, each HTTP chunk's payload a JSON object `{ "chunk": [byte, ...] }`, terminated by the
///   standard zero-length chunk.
///
/// TTS synthesis is streamed: `synthesize_streaming` parses the response as it arrives off the
/// socket and calls `on_chunk` per HTTP chunk, so the first audio chunk can reach the client
/// before the sidecar has written its last one — it does not wait for a Content-Length body to
/// close. Cancellation means "stop waiting and drop the connection", not "stop the generator
/// mid-sentence" — the sidecar and the paid provider behind it may well finish (and charge for)
/// work whose bytes this relay throws away.
pub struct HttpContractProvider {
    base: HttpBase,
}

impl HttpContractProvider {
    pub fn new(base_url: &str) -> Result<Self, ProviderConfigError> {
        Ok(Self {
            base: parse_http_base(base_url)?,
        })
    }

    fn connect(&self) -> Result<TcpStream, ProviderError> {
        let mut last_error = None;
        let addrs = (self.base.host.as_str(), self.base.port)
            .to_socket_addrs()
            .map_err(|err| {
                ProviderError::Unavailable(format!("provider address unresolvable: {err}"))
            })?;
        for addr in addrs {
            match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
                Ok(stream) => return Ok(stream),
                Err(err) => last_error = Some(err),
            }
        }
        Err(ProviderError::Unavailable(match last_error {
            Some(err) => format!("provider connect failed: {err}"),
            None => "provider address resolved to nothing".to_string(),
        }))
    }

    fn post_json(
        &self,
        path: &str,
        body: Value,
        cancel: &dyn CancelSignal,
    ) -> Result<Value, ProviderError> {
        if cancel.is_cancelled() {
            return Err(ProviderError::Cancelled);
        }
        let payload = body.to_string();
        let request = format!(
            "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{payload}",
            host = self.base.host,
            len = payload.len(),
        );
        let mut stream = self.connect()?;
        if cancel.is_cancelled() {
            return Err(ProviderError::Cancelled);
        }
        stream
            .write_all(request.as_bytes())
            .map_err(|err| ProviderError::Unavailable(format!("provider write failed: {err}")))?;
        // Park in short slices instead of one unbounded `read_to_string`, so the cancel flag is
        // observed within CANCEL_POLL_INTERVAL rather than after the provider finally answers.
        // Dropping `stream` on the way out closes the connection, which is also how the sidecar
        // learns nobody is waiting any more.
        stream
            .set_read_timeout(Some(CANCEL_POLL_INTERVAL))
            .map_err(|err| {
                ProviderError::Unavailable(format!("provider read timeout unsettable: {err}"))
            })?;
        let mut response = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            if cancel.is_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    if response.len() + read > MAX_RESPONSE_BYTES {
                        return Err(ProviderError::Unavailable(format!(
                            "provider response exceeded {MAX_RESPONSE_BYTES} bytes"
                        )));
                    }
                    response.extend_from_slice(&buffer[..read]);
                }
                // A read timeout is the poll tick, not a failure: loop back and re-check `cancel`.
                // macOS reports it as WouldBlock, Linux as WouldBlock or TimedOut — accept both,
                // and treat a signal-interrupted read the same way.
                Err(err)
                    if matches!(
                        err.kind(),
                        ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
                    ) => {}
                Err(err) => {
                    return Err(ProviderError::Unavailable(format!(
                        "provider read failed: {err}"
                    )))
                }
            }
        }
        let response = String::from_utf8(response).map_err(|err| {
            ProviderError::Unavailable(format!("provider response was not utf-8: {err}"))
        })?;
        let (head, body) = response.split_once("\r\n\r\n").ok_or_else(|| {
            ProviderError::Unavailable("provider returned malformed HTTP response".into())
        })?;
        if !head.starts_with("HTTP/1.1 200") && !head.starts_with("HTTP/1.0 200") {
            return Err(ProviderError::Unavailable(format!(
                "provider returned non-200 response: {}",
                head.lines().next().unwrap_or("<empty>")
            )));
        }
        serde_json::from_str(body)
            .map_err(|err| ProviderError::Unavailable(format!("provider returned bad JSON: {err}")))
    }
}

impl ProviderAdapter for HttpContractProvider {
    fn label(&self) -> &'static str {
        "http-contract"
    }
}

impl SttProvider for HttpContractProvider {
    fn push_audio(
        &self,
        session_id: &str,
        frame: &[u8],
        cancel: &dyn CancelSignal,
    ) -> Result<Option<Transcript>, ProviderError> {
        let value = self.post_json(
            "/v1/stt/push",
            json!({ "session_id": session_id, "audio_bytes": frame }),
            cancel,
        )?;
        let text = value.get("text").and_then(Value::as_str);
        let is_final = value
            .get("is_final")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Ok(text.map(|text| Transcript {
            text: text.to_string(),
            is_final,
        }))
    }

    fn end_of_turn(
        &self,
        session_id: &str,
        cancel: &dyn CancelSignal,
    ) -> Result<Transcript, ProviderError> {
        let value = self.post_json("/v1/stt/end", json!({ "session_id": session_id }), cancel)?;
        let text = value.get("text").and_then(Value::as_str).ok_or_else(|| {
            ProviderError::Unavailable("provider omitted final transcript".into())
        })?;
        let is_final = value
            .get("is_final")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        Ok(Transcript {
            text: text.to_string(),
            is_final,
        })
    }
}

/// Finds the first occurrence of `needle` in `haystack`, or `None` if it isn't (yet) present —
/// used to detect header/chunk boundaries in a growing read buffer without a streaming parser
/// dependency.
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|window| window == needle)
}

impl HttpContractProvider {
    /// Appends whatever is available on `stream` to `buf`, honoring `cancel` the same way
    /// `post_json` does (short read timeout, re-poll, treat the resulting `WouldBlock`/`TimedOut`
    /// as "nothing yet" rather than an error). Returns `Ok(false)` on a clean EOF.
    fn read_more(
        &self,
        stream: &mut TcpStream,
        buf: &mut Vec<u8>,
        cancel: &dyn CancelSignal,
    ) -> Result<bool, ProviderError> {
        let mut chunk = [0u8; 8192];
        loop {
            if cancel.is_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            match stream.read(&mut chunk) {
                Ok(0) => return Ok(false),
                Ok(read) => {
                    if buf.len() + read > MAX_RESPONSE_BYTES {
                        return Err(ProviderError::Unavailable(format!(
                            "provider response exceeded {MAX_RESPONSE_BYTES} bytes"
                        )));
                    }
                    buf.extend_from_slice(&chunk[..read]);
                    return Ok(true);
                }
                Err(err)
                    if matches!(
                        err.kind(),
                        ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
                    ) => {}
                Err(err) => {
                    return Err(ProviderError::Unavailable(format!(
                        "provider read failed: {err}"
                    )))
                }
            }
        }
    }

    /// Streams the `/v1/tts/synthesize` response, calling `on_chunk` per HTTP chunk as it
    /// arrives — the whole point being that this returns from reading chunk 1 without having
    /// read chunk N yet. See the module doc for the wire shape.
    fn synthesize_streaming(
        &self,
        text: &str,
        voice_id: &str,
        emotion: &str,
        cancel: &dyn CancelSignal,
        on_chunk: &mut dyn FnMut(Vec<u8>) -> Result<(), ProviderError>,
    ) -> Result<(), ProviderError> {
        if cancel.is_cancelled() {
            return Err(ProviderError::Cancelled);
        }
        let payload = json!({ "text": text, "voice_id": voice_id, "emotion": emotion }).to_string();
        let request = format!(
            "POST /v1/tts/synthesize HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{payload}",
            host = self.base.host,
            len = payload.len(),
        );
        let mut stream = self.connect()?;
        if cancel.is_cancelled() {
            return Err(ProviderError::Cancelled);
        }
        stream
            .write_all(request.as_bytes())
            .map_err(|err| ProviderError::Unavailable(format!("provider write failed: {err}")))?;
        stream
            .set_read_timeout(Some(CANCEL_POLL_INTERVAL))
            .map_err(|err| {
                ProviderError::Unavailable(format!("provider read timeout unsettable: {err}"))
            })?;

        let mut buf: Vec<u8> = Vec::new();
        let header_end = loop {
            if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
                break pos + 4;
            }
            if !self.read_more(&mut stream, &mut buf, cancel)? {
                return Err(ProviderError::Unavailable(
                    "provider closed connection before headers completed".into(),
                ));
            }
        };
        let head = String::from_utf8(buf[..header_end].to_vec()).map_err(|err| {
            ProviderError::Unavailable(format!("provider header was not utf-8: {err}"))
        })?;
        let mut lines = head.split("\r\n");
        let status_line = lines.next().unwrap_or_default();
        if !status_line.starts_with("HTTP/1.1 200") && !status_line.starts_with("HTTP/1.0 200") {
            return Err(ProviderError::Unavailable(format!(
                "provider returned non-200 response: {status_line}"
            )));
        }
        let chunked = lines.any(|line| {
            line.split_once(':')
                .map(|(name, value)| {
                    name.trim().eq_ignore_ascii_case("transfer-encoding")
                        && value.trim().eq_ignore_ascii_case("chunked")
                })
                .unwrap_or(false)
        });
        if !chunked {
            return Err(ProviderError::Unavailable(
                "provider tts response was not streamed (missing Transfer-Encoding: chunked)"
                    .into(),
            ));
        }

        let mut body = buf[header_end..].to_vec();
        loop {
            if cancel.is_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            let size_line_end = loop {
                if let Some(pos) = find_subslice(&body, b"\r\n") {
                    break pos;
                }
                if !self.read_more(&mut stream, &mut body, cancel)? {
                    return Err(ProviderError::Unavailable(
                        "provider closed connection mid chunk-size line".into(),
                    ));
                }
            };
            let size_line = String::from_utf8(body[..size_line_end].to_vec()).map_err(|err| {
                ProviderError::Unavailable(format!("chunk-size line was not utf-8: {err}"))
            })?;
            let size = usize::from_str_radix(size_line.trim(), 16).map_err(|_| {
                ProviderError::Unavailable(format!("invalid chunk-size line: {size_line:?}"))
            })?;
            // <size-line>\r\n<data>\r\n
            let needed = size_line_end + 2 + size + 2;
            loop {
                if body.len() >= needed {
                    break;
                }
                if !self.read_more(&mut stream, &mut body, cancel)? {
                    return Err(ProviderError::Unavailable(
                        "provider closed connection mid chunk body".into(),
                    ));
                }
            }
            if size == 0 {
                // The terminating zero-length chunk — this sidecar never sends trailers, so the
                // remaining "\r\n" is the whole trailer section.
                return Ok(());
            }
            let data = &body[size_line_end + 2..size_line_end + 2 + size];
            let value: Value = serde_json::from_slice(data).map_err(|err| {
                ProviderError::Unavailable(format!("provider chunk was not valid json: {err}"))
            })?;
            let values = value
                .get("chunk")
                .and_then(Value::as_array)
                .ok_or_else(|| ProviderError::Unavailable("provider chunk omitted \"chunk\"".into()))?;
            let bytes = values
                .iter()
                .map(|byte| {
                    let value = byte.as_u64().ok_or_else(|| {
                        ProviderError::Unavailable("provider audio byte must be unsigned".into())
                    })?;
                    u8::try_from(value).map_err(|_| {
                        ProviderError::Unavailable("provider audio byte exceeded u8".into())
                    })
                })
                .collect::<Result<Vec<u8>, ProviderError>>()?;
            on_chunk(bytes)?;
            body.drain(0..needed);
        }
    }
}

impl TtsProvider for HttpContractProvider {
    fn synthesize(
        &self,
        text: &str,
        voice_id: &str,
        emotion: &str,
        cancel: &dyn CancelSignal,
        on_chunk: &mut dyn FnMut(Vec<u8>) -> Result<(), ProviderError>,
    ) -> Result<(), ProviderError> {
        self.synthesize_streaming(text, voice_id, emotion, cancel, on_chunk)
    }
}

/// Deterministic in-process fake. Records what it was asked to do so tests can assert the session
/// loop's behaviour (e.g. that a barge-in stopped synthesis) rather than just its return values.
#[derive(Default)]
pub struct FakeProvider {
    pub audio_frames_received: Arc<Mutex<usize>>,
    pub synthesized: Arc<Mutex<Vec<String>>>,
    pub fail: bool,
}

impl ProviderAdapter for FakeProvider {
    fn label(&self) -> &'static str {
        "fake-t0"
    }
}

impl FakeProvider {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub fn failing() -> Self {
        Self {
            fail: true,
            ..Self::default()
        }
    }
}

impl SttProvider for FakeProvider {
    fn push_audio(
        &self,
        _session_id: &str,
        frame: &[u8],
        cancel: &dyn CancelSignal,
    ) -> Result<Option<Transcript>, ProviderError> {
        if cancel.is_cancelled() {
            return Err(ProviderError::Cancelled);
        }
        if self.fail {
            return Err(ProviderError::Unavailable("fake stt down".into()));
        }
        // An empty frame is a client bug the real providers would reject too — surfacing it here
        // keeps the fake honest rather than silently counting a frame carrying no audio.
        if frame.is_empty() {
            return Err(ProviderError::Unavailable("empty audio frame".into()));
        }
        let mut count = self
            .audio_frames_received
            .lock()
            .expect("mutex not poisoned");
        *count += 1;
        // Emit a partial every third frame — enough structure for the loop's partial-handling path
        // to be exercised without pretending to model real endpointing.
        if *count % 3 == 0 {
            Ok(Some(Transcript {
                text: format!("partial after {count} frames"),
                is_final: false,
            }))
        } else {
            Ok(None)
        }
    }

    fn end_of_turn(
        &self,
        _session_id: &str,
        cancel: &dyn CancelSignal,
    ) -> Result<Transcript, ProviderError> {
        if cancel.is_cancelled() {
            return Err(ProviderError::Cancelled);
        }
        if self.fail {
            return Err(ProviderError::Unavailable("fake stt down".into()));
        }
        Ok(Transcript {
            text: "final transcript".into(),
            is_final: true,
        })
    }
}

impl TtsProvider for FakeProvider {
    fn synthesize(
        &self,
        text: &str,
        _voice_id: &str,
        _emotion: &str,
        cancel: &dyn CancelSignal,
        on_chunk: &mut dyn FnMut(Vec<u8>) -> Result<(), ProviderError>,
    ) -> Result<(), ProviderError> {
        // Checked before the work, exactly like the HTTP adapter: a cancel that lands before
        // dispatch must bill nothing, and `session.rs` relies on that distinction.
        if cancel.is_cancelled() {
            return Err(ProviderError::Cancelled);
        }
        if self.fail {
            return Err(ProviderError::Unavailable("fake tts down".into()));
        }
        self.synthesized
            .lock()
            .expect("mutex not poisoned")
            .push(text.to_string());
        on_chunk(vec![0u8; 320])?;
        on_chunk(vec![1u8; 320])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;

    /// A cancel signal a test can flip, and that reports how many times it was polled — the only
    /// way to prove the provider actually re-checks it rather than checking once at entry.
    #[derive(Default)]
    struct TestCancel {
        cancelled: AtomicBool,
        polls: Arc<Mutex<usize>>,
    }

    impl CancelSignal for TestCancel {
        fn is_cancelled(&self) -> bool {
            *self.polls.lock().expect("mutex not poisoned") += 1;
            self.cancelled.load(Ordering::Acquire)
        }
    }

    fn read_http_request(stream: &mut TcpStream) -> (String, String) {
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 512];
        loop {
            let read = stream.read(&mut buffer).expect("read request");
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                let text = String::from_utf8(bytes.clone()).expect("utf8 request");
                let length = text
                    .lines()
                    .find_map(|line| line.strip_prefix("Content-Length: "))
                    .and_then(|value| value.parse::<usize>().ok())
                    .expect("content length");
                let header_end = bytes
                    .windows(4)
                    .position(|window| window == b"\r\n\r\n")
                    .expect("header end")
                    + 4;
                while bytes.len() - header_end < length {
                    let read = stream.read(&mut buffer).expect("read body");
                    if read == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&buffer[..read]);
                }
                let request = String::from_utf8(bytes).expect("utf8 full request");
                let first_line = request.lines().next().expect("request line").to_string();
                let body = request
                    .split_once("\r\n\r\n")
                    .map(|(_, body)| body.to_string())
                    .unwrap_or_default();
                return (first_line, body);
            }
        }
        panic!("request ended before headers");
    }

    /// Writes a chunked-transfer TTS response: headers, then one HTTP chunk per entry of
    /// `chunks`, then the zero-length terminator. Sleeping `between` before each chunk (default
    /// `Duration::ZERO`) is what lets a test prove a chunk arrived before a later one was ready.
    fn write_chunked_tts_response(stream: &mut TcpStream, chunks: &[Vec<u8>], between: Duration) {
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n")
            .expect("write tts headers");
        for chunk in chunks {
            if !between.is_zero() {
                thread::sleep(between);
            }
            let body = json!({ "chunk": chunk }).to_string();
            let framed = format!("{:x}\r\n{}\r\n", body.len(), body);
            stream
                .write_all(framed.as_bytes())
                .expect("write tts chunk");
        }
        stream.write_all(b"0\r\n\r\n").expect("write tts terminator");
    }

    fn start_contract_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind contract server");
        let addr = listener.local_addr().expect("server addr");
        thread::spawn(move || {
            for stream in listener.incoming().take(3) {
                let mut stream = stream.expect("incoming stream");
                let (line, body) = read_http_request(&mut stream);
                if line.starts_with("POST /v1/stt/push ") {
                    assert!(body.contains("\"session_id\":\"s1\""));
                    assert!(body.contains("\"audio_bytes\":[1,2,3]"));
                    let response_body = r#"{"text":"hello partial","is_final":false}"#;
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        response_body.len(),
                        response_body
                    );
                    stream
                        .write_all(response.as_bytes())
                        .expect("write response");
                } else if line.starts_with("POST /v1/stt/end ") {
                    let response_body = r#"{"text":"hello final","is_final":true}"#;
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        response_body.len(),
                        response_body
                    );
                    stream
                        .write_all(response.as_bytes())
                        .expect("write response");
                } else if line.starts_with("POST /v1/tts/synthesize ") {
                    assert!(body.contains("\"voice_id\":\"orb.warm.v1\""));
                    write_chunked_tts_response(
                        &mut stream,
                        &[vec![7, 8], vec![9]],
                        Duration::ZERO,
                    );
                } else {
                    let response_body = r#"{"error":"unexpected path"}"#;
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        response_body.len(),
                        response_body
                    );
                    stream
                        .write_all(response.as_bytes())
                        .expect("write response");
                }
            }
        });
        format!("http://{}", addr)
    }

    /// Accepts a connection, reads the request, then goes silent for `hold` — the shape of a real
    /// TTS call, which is seconds of nothing followed by one large body.
    fn start_slow_server(hold: Duration) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind slow server");
        let addr = listener.local_addr().expect("server addr");
        thread::spawn(move || {
            for stream in listener.incoming().take(2) {
                let mut stream = stream.expect("incoming stream");
                let _ = read_http_request(&mut stream);
                thread::sleep(hold);
                write_chunked_tts_response(&mut stream, &[vec![1]], Duration::ZERO);
            }
        });
        format!("http://{}", addr)
    }

    /// Accepts one connection and writes `chunks` as a chunked-transfer TTS response, sleeping
    /// `between` before each one — proving the client can observe chunk 1 before chunk N was
    /// even written, let alone before the whole response closed.
    fn start_streaming_tts_server(chunks: Vec<Vec<u8>>, between: Duration) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind streaming server");
        let addr = listener.local_addr().expect("server addr");
        thread::spawn(move || {
            for stream in listener.incoming().take(1) {
                let mut stream = stream.expect("incoming stream");
                let _ = read_http_request(&mut stream);
                write_chunked_tts_response(&mut stream, &chunks, between);
            }
        });
        format!("http://{}", addr)
    }

    #[test]
    fn default_provider_is_explicitly_t0_fake() {
        let provider = default_provider();
        assert_eq!(provider.label(), "fake-t0");
    }

    #[test]
    fn external_selection_requires_injected_adapter() {
        let result = provider_from_config(
            ProviderConfig {
                backend: ProviderBackend::External,
                http_base_url: None,
            },
            None,
        );
        assert!(matches!(
            result,
            Err(ProviderConfigError::ExternalAdapterRequired)
        ));
    }

    #[test]
    fn missing_backend_config_fails_closed_instead_of_defaulting_to_fake() {
        // No `ORB_RELAY_PROVIDER` at all (the boot-time shape of a deployment that forgot to set
        // it) must be a typed startup error, never a silent choice of the fake provider — a
        // forgotten env var must not become audible fake speech in production.
        let result = ProviderConfig::from_backend_var(None, None);
        assert!(matches!(result, Err(ProviderConfigError::MissingBackend)));
    }

    #[test]
    fn explicit_fake_opt_in_still_works() {
        let result = ProviderConfig::from_backend_var(Some("fake".into()), None)
            .expect("explicit fake backend");
        assert_eq!(result.backend, ProviderBackend::Fake);
    }

    #[test]
    fn http_selection_requires_a_base_url() {
        let result = provider_from_config(
            ProviderConfig {
                backend: ProviderBackend::Http,
                http_base_url: None,
            },
            None,
        );
        assert!(matches!(
            result,
            Err(ProviderConfigError::MissingHttpBaseUrl)
        ));
    }

    #[test]
    fn http_contract_provider_uses_stt_and_tts_contracts() {
        let base = start_contract_server();
        let provider = provider_from_config(
            ProviderConfig {
                backend: ProviderBackend::Http,
                http_base_url: Some(base),
            },
            None,
        )
        .expect("http provider");
        assert_eq!(provider.label(), "http-contract");

        let partial = provider
            .push_audio("s1", &[1, 2, 3], &NeverCancelled)
            .expect("stt push")
            .expect("partial transcript");
        assert_eq!(
            partial,
            Transcript {
                text: "hello partial".into(),
                is_final: false,
            }
        );

        let final_transcript = provider.end_of_turn("s1", &NeverCancelled).expect("stt end");
        assert_eq!(
            final_transcript,
            Transcript {
                text: "hello final".into(),
                is_final: true,
            }
        );

        let mut chunks = Vec::new();
        provider
            .synthesize("hello", "orb.warm.v1", "calm", &NeverCancelled, &mut |c| {
                chunks.push(c);
                Ok(())
            })
            .expect("tts synthesize");
        assert_eq!(chunks, vec![vec![7, 8], vec![9]]);
    }

    #[test]
    fn tts_first_chunk_is_delivered_before_the_last_chunk_is_even_written() {
        // The defect, proven at the HTTP boundary: a genuinely streaming sidecar response must
        // not be held back until the whole thing has arrived. Three chunks, 200ms apart — if the
        // client only sees chunk 1 after ~600ms (the old "one whole JSON body" behaviour), this
        // fails; the fix delivers it within one inter-chunk gap of the request landing.
        let base = start_streaming_tts_server(
            vec![vec![1u8], vec![2u8], vec![3u8]],
            Duration::from_millis(200),
        );
        let provider = HttpContractProvider::new(&base).expect("http provider");
        let started = std::time::Instant::now();
        let mut first_chunk_at = None;
        let mut chunks = Vec::new();
        provider
            .synthesize("hello", "v", "warm", &NeverCancelled, &mut |c| {
                if first_chunk_at.is_none() {
                    first_chunk_at = Some(started.elapsed());
                }
                chunks.push(c);
                Ok(())
            })
            .expect("tts synthesize");
        assert_eq!(chunks, vec![vec![1u8], vec![2u8], vec![3u8]]);
        let first_chunk_at = first_chunk_at.expect("at least one chunk arrived");
        assert!(
            first_chunk_at < Duration::from_millis(450),
            "first chunk arrived at {first_chunk_at:?}, expected well under the ~600ms it takes \
             to write all three chunks — the response must not be fully buffered before send"
        );
    }

    #[test]
    fn synthesis_already_cancelled_never_reaches_the_provider() {
        // The pre-dispatch case: nothing was asked of the paid provider, so nothing is billable.
        let base = start_slow_server(Duration::from_millis(50));
        let provider = HttpContractProvider::new(&base).expect("http provider");
        let cancel = TestCancel::default();
        cancel.cancelled.store(true, Ordering::Release);
        let result = provider.synthesize("hello", "v", "warm", &cancel, &mut |_| Ok(()));
        assert!(matches!(result, Err(ProviderError::Cancelled)));
    }

    #[test]
    fn an_in_flight_synthesis_is_abandoned_when_the_flag_flips() {
        // The case that matters for barge-in: the request is already on the wire and the provider
        // has not answered. Without the polled read loop this call blocks for the full hold.
        let base = start_slow_server(Duration::from_secs(5));
        let provider = HttpContractProvider::new(&base).expect("http provider");
        let cancel = Arc::new(TestCancel::default());
        let flipper = Arc::clone(&cancel);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            flipper.cancelled.store(true, Ordering::Release);
        });
        let started = std::time::Instant::now();
        let result = provider.synthesize("hello", "v", "warm", cancel.as_ref(), &mut |_| Ok(()));
        let elapsed = started.elapsed();
        assert!(
            matches!(result, Err(ProviderError::Cancelled)),
            "expected Cancelled, got {result:?}"
        );
        // Generous bound: the point is "hundreds of ms, not the provider's full 5s", and CI boxes
        // are slow. A regression to the old blocking read shows up as ~5s, not as 1.1s.
        assert!(
            elapsed < Duration::from_secs(2),
            "cancel took {elapsed:?}; it must not wait out the provider"
        );
        assert!(
            *cancel.polls.lock().expect("mutex not poisoned") > 1,
            "the read loop must re-poll the cancel flag, not check it once at entry"
        );
    }

    /// A third-party adapter satisfies the provider boundary without any change to the layers
    /// above it. `session.rs` no longer touches a provider at all (it describes work rather than
    /// performing it), so this contract is asserted here, where the boundary actually lives.
    struct ProbeAdapter;

    impl SttProvider for ProbeAdapter {
        fn push_audio(
            &self,
            _session_id: &str,
            _frame: &[u8],
            _cancel: &dyn CancelSignal,
        ) -> Result<Option<Transcript>, ProviderError> {
            Ok(Some(Transcript {
                text: "probe partial".into(),
                is_final: false,
            }))
        }

        fn end_of_turn(
            &self,
            _session_id: &str,
            _cancel: &dyn CancelSignal,
        ) -> Result<Transcript, ProviderError> {
            Ok(Transcript {
                text: "probe final".into(),
                is_final: true,
            })
        }
    }

    impl TtsProvider for ProbeAdapter {
        fn synthesize(
            &self,
            _text: &str,
            _voice_id: &str,
            _emotion: &str,
            _cancel: &dyn CancelSignal,
            on_chunk: &mut dyn FnMut(Vec<u8>) -> Result<(), ProviderError>,
        ) -> Result<(), ProviderError> {
            on_chunk(vec![7u8; 4])
        }
    }

    impl ProviderAdapter for ProbeAdapter {
        fn label(&self) -> &'static str {
            "external-test-probe"
        }
    }

    #[test]
    fn a_third_party_adapter_satisfies_the_provider_boundary() {
        let provider = ProbeAdapter;
        assert_eq!(provider.label(), "external-test-probe");
        assert_eq!(
            provider
                .push_audio("s1", &[0u8; 320], &NeverCancelled)
                .expect("probe push"),
            Some(Transcript {
                text: "probe partial".into(),
                is_final: false,
            })
        );
        let mut chunks = Vec::new();
        provider
            .synthesize("hello", "probe", "neutral", &NeverCancelled, &mut |c| {
                chunks.push(c);
                Ok(())
            })
            .expect("probe synthesis");
        assert_eq!(chunks, vec![vec![7u8; 4]]);
    }

    #[test]
    fn a_down_provider_reports_an_outage_rather_than_silence() {
        // The distinction the whole `ProviderError` split rests on: a failing provider must be
        // `Unavailable` (which closes the session), never `Cancelled` (which never does).
        let provider = FakeProvider::failing();
        assert!(matches!(
            provider.push_audio("s1", &[1, 2, 3], &NeverCancelled),
            Err(ProviderError::Unavailable(_))
        ));
        assert!(matches!(
            provider.end_of_turn("s1", &NeverCancelled),
            Err(ProviderError::Unavailable(_))
        ));
        assert!(matches!(
            provider.synthesize("hi", "v", "warm", &NeverCancelled, &mut |_| Ok(())),
            Err(ProviderError::Unavailable(_))
        ));
    }

    #[test]
    fn a_cancelled_stt_push_is_not_reported_as_an_outage() {
        let provider = FakeProvider::new();
        let cancel = TestCancel::default();
        cancel.cancelled.store(true, Ordering::Release);
        let result = provider.push_audio("s1", &[1, 2, 3], &cancel);
        assert!(matches!(result, Err(ProviderError::Cancelled)));
        // and the frame was never counted
        assert_eq!(
            *provider.audio_frames_received.lock().expect("mutex"),
            0,
            "a cancelled push must not count as received audio"
        );
    }
}
