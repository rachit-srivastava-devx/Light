//! The Focus Orb — realtime audio data plane (docs/adr/0004-backend-language-split.md).
//!
//! Accepts a WebSocket per session, pipes mic frames to STT and TTS audio back. The decisions live
//! in `session::Session` (a pure state machine, fully unit-tested); this file is the socket
//! plumbing that executes its `Action`s and owns every piece of I/O.
//!
//! ## Why this connection is four tasks and not one loop
//!
//! It used to be one sequential loop: read a frame, do the provider work inline, write the result,
//! read the next frame. That shape makes barge-in structurally impossible. A real TTS synthesis is
//! seconds long, and `source.next()` was not called again until the whole utterance had been
//! synthesised AND every chunk flushed — so a `barge_in` frame sent by the client during the
//! orb's speech sat unread in the kernel buffer and could only ever cancel the NEXT turn.
//! Measured: p50 1897ms, p95 2292ms against a 100ms budget, and a control trial that received
//! statistically the same number of audio bytes as an interrupted one (i.e. barge-in truncated
//! nothing at all). Diagnosed as "Fix 9" in docs/REVIEW-2026-08-06-END-TO-END-FLOW.md.
//!
//! So the connection is split:
//!
//!   - the **read loop** (`serve_connection`) only ever parses a frame, asks the session what to
//!     do, and dispatches. It performs no provider I/O and never waits on any, so a control frame
//!     is always at most one already-buffered frame away from being handled;
//!   - the **write task** owns the sink and drops audio belonging to a retired generation before
//!     it reaches the socket;
//!   - the **STT worker** runs pushes and finalises sequentially off the read loop (ordering is
//!     preserved; the read loop is not blocked). Without this, the ~200ms of mic audio the client
//!     sends *before* it qualifies a barge-in would queue an STT round trip per frame ahead of the
//!     `barge_in` itself;
//!   - a **speech task** per utterance, cancellable, holding the only blocking synthesis call.

mod cancel;
mod devlog;
mod latency_budget;
mod protocol;
mod provider;
mod session;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Map, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use cancel::CancelFlag;
use latency_budget::{
    classify, hot_path_budget_ms, hot_path_within_budget, HotPathBudget, TurnKind,
};
use protocol::{ClientFrame, ServerFrame};
use provider::{
    provider_from_config, ProviderAdapter, ProviderBackend, ProviderConfig, ProviderError,
};
use session::{Action, Session, SpeechOutcome, SpeechRequest, SttFailure, SttOutcome};

type WsSink = SplitSink<WebSocketStream<TcpStream>, Message>;
type WsSource = SplitStream<WebSocketStream<TcpStream>>;
/// Errors on the connection path must be `Send`: the read loop holds its result across an `await`
/// inside `tokio::select!`, and a plain `Box<dyn Error>` would make the whole connection future
/// non-`Send` and therefore unspawnable.
type ConnResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// `live_generation` sentinel for "no speech is on the wire". `Session` hands out generations from
/// 1, so 0 can never collide with a real one.
const NO_LIVE_GENERATION: u64 = 0;
/// Outbound queue depth, in frames. At the 320-byte chunks this relay ships that is roughly 2.5s
/// of 16kHz PCM in flight — enough that a normal network hiccup never stalls a speech task, small
/// enough that a wedged client cannot buffer an unbounded amount of audio in this process.
const OUTBOUND_QUEUE: usize = 256;
/// Inbound STT queue depth, in mic frames. At the client's 20-40ms cadence this is 5-10 seconds of
/// backlog; reaching it means STT is not keeping up with realtime, which is an incident, not
/// jitter. Frames past it are dropped loudly rather than blocking the read loop — see
/// `SttDispatch::Full`.
const STT_QUEUE: usize = 256;
/// Depth of the completed-work channel feeding results back to the read loop.
const OUTCOME_QUEUE: usize = 256;
/// How long the TTS send path sleeps between retries when the outbound queue is full, before
/// re-checking whether the utterance was cancelled. Bounds how late a barge-in can be noticed
/// while backpressured; sized well under `latency_budget::BARGE_IN_YIELD_BUDGET_MS` (100ms).
const CHUNK_QUEUE_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(10);
/// How long teardown waits for already-cancelled provider work to report what it cost.
const TEARDOWN_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(750);
/// Hard ceiling on a single provider call (an STT push/end-of-turn, or a TTS synthesis) dispatched
/// via `spawn_blocking`. Without this, a provider that hangs (network partition, stalled peer,
/// misbehaving sidecar) parks its blocking-pool thread indefinitely — one session's stuck call
/// never un-stalls, and enough stuck calls exhaust the fixed-size blocking pool shared by every
/// connection, starving sessions that were never near the bad provider. `tokio::time::timeout`
/// bounds the wait so the caller gets `ProviderError::TimedOut` and moves on; it does not (and
/// cannot) kill the blocking thread itself, only stop waiting on it.
const PROVIDER_TIMEOUT: Duration = Duration::from_secs(30);

fn as_object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

/// Compact, non-audio summary of what a frame produced — the audio bytes themselves are excluded
/// (dev-logs/ is a debugging aid, not a place to dump raw TTS chunks).
fn describe_frames(frames: &[ServerFrame]) -> Value {
    Value::Array(
        frames
            .iter()
            .map(|f| match f {
                ServerFrame::ListeningConfirmed { .. } => json!({"type": "listening_confirmed"}),
                ServerFrame::Transcript { text, is_final, .. } => json!({
                    "type": "transcript",
                    "is_final": is_final,
                    "text": devlog::truncate(text, 200),
                }),
                ServerFrame::SpeechStarting { .. } => json!({"type": "speech_starting"}),
                ServerFrame::SpeechComplete { .. } => json!({"type": "speech_complete"}),
                ServerFrame::Closing { reason, .. } => {
                    json!({"type": "closing", "reason": format!("{reason:?}")})
                }
                ServerFrame::AudioBedFallback { .. } => json!({"type": "audio_bed_fallback"}),
            })
            .collect(),
    )
}

fn describe_action(action: &Action) -> Value {
    match action {
        Action::Send(frames) => json!({"kind": "send", "frames": describe_frames(frames)}),
        Action::DispatchAudio { session_id } => {
            json!({"kind": "dispatch_audio", "session_id": session_id})
        }
        Action::DispatchEndOfTurn { session_id } => {
            json!({"kind": "dispatch_end_of_turn", "session_id": session_id})
        }
        Action::BeginSpeech(request) => json!({
            "kind": "begin_speech",
            "generation": request.generation,
            "text": devlog::truncate(&request.text, 200),
            "text_chars": request.text.chars().count(),
            "voice_id": request.voice_id,
            "emotion": request.emotion,
        }),
        Action::YieldSpeech(frames) => {
            json!({"kind": "yield_speech", "frames": describe_frames(frames)})
        }
        Action::SendAndClose(frames) => {
            json!({"kind": "send_and_close", "frames": describe_frames(frames)})
        }
        Action::SendFallback(frames) => {
            json!({"kind": "send_fallback", "frames": describe_frames(frames)})
        }
        Action::Ignore => json!({"kind": "ignore"}),
    }
}

/// Emits one obvious, searchable event for every transcript returned by the STT provider. The
/// existing `frame.processed` event is useful for protocol debugging, but nesting the transcript
/// inside `out.frames` made it too easy to miss the exact words STT produced. Audio is represented
/// only by counts/bytes; raw microphone data never enters dev-logs.
fn log_stt_transcripts(
    action: &Action,
    provider: &str,
    audio_frames_received: u64,
    audio_bytes_received: u64,
    processing_latency_ms: f64,
) {
    let frames = match action {
        Action::Send(frames) | Action::SendAndClose(frames) | Action::YieldSpeech(frames) => frames,
        Action::SendFallback(frames) => frames,
        Action::DispatchAudio { .. }
        | Action::DispatchEndOfTurn { .. }
        | Action::BeginSpeech(_)
        | Action::Ignore => return,
    };
    for frame in frames {
        let ServerFrame::Transcript {
            tenant_id,
            session_id,
            text,
            is_final,
        } = frame
        else {
            continue;
        };
        devlog::dev_log(
            "stt.transcript",
            "info",
            as_object(json!({
                "provider": provider,
                "tenant_id": tenant_id,
                "session_id": session_id,
                "is_final": is_final,
                "transcript_kind": if *is_final { "final" } else { "partial" },
                "text": devlog::truncate(text, 500),
                "text_chars": text.chars().count(),
                "audio_frames_received": audio_frames_received,
                "audio_bytes_received": audio_bytes_received,
                "processing_latency_ms": processing_latency_ms,
            })),
        );
    }
}

/// Compact summary of the inbound client frame — mirrors `describe_frames` above but for the
/// client -> relay direction, so dev-logs/ shows both sides of every hop.
fn describe_client_frame(frame: &ClientFrame) -> Value {
    match frame {
        ClientFrame::StartListening {
            tenant_id,
            session_id,
        } => json!({"type": "start_listening", "tenant_id": tenant_id, "session_id": session_id}),
        ClientFrame::EndOfTurn {
            tenant_id,
            session_id,
        } => json!({"type": "end_of_turn", "tenant_id": tenant_id, "session_id": session_id}),
        ClientFrame::Speak {
            tenant_id,
            session_id,
            text,
            voice_id,
            emotion,
        } => json!({
            "type": "speak",
            "tenant_id": tenant_id,
            "session_id": session_id,
            "text": devlog::truncate(text, 200),
            "voice_id": voice_id,
            "emotion": emotion,
        }),
        ClientFrame::BargeIn {
            tenant_id,
            session_id,
        } => json!({"type": "barge_in", "tenant_id": tenant_id, "session_id": session_id}),
        ClientFrame::Pause {
            tenant_id,
            session_id,
        } => json!({"type": "pause", "tenant_id": tenant_id, "session_id": session_id}),
    }
}

// ---------------------------------------------------------------------------------------------
// Outbound plumbing
// ---------------------------------------------------------------------------------------------

/// Anything queued for the socket.
enum Outbound {
    Frame {
        /// `None` for frames that must always be delivered. `Some(g)` for anything belonging to
        /// speech generation `g` — dropped by the writer once `g` is no longer the live one, which
        /// is what stops a barge-in from being followed by the audio it just cancelled.
        generation: Option<u64>,
        message: Message,
    },
    /// Flush what is queued, send a close frame, and stop.
    Close,
}

/// Counters the writer keeps so a barge-in leaves proof in dev-logs/ that audio was actually
/// suppressed, rather than only that a cancel was requested.
#[derive(Default)]
struct DropCounters {
    frames: AtomicU64,
    bytes: AtomicU64,
}

/// Owns the sink. The only place a byte reaches the socket, and therefore the only place that can
/// guarantee a retired generation's audio never does.
async fn write_loop<S>(
    mut sink: S,
    mut rx: mpsc::Receiver<Outbound>,
    live_generation: Arc<AtomicU64>,
    dropped: Arc<DropCounters>,
) where
    // Generic over the sink so the drop rule can be tested against a recording sink rather than
    // only through a live socket, where loopback delivers a whole burst before an interrupt can
    // physically arrive and the rule is therefore never exercised.
    S: futures_util::Sink<Message> + Unpin,
{
    // Dropped chunks are accumulated per generation and logged as one event rather than one per
    // chunk: a cancelled utterance is hundreds of chunks, and a line each would bury the signal.
    let mut pending: Option<(u64, u64, u64)> = None;
    fn flush(pending: &mut Option<(u64, u64, u64)>) {
        if let Some((generation, frames, bytes)) = pending.take() {
            devlog::dev_log(
                "speech.audio_dropped",
                "info",
                as_object(json!({
                    "generation": generation,
                    "dropped_frames": frames,
                    "dropped_bytes": bytes,
                    "reason": "generation_retired_by_barge_in_or_pause",
                })),
            );
        }
    }
    while let Some(item) = rx.recv().await {
        match item {
            Outbound::Frame {
                generation: Some(generation),
                message,
            } if live_generation.load(Ordering::Acquire) != generation => {
                let bytes = message.len() as u64;
                dropped.frames.fetch_add(1, Ordering::Relaxed);
                dropped.bytes.fetch_add(bytes, Ordering::Relaxed);
                match &mut pending {
                    Some((pending_generation, frames, dropped_bytes))
                        if *pending_generation == generation =>
                    {
                        *frames += 1;
                        *dropped_bytes += bytes;
                    }
                    _ => {
                        flush(&mut pending);
                        pending = Some((generation, 1, bytes));
                    }
                }
            }
            Outbound::Frame { message, .. } => {
                flush(&mut pending);
                if sink.send(message).await.is_err() {
                    break;
                }
            }
            Outbound::Close => {
                flush(&mut pending);
                let _ = sink.send(Message::Close(None)).await;
                break;
            }
        }
    }
    flush(&mut pending);
    let _ = sink.close().await;
}

// ---------------------------------------------------------------------------------------------
// STT worker
// ---------------------------------------------------------------------------------------------

struct SttJob {
    session_id: String,
    kind: SttJobKind,
}

enum SttJobKind {
    Push(Vec<u8>),
    EndOfTurn,
}

/// Runs STT calls sequentially, off the read loop. Sequential on purpose: transcripts are a
/// stream and reordering them would corrupt the turn. The read loop never waits on this.
async fn stt_loop(
    providers: Arc<dyn ProviderAdapter>,
    mut rx: mpsc::Receiver<SttJob>,
    outcome_tx: mpsc::Sender<Outcome>,
    cancel: CancelFlag,
    provider_timeout: Duration,
) {
    while let Some(job) = rx.recv().await {
        let is_end_of_turn = matches!(job.kind, SttJobKind::EndOfTurn);
        let providers = Arc::clone(&providers);
        let cancel = cancel.clone();
        // spawn_blocking: the provider's HTTP call is synchronous. Running it on a runtime worker
        // thread would park that worker — and with it every other connection sharing it.
        let handle = tokio::task::spawn_blocking(move || match job.kind {
            SttJobKind::Push(frame) => SttOutcome::Push(
                providers
                    .push_audio(&job.session_id, &frame, &cancel)
                    .map_err(SttFailure::from),
            ),
            SttJobKind::EndOfTurn => SttOutcome::EndOfTurn(
                providers
                    .end_of_turn(&job.session_id, &cancel)
                    .map_err(SttFailure::from),
            ),
        });
        // Bounded wait: a provider that never returns must not park this loop (and with it every
        // job queued behind it, including this session's own barge-in-adjacent control frames)
        // forever. The blocking thread itself keeps running detached — there is no way to kill a
        // synchronous OS thread from here — but this loop stops waiting on it and moves on.
        let joined = match tokio::time::timeout(provider_timeout, handle).await {
            Ok(joined) => joined,
            Err(_elapsed) => {
                let failure = SttFailure::from(ProviderError::TimedOut);
                Ok(if is_end_of_turn {
                    SttOutcome::EndOfTurn(Err(failure))
                } else {
                    SttOutcome::Push(Err(failure))
                })
            }
        };
        let outcome = match joined {
            Ok(outcome) => outcome,
            // A panic inside the provider is an outage, not silence: surface it as one so the
            // session closes with `provider_failure` instead of going quietly deaf.
            Err(err) => {
                let failure = SttFailure::Unavailable(format!("stt worker panicked: {err}"));
                if is_end_of_turn {
                    SttOutcome::EndOfTurn(Err(failure))
                } else {
                    SttOutcome::Push(Err(failure))
                }
            }
        };
        if outcome_tx.send(Outcome::Stt(outcome)).await.is_err() {
            break;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Speech task
// ---------------------------------------------------------------------------------------------

/// Clock readings for one utterance. Kept here rather than in `SpeechOutcome` so `session.rs`
/// stays free of wall-clock reads (AGENTS.md determinism invariant).
struct SpeechTelemetry {
    generation: u64,
    /// When the client's `end_of_turn` arrived, if this speech answers one.
    user_stopped_at: Option<Instant>,
    /// When the `speak` frame was read off the socket.
    speak_received_at: Instant,
    /// When the FIRST audio chunk was queued for the socket. `None` if none ever was.
    first_chunk_at: Option<Instant>,
    audio_chunks: usize,
    audio_bytes: usize,
    cancelled: bool,
}

/// Work the read loop dispatched, coming back to it. The read loop is the only owner of `Session`,
/// so every asynchronous result has to return here to move phase or usage.
enum Outcome {
    Stt(SttOutcome),
    Speech {
        outcome: SpeechOutcome,
        telemetry: SpeechTelemetry,
    },
}

/// A live utterance the read loop can cancel. Dropping the handle does not cancel — `cancel()`
/// does, and the task is deliberately never awaited by the read loop (awaiting it would reintroduce
/// exactly the blocking this whole design removes).
struct SpeechHandle {
    generation: u64,
    cancel: CancelFlag,
}

/// What the (synchronous, `spawn_blocking`) `on_chunk` callback has observed so far. Shared with
/// the async task via a mutex rather than a channel: the callback runs on the blocking thread and
/// must never `.await`, so it updates this directly and the async task reads it back once
/// `synthesize()` returns (or the whole point — sends the audio itself — would still have already
/// happened by then, since that's on the outbound channel, not in here).
#[derive(Default)]
struct ChunkProgress {
    first_chunk_at: Option<Instant>,
    audio_chunks: usize,
    audio_bytes: usize,
    /// Set when the outbound writer is gone (connection tearing down). Distinguished from a
    /// user-initiated cancel: on this path there is no one left to report an outcome to.
    writer_gone: bool,
}

#[allow(clippy::too_many_arguments)]
fn spawn_speech(
    providers: Arc<dyn ProviderAdapter>,
    request: SpeechRequest,
    outbound_tx: mpsc::Sender<Outbound>,
    outcome_tx: mpsc::Sender<Outcome>,
    cancel: CancelFlag,
    user_stopped_at: Option<Instant>,
    speak_received_at: Instant,
    provider_timeout: Duration,
) {
    tokio::spawn(async move {
        let generation = request.generation;
        let chars = request.text.chars().count() as u64;
        let mut telemetry = SpeechTelemetry {
            generation,
            user_stopped_at,
            speak_received_at,
            first_chunk_at: None,
            audio_chunks: 0,
            audio_bytes: 0,
            cancelled: false,
        };

        // Checked here, before dispatch, so "cancelled before the provider was ever asked" is
        // distinguishable from "cancelled with the request already on the wire". Only the second
        // is billable, and only this ordering can tell them apart.
        if cancel.is_cancelled() {
            telemetry.cancelled = true;
            let _ = outcome_tx
                .send(Outcome::Speech {
                    outcome: SpeechOutcome::Cancelled {
                        generation,
                        chars: 0,
                    },
                    telemetry,
                })
                .await;
            return;
        }

        let progress = Arc::new(Mutex::new(ChunkProgress::default()));

        let synthesis = {
            let providers = Arc::clone(&providers);
            let cancel = cancel.clone();
            let text = request.text.clone();
            let voice_id = request.voice_id.clone();
            let emotion = request.emotion.clone();
            let outbound_tx = outbound_tx.clone();
            let progress = Arc::clone(&progress);
            let handle = tokio::task::spawn_blocking(move || {
                // Runs synchronously on the blocking thread, once per chunk, as the provider
                // produces it — this is the fix: the chunk reaches the client's outbound queue
                // immediately rather than waiting for every other chunk to exist first.
                let chunk_cancel = cancel.clone();
                let mut on_chunk = move |chunk: Vec<u8>| -> Result<(), ProviderError> {
                    let bytes = chunk.len();
                    let mut item = Outbound::Frame {
                        generation: Some(generation),
                        message: Message::Binary(chunk),
                    };
                    // Polled retry rather than a single `blocking_send`: a full outbound queue
                    // (2.5s of buffered audio) must not hold a cancelled utterance's synthesis
                    // open until the client drains it. This is the same poll-and-recheck shape
                    // `HttpContractProvider` already uses against `CANCEL_POLL_INTERVAL` while
                    // parked on a blocking socket read.
                    loop {
                        if chunk_cancel.is_cancelled() {
                            return Err(ProviderError::Cancelled);
                        }
                        match outbound_tx.try_send(item) {
                            Ok(()) => break,
                            Err(mpsc::error::TrySendError::Full(returned)) => {
                                item = returned;
                                std::thread::sleep(CHUNK_QUEUE_POLL_INTERVAL);
                            }
                            // Writer gone: the connection is closing. Nothing to report to a
                            // loop that is already tearing down — stop the provider rather than
                            // let it keep producing (and this relay keep billing) audio nobody
                            // can receive.
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                progress.lock().expect("mutex not poisoned").writer_gone = true;
                                return Err(ProviderError::Cancelled);
                            }
                        }
                    }
                    let mut progress = progress.lock().expect("mutex not poisoned");
                    if progress.first_chunk_at.is_none() {
                        progress.first_chunk_at = Some(Instant::now());
                    }
                    progress.audio_chunks += 1;
                    progress.audio_bytes += bytes;
                    Ok(())
                };
                providers.synthesize(&text, &voice_id, &emotion, &cancel, &mut on_chunk)
            });
            // Bounded wait: see `stt_loop`'s identical wrapper. A synthesis that never returns
            // must not hold this task (and the generation it owns) open forever; the blocking
            // thread itself keeps running detached (still streaming chunks via on_chunk above
            // until it notices `cancel`), but this task stops waiting on it.
            match tokio::time::timeout(provider_timeout, handle).await {
                Ok(joined) => joined,
                Err(_elapsed) => Ok(Err(ProviderError::TimedOut)),
            }
        };

        {
            let progress = progress.lock().expect("mutex not poisoned");
            telemetry.first_chunk_at = progress.first_chunk_at;
            telemetry.audio_chunks = progress.audio_chunks;
            telemetry.audio_bytes = progress.audio_bytes;
            if progress.writer_gone {
                return;
            }
        }

        match synthesis {
            Ok(Ok(())) => {
                // Tagged with the generation like the audio itself, so an interrupted utterance
                // cannot tell the client "speech complete" a second time after a barge-in already
                // did.
                let _ = outbound_tx
                    .send(Outbound::Frame {
                        generation: Some(generation),
                        message: match serde_json::to_string(&request.complete) {
                            Ok(text) => Message::Text(text),
                            Err(_) => return,
                        },
                    })
                    .await;
                let outcome = SpeechOutcome::Delivered { generation, chars };
                let _ = outcome_tx
                    .send(Outcome::Speech { outcome, telemetry })
                    .await;
            }
            // Covers both "cancelled before the provider replied at all" and "cancelled between
            // two chunks" — the request had already been dispatched (the check above proved the
            // flag was clear when we started), so the provider may well have begun — and charged
            // for — this utterance. Bill it rather than under-report cost.
            Ok(Err(ProviderError::Cancelled)) => {
                telemetry.cancelled = true;
                let _ = outcome_tx
                    .send(Outcome::Speech {
                        outcome: SpeechOutcome::Cancelled { generation, chars },
                        telemetry,
                    })
                    .await;
            }
            Ok(Err(ProviderError::Unavailable(detail))) => {
                let _ = outcome_tx
                    .send(Outcome::Speech {
                        outcome: SpeechOutcome::Failed { generation, detail },
                        telemetry,
                    })
                    .await;
            }
            Ok(Err(ProviderError::TimedOut)) => {
                let _ = outcome_tx
                    .send(Outcome::Speech {
                        outcome: SpeechOutcome::Failed {
                            generation,
                            detail: "provider timed out".into(),
                        },
                        telemetry,
                    })
                    .await;
                return;
            }
            Err(err) => {
                let _ = outcome_tx
                    .send(Outcome::Speech {
                        outcome: SpeechOutcome::Failed {
                            generation,
                            detail: format!("tts worker panicked: {err}"),
                        },
                        telemetry,
                    })
                    .await;
            }
        }
    });
}

// ---------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr: SocketAddr = std::env::var("ORB_RELAY_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8091".to_string())
        .parse()?;
    let provider_config =
        ProviderConfig::from_env().map_err(|err| format!("invalid provider config: {err:?}"))?;
    if matches!(
        provider_config.backend,
        ProviderBackend::External | ProviderBackend::Http
    ) {
        provider_from_config(provider_config.clone(), None)
            .map_err(|err| format!("provider unavailable: {err:?}"))?;
    }
    let listener = TcpListener::bind(addr).await?;
    println!("orb-relay-rs listening on {addr}");
    devlog::dev_log(
        "latency.budgets",
        "info",
        as_object(json!({
            "first_audio_ms": hot_path_budget_ms(HotPathBudget::FirstAudio),
            "presence_cover_ms": hot_path_budget_ms(HotPathBudget::ThinkTimeCover),
            "barge_in_yield_ms": hot_path_budget_ms(HotPathBudget::BargeInYield),
            "barge_in_qualification_ms": hot_path_budget_ms(HotPathBudget::BargeInSpeechQualification),
        })),
    );

    loop {
        let (stream, _peer) = listener.accept().await?;
        let providers = provider_from_config(provider_config.clone(), None)
            .map_err(|err| format!("provider unavailable: {err:?}"))?;
        println!(
            "accepted realtime connection with provider={}",
            providers.label()
        );
        devlog::dev_log(
            "connection.accepted",
            "info",
            as_object(json!({"provider": providers.label()})),
        );
        tokio::spawn(async move {
            if let Err(err) = serve_connection(stream, providers, PROVIDER_TIMEOUT).await {
                eprintln!("connection ended: {err}");
                devlog::dev_log(
                    "connection.error",
                    "error",
                    as_object(json!({"error": err.to_string()})),
                );
            }
        });
    }
}

/// Everything the read loop needs to reach its helpers, so `execute` does not take nine arguments.
struct Connection {
    providers: Arc<dyn ProviderAdapter>,
    outbound_tx: mpsc::Sender<Outbound>,
    outcome_tx: mpsc::Sender<Outcome>,
    stt_tx: mpsc::Sender<SttJob>,
    live_generation: Arc<AtomicU64>,
    dropped: Arc<DropCounters>,
    /// Cancels every provider call still outstanding when the connection tears down.
    connection_cancel: CancelFlag,
    speech: Option<SpeechHandle>,
    stt_frames_dropped: u64,
    /// Ceiling for the blocking TTS call `spawn_speech` dispatches. Same value `stt_loop` uses for
    /// its own provider calls (`PROVIDER_TIMEOUT` in production); threaded as a field rather than a
    /// bare `PROVIDER_TIMEOUT` reference at the call site so tests can shorten it.
    provider_timeout: Duration,
}

impl Connection {
    async fn send_frame(&self, frame: &ServerFrame) -> ConnResult<()> {
        self.outbound_tx
            .send(Outbound::Frame {
                generation: None,
                message: Message::Text(serde_json::to_string(frame)?),
            })
            .await
            .map_err(|_| "outbound writer stopped")?;
        Ok(())
    }

    /// Retires the live generation: nothing it has queued or will queue reaches the socket, and
    /// its provider call is told to stop waiting. Deliberately does NOT await the speech task —
    /// awaiting it here would park the read loop on the very call we are cancelling.
    fn retire_live_speech(&mut self) -> Option<u64> {
        self.live_generation
            .store(NO_LIVE_GENERATION, Ordering::Release);
        let handle = self.speech.take()?;
        handle.cancel.cancel();
        Some(handle.generation)
    }
}

async fn serve_connection(
    stream: TcpStream,
    providers: Arc<dyn ProviderAdapter>,
    provider_timeout: Duration,
) -> ConnResult<()> {
    let ws = tokio_tungstenite::accept_async(stream).await?;
    let (sink, mut source): (WsSink, WsSource) = ws.split();

    let live_generation = Arc::new(AtomicU64::new(NO_LIVE_GENERATION));
    let dropped = Arc::new(DropCounters::default());
    let (outbound_tx, outbound_rx) = mpsc::channel::<Outbound>(OUTBOUND_QUEUE);
    let writer = tokio::spawn(write_loop(
        sink,
        outbound_rx,
        Arc::clone(&live_generation),
        Arc::clone(&dropped),
    ));

    let (outcome_tx, mut outcome_rx) = mpsc::channel::<Outcome>(OUTCOME_QUEUE);
    let (stt_tx, stt_rx) = mpsc::channel::<SttJob>(STT_QUEUE);
    let connection_cancel = CancelFlag::new();
    let stt_worker = tokio::spawn(stt_loop(
        Arc::clone(&providers),
        stt_rx,
        outcome_tx.clone(),
        connection_cancel.clone(),
        provider_timeout,
    ));

    let mut connection = Connection {
        providers: Arc::clone(&providers),
        outbound_tx,
        outcome_tx,
        stt_tx,
        live_generation,
        dropped: Arc::clone(&dropped),
        connection_cancel: connection_cancel.clone(),
        speech: None,
        stt_frames_dropped: 0,
        provider_timeout,
    };

    let mut session = Session::new();
    let mut audio_frames_received = 0_u64;
    let mut audio_bytes_received = 0_u64;
    let mut user_stopped_at: Option<Instant> = None;
    let provider_label = providers.label();
    let mut result: ConnResult<()> = Ok(());

    'connection: loop {
        tokio::select! {
            // Biased so a completed provider result is folded in before the next inbound frame is
            // read. Without it a burst of mic frames can starve the outcome branch, and the
            // session's phase would lag behind the work that has actually finished.
            biased;
            Some(outcome) = outcome_rx.recv() => {
                match handle_outcome(
                    &mut session,
                    &mut connection,
                    outcome,
                    provider_label,
                    audio_frames_received,
                    audio_bytes_received,
                )
                .await
                {
                    Ok(true) => break 'connection,
                    Ok(false) => {}
                    Err(err) => { result = Err(err); break 'connection; }
                }
            }
            message = source.next() => {
                let Some(message) = message else { break 'connection };
                let message = match message {
                    Ok(message) => message,
                    Err(err) => { result = Err(Box::new(err)); break 'connection; }
                };
                match handle_inbound(
                    &mut session,
                    &mut connection,
                    message,
                    &mut user_stopped_at,
                    &mut audio_frames_received,
                    &mut audio_bytes_received,
                    provider_label,
                )
                .await
                {
                    Ok(true) => break 'connection,
                    Ok(false) => {}
                    Err(err) => { result = Err(err); break 'connection; }
                }
            }
            else => break 'connection,
        }
    }

    // Teardown: stop every outstanding provider call, then let the writer flush what is already
    // queued. `connection_cancel` covers the STT worker; `retire_live_speech` covers synthesis.
    connection.retire_live_speech();
    connection_cancel.cancel();
    let Connection {
        outbound_tx,
        outcome_tx,
        stt_tx,
        stt_frames_dropped,
        ..
    } = connection;
    drop(stt_tx);
    drop(outcome_tx);
    drop(outbound_tx);

    // Fold in whatever the cancelled workers report before the counters are read. A client that
    // disconnects mid-utterance still cost real provider characters, and without this drain those
    // characters are silently lost — the loop has already exited, so their outcome would never
    // reach the session and `session.ended` would under-report the bill (C12).
    //
    // Bounded: everything outstanding has just been cancelled, and a cancelled provider call
    // returns within `provider::CANCEL_POLL_INTERVAL`. The timeout exists so a wedged provider
    // delays a closed connection's bookkeeping rather than leaking the task forever.
    let drained = tokio::time::timeout(TEARDOWN_DRAIN_TIMEOUT, async {
        while let Some(outcome) = outcome_rx.recv().await {
            match outcome {
                Outcome::Speech { outcome, .. } => {
                    session.on_speech_outcome(outcome);
                }
                Outcome::Stt(outcome) => {
                    session.on_stt_outcome(outcome);
                }
            }
        }
    })
    .await;
    if drained.is_err() {
        devlog::dev_log(
            "session.drain_timeout",
            "warn",
            as_object(json!({
                "timeout_ms": TEARDOWN_DRAIN_TIMEOUT.as_millis() as u64,
                "consequence": "usage counters may under-report this session's provider cost",
            })),
        );
    }
    let _ = stt_worker.await;
    let _ = writer.await;

    // Usage is drained here and reported to relay-py out-of-band — never a synchronous call on the
    // audio path (0004). Wiring the reporter is the cost-plane slice; the counters are already
    // accurate, so nothing is lost by that landing separately.
    println!(
        "session ended: stt_frames={} tts_chars={}",
        session.usage.stt_frames, session.usage.tts_chars
    );
    devlog::dev_log(
        "session.ended",
        "info",
        as_object(json!({
            "stt_frames": session.usage.stt_frames,
            "tts_chars": session.usage.tts_chars,
            "stale_audio_frames_dropped": dropped.frames.load(Ordering::Acquire),
            "stale_audio_bytes_dropped": dropped.bytes.load(Ordering::Acquire),
            "mic_frames_dropped_stt_backlog": stt_frames_dropped,
        })),
    );
    result
}

/// Handles one inbound WebSocket message. Returns `Ok(true)` when the connection should close.
#[allow(clippy::too_many_arguments)]
async fn handle_inbound(
    session: &mut Session,
    connection: &mut Connection,
    message: Message,
    user_stopped_at: &mut Option<Instant>,
    audio_frames_received: &mut u64,
    audio_bytes_received: &mut u64,
    provider_label: &str,
) -> ConnResult<bool> {
    let frame_started = Instant::now();
    let parsed_control = match &message {
        Message::Text(text) => serde_json::from_str::<ClientFrame>(text).ok(),
        _ => None,
    };
    let is_end_of_turn = matches!(parsed_control.as_ref(), Some(ClientFrame::EndOfTurn { .. }));
    let is_barge_in = matches!(parsed_control.as_ref(), Some(ClientFrame::BargeIn { .. }));
    let is_speak = matches!(parsed_control.as_ref(), Some(ClientFrame::Speak { .. }));
    if is_end_of_turn {
        *user_stopped_at = Some(frame_started);
        devlog::dev_log(
            "presence.cover_required",
            "info",
            as_object(json!({
                "budget_ms": hot_path_budget_ms(HotPathBudget::ThinkTimeCover),
                "guarantee_owner": "mobile_on_device_presence",
                "measurement_scope": "control_frame_not_rendered_audio",
            })),
        );
    }
    let content_request_latency_ms = if is_speak {
        user_stopped_at.map(|started| started.elapsed().as_secs_f64() * 1000.0)
    } else {
        None
    };
    let inbound_kind = match &message {
        Message::Binary(audio) => json!({"type": "mic_audio", "bytes": audio.len()}),
        Message::Text(_) => parsed_control
            .as_ref()
            .map(describe_client_frame)
            .unwrap_or_else(|| json!({"type": "unparseable_text"})),
        Message::Close(_) => json!({"type": "close"}),
        _ => json!({"type": "other"}),
    };

    let mut audio_frame: Option<Vec<u8>> = None;
    let action = match message {
        Message::Binary(audio) => {
            *audio_frames_received += 1;
            *audio_bytes_received += audio.len() as u64;
            audio_frame = Some(audio);
            session.on_audio_frame()
        }
        Message::Text(text) => match serde_json::from_str::<ClientFrame>(&text) {
            Ok(frame) => session.handle_client_frame(frame),
            // An unparseable control frame is a client bug, not a session failure: report it
            // and keep the session (and therefore the user's audio bed) alive.
            Err(err) => {
                eprintln!("dropping unparseable client frame: {err}");
                devlog::dev_log(
                    "frame.unparseable",
                    "warn",
                    as_object(json!({"error": err.to_string()})),
                );
                Action::Send(vec![])
            }
        },
        Message::Close(_) => return Ok(true),
        _ => Action::Send(vec![]),
    };

    // Skip the noisy 20-40ms-cadence mic_audio frames unless they actually produced something
    // other than a plain dispatch — otherwise dev-logs/relay-rs.ndjson would be almost entirely
    // per-frame acks and the signal (transcripts, speak, barge-in, failures) would be buried.
    let is_silent_audio_ack = inbound_kind["type"] == "mic_audio"
        && matches!(&action, Action::DispatchAudio { .. } | Action::Ignore);

    if is_speak {
        devlog::dev_log(
            "latency.content_request_accepted",
            "info",
            as_object(json!({
                "content_request_latency_ms": content_request_latency_ms,
                "presence_cover_budget_ms": hot_path_budget_ms(HotPathBudget::ThinkTimeCover),
                "measurement_scope": "speak_frame_accepted_not_audio_ready",
            })),
        );
    }

    let close_after = matches!(&action, Action::SendAndClose(_));
    let speak_received_at = frame_started;
    let stopped_at = *user_stopped_at;
    if is_speak {
        *user_stopped_at = None;
    }

    execute(
        session,
        connection,
        action,
        audio_frame,
        stopped_at,
        speak_received_at,
        provider_label,
        *audio_frames_received,
        *audio_bytes_received,
        &inbound_kind,
        is_silent_audio_ack,
        frame_started,
    )
    .await?;

    if is_barge_in {
        // The number the 100ms budget is actually about: how long the relay took, from reading the
        // frame, to having yielded the floor — cancelled the generation, stopped its audio, and
        // queued the acknowledgement. Under the old sequential loop this could not be logged at
        // all, because the frame was not read until the utterance had already finished.
        let observed_ms = frame_started.elapsed().as_millis();
        let observed_ms = u64::try_from(observed_ms).unwrap_or(u64::MAX);
        devlog::dev_log(
            "latency.barge_in_yield",
            if hot_path_within_budget(HotPathBudget::BargeInYield, observed_ms) {
                "info"
            } else {
                "warn"
            },
            as_object(json!({
                "yield_latency_ms": frame_started.elapsed().as_secs_f64() * 1000.0,
                "budget_ms": hot_path_budget_ms(HotPathBudget::BargeInYield),
                "within_budget": hot_path_within_budget(HotPathBudget::BargeInYield, observed_ms),
                "stale_audio_frames_dropped_total": connection.dropped.frames.load(Ordering::Acquire),
                "stale_audio_bytes_dropped_total": connection.dropped.bytes.load(Ordering::Acquire),
                "measurement_scope": "relay_read_to_yield_executed_not_device_silence",
            })),
        );
    }

    Ok(close_after)
}

/// Handles one completed piece of off-loop work. Returns `Ok(true)` when the connection should
/// close.
async fn handle_outcome(
    session: &mut Session,
    connection: &mut Connection,
    outcome: Outcome,
    provider_label: &str,
    audio_frames_received: u64,
    audio_bytes_received: u64,
) -> ConnResult<bool> {
    let started = Instant::now();
    let (action, inbound_kind) = match outcome {
        Outcome::Stt(outcome) => {
            let kind = match &outcome {
                SttOutcome::Push(_) => json!({"type": "stt_push_result"}),
                SttOutcome::EndOfTurn(_) => json!({"type": "stt_end_of_turn_result"}),
            };
            (session.on_stt_outcome(outcome), kind)
        }
        Outcome::Speech { outcome, telemetry } => {
            log_speech_outcome(&outcome, &telemetry, provider_label);
            (
                session.on_speech_outcome(outcome),
                json!({"type": "speech_outcome", "generation": telemetry.generation}),
            )
        }
    };
    let close_after = matches!(&action, Action::SendAndClose(_));
    execute(
        session,
        connection,
        action,
        None,
        None,
        started,
        provider_label,
        audio_frames_received,
        audio_bytes_received,
        &inbound_kind,
        false,
        started,
    )
    .await?;
    Ok(close_after)
}

/// The subset of `log_speech_outcome`'s devlog fields worth asserting on in a unit test,
/// pulled out of the devlog-writing function so it's callable without real file I/O.
struct SpeechOutcomeFields {
    audio_synthetic: bool,
}

fn speech_outcome_fields(
    _outcome: &SpeechOutcome,
    _telemetry: &SpeechTelemetry,
    provider_label: &str,
) -> SpeechOutcomeFields {
    SpeechOutcomeFields {
        // Explicit, machine-checkable marker (not just a human reading the provider label)
        // that any audio bytes reported are synthetic fixture data, never real speech.
        audio_synthetic: provider_label.starts_with("fake"),
    }
}

fn log_speech_outcome(outcome: &SpeechOutcome, telemetry: &SpeechTelemetry, provider_label: &str) {
    let first_chunk_ready_ms = telemetry
        .first_chunk_at
        .map(|at| at.duration_since(telemetry.speak_received_at).as_secs_f64() * 1000.0);
    // The presence-cover question: from the user falling silent to the first audio byte actually
    // being queued for them. Only meaningful when this speech answers an end_of_turn.
    let content_first_chunk_ready_server_ms = match (telemetry.user_stopped_at, telemetry.first_chunk_at) {
        (Some(stopped), Some(first)) => Some(first.duration_since(stopped).as_secs_f64() * 1000.0),
        _ => None,
    };
    let observed_ms = content_first_chunk_ready_server_ms
        .map(|ms| ms as u64)
        .unwrap_or(u64::MAX);
    let (verdict, level) = match outcome {
        SpeechOutcome::Delivered { .. } if telemetry.audio_bytes > 0 => ("delivered", "info"),
        SpeechOutcome::Delivered { .. } => ("delivered_without_audio", "warn"),
        SpeechOutcome::Cancelled { .. } => ("cancelled", "info"),
        SpeechOutcome::Failed { .. } => ("failed", "error"),
    };
    devlog::dev_log(
        "latency.content_first_chunk_server",
        level,
        as_object(json!({
            "generation": telemetry.generation,
            "outcome": verdict,
            "provider": provider_label,
            // Explicit, machine-checkable marker (not just a human reading the provider label)
            // that any audio bytes reported above are synthetic fixture data, never real speech.
            "audio_synthetic": speech_outcome_fields(outcome, telemetry, provider_label).audio_synthetic,
            "audio_chunks_delivered": telemetry.audio_chunks,
            "audio_bytes_ready": telemetry.audio_bytes,
            "synthesis_to_first_chunk_ms": first_chunk_ready_ms,
            "content_first_chunk_ready_server_ms": content_first_chunk_ready_server_ms,
            "presence_cover_budget_ms": hot_path_budget_ms(HotPathBudget::ThinkTimeCover),
            "inside_presence_window_server_proxy": hot_path_within_budget(
                HotPathBudget::ThinkTimeCover,
                observed_ms,
            ),
            "deterministic_budget_verdict": format!("{:?}", classify(TurnKind::Deterministic, observed_ms)),
            "conversational_budget_verdict": format!("{:?}", classify(TurnKind::Conversational, observed_ms)),
            "measurement_scope": "server_first_chunk_queued_not_device_rendered_audio",
        })),
    );
}

/// Executes one `Action`. The single place an `Action` turns into I/O.
#[allow(clippy::too_many_arguments)]
async fn execute(
    session: &mut Session,
    connection: &mut Connection,
    action: Action,
    audio_frame: Option<Vec<u8>>,
    user_stopped_at: Option<Instant>,
    speak_received_at: Instant,
    provider_label: &str,
    audio_frames_received: u64,
    audio_bytes_received: u64,
    inbound_kind: &Value,
    is_silent_audio_ack: bool,
    frame_started: Instant,
) -> ConnResult<()> {
    let processing_latency_ms = frame_started.elapsed().as_secs_f64() * 1000.0;
    log_stt_transcripts(
        &action,
        provider_label,
        audio_frames_received,
        audio_bytes_received,
        processing_latency_ms,
    );
    if !is_silent_audio_ack {
        devlog::dev_log(
            "frame.processed",
            if matches!(&action, Action::SendAndClose(_)) {
                "warn"
            } else {
                "info"
            },
            as_object(json!({
                "in": inbound_kind,
                "out": describe_action(&action),
                "latency_ms": processing_latency_ms,
            })),
        );
    }

    match action {
        Action::Send(frames) => {
            for frame in &frames {
                connection.send_frame(frame).await?;
            }
        }
        Action::DispatchAudio { session_id } => {
            let Some(frame) = audio_frame else {
                return Ok(());
            };
            match connection.stt_tx.try_send(SttJob {
                session_id,
                kind: SttJobKind::Push(frame),
            }) {
                Ok(()) => {}
                // Dropping realtime audio is bad; blocking the read loop behind an STT backlog is
                // worse — that is precisely the failure this connection was restructured to remove.
                // So drop the frame, loudly, and keep control frames flowing.
                Err(mpsc::error::TrySendError::Full(_)) => {
                    connection.stt_frames_dropped += 1;
                    devlog::dev_log(
                        "stt.frame_dropped",
                        "warn",
                        as_object(json!({
                            "reason": "stt_backlog_full",
                            "queue_depth": STT_QUEUE,
                            "dropped_total": connection.stt_frames_dropped,
                        })),
                    );
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {}
            }
        }
        Action::DispatchEndOfTurn { session_id } => {
            // Unlike a mic frame, a finalise is not droppable: it is the frame the turn's answer
            // depends on, and it arrives once per turn rather than every 20ms.
            connection
                .stt_tx
                .send(SttJob {
                    session_id,
                    kind: SttJobKind::EndOfTurn,
                })
                .await
                .map_err(|_| "stt worker stopped")?;
        }
        Action::BeginSpeech(request) => {
            // A second `speak` while one is in flight supersedes it: retire the old generation
            // before the new one goes live so no byte of it can still be on the wire.
            connection.retire_live_speech();
            connection
                .live_generation
                .store(request.generation, Ordering::Release);
            // Untagged: the client is being told to duck the bed BEFORE synthesis begins, and it
            // needs the matching `speech_complete` (this utterance's, or a barge-in's) to restore.
            connection.send_frame(&request.starting).await?;
            let cancel = CancelFlag::new();
            connection.speech = Some(SpeechHandle {
                generation: request.generation,
                cancel: cancel.clone(),
            });
            spawn_speech(
                Arc::clone(&connection.providers),
                request,
                connection.outbound_tx.clone(),
                connection.outcome_tx.clone(),
                cancel,
                user_stopped_at,
                speak_received_at,
                connection.provider_timeout,
            );
        }
        Action::YieldSpeech(frames) => {
            // Cancel FIRST, then acknowledge. Reversed, the client is told the orb stopped talking
            // and then hears the rest of the sentence.
            let retired = connection.retire_live_speech();
            devlog::dev_log(
                "speech.yielded",
                "info",
                as_object(json!({
                    "retired_generation": retired,
                    "had_live_generation": retired.is_some(),
                })),
            );
            for frame in &frames {
                connection.send_frame(frame).await?;
            }
        }
        Action::SendAndClose(frames) => {
            connection.retire_live_speech();
            connection.connection_cancel.cancel();
            for frame in &frames {
                connection.send_frame(frame).await?;
            }
            let _ = connection.outbound_tx.send(Outbound::Close).await;
        }
        Action::SendFallback(frames) => {
            // Defect A4: provider failure must NOT close the socket. The session is degraded and
            // the client keeps the bed alive (§5). We cancel live speech (the provider that was
            // synthesising it failed), send the fallback frame, but keep the connection open so
            // the client can still pause or close explicitly.
            connection.retire_live_speech();
            for frame in &frames {
                connection.send_frame(frame).await?;
            }
            devlog::dev_log(
                "voice.degraded",
                "warn",
                as_object(json!({
                    "fallback": "audio_bed",
                    "socket": "kept_open",
                })),
            );
        }
        Action::Ignore => {}
    }
    let _ = session;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! End-to-end tests of the CONNECTION, not of the state machine.
    //!
    //! `session.rs` proves the decisions; these prove the plumbing that executes them, because the
    //! defect this file exists to fix lived entirely in the plumbing. Every test here drives a real
    //! WebSocket against a real `serve_connection` with a provider whose synthesis genuinely takes
    //! time, and asserts on bytes observed by a client — not on an `Action` value.

    use super::*;
    use crate::provider::{CancelSignal, SttProvider, Transcript, TtsProvider};
    use std::sync::atomic::AtomicUsize;
    use std::sync::Once;
    use std::time::Duration;
    use tokio_tungstenite::connect_async;

    /// dev-logs/ is a real corpus that `scripts/voice-ux-gate.mjs` reads and grades. A test run
    /// must never append synthetic rows to it.
    fn silence_dev_logs() {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            std::env::set_var("ORB_DEV_LOGGING", "0");
            std::env::set_var("ORB_DEV_LOG_DIR", std::env::temp_dir().join("orb-relay-rs-tests"));
        });
    }

    /// A provider whose synthesis takes real time and polls the cancel flag while it does — the
    /// shape of the live Fish path (seconds of nothing, then one whole payload).
    struct SlowTts {
        synthesis_delay: Duration,
        chunks: usize,
        dispatched: Arc<AtomicUsize>,
    }

    impl SlowTts {
        fn new(synthesis_delay: Duration, chunks: usize) -> Self {
            Self {
                synthesis_delay,
                chunks,
                dispatched: Arc::new(AtomicUsize::new(0)),
            }
        }

        /// Every chunk of an utterance is filled with the first byte of its text, so two
        /// utterances on one connection are distinguishable on the wire. Without this a test
        /// cannot tell "the live utterance arrived" from "the superseded one did".
        fn fill_byte(text: &str) -> u8 {
            text.as_bytes().first().copied().unwrap_or(0)
        }
    }

    impl TtsProvider for SlowTts {
        fn synthesize(
            &self,
            text: &str,
            _voice_id: &str,
            _emotion: &str,
            cancel: &dyn CancelSignal,
            on_chunk: &mut dyn FnMut(Vec<u8>) -> Result<(), ProviderError>,
        ) -> Result<(), ProviderError> {
            if cancel.is_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            self.dispatched.fetch_add(1, Ordering::SeqCst);
            let deadline = Instant::now() + self.synthesis_delay;
            while Instant::now() < deadline {
                if cancel.is_cancelled() {
                    return Err(ProviderError::Cancelled);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            if cancel.is_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            for _ in 0..self.chunks {
                on_chunk(vec![Self::fill_byte(text); 320])?;
            }
            Ok(())
        }
    }

    impl SttProvider for SlowTts {
        fn push_audio(
            &self,
            _session_id: &str,
            _frame: &[u8],
            _cancel: &dyn CancelSignal,
        ) -> Result<Option<Transcript>, ProviderError> {
            Ok(None)
        }

        fn end_of_turn(
            &self,
            _session_id: &str,
            _cancel: &dyn CancelSignal,
        ) -> Result<Transcript, ProviderError> {
            Ok(Transcript {
                text: "final".into(),
                is_final: true,
            })
        }
    }

    impl ProviderAdapter for SlowTts {
        fn label(&self) -> &'static str {
            "slow-test-tts"
        }
    }

    /// An STT provider that blocks for `delay` on every mic frame — the condition under which the
    /// old loop starved control frames, since the client sends ~200ms of audio before it qualifies
    /// a barge-in.
    struct SlowStt {
        push_delay: Duration,
    }

    impl SttProvider for SlowStt {
        fn push_audio(
            &self,
            _session_id: &str,
            _frame: &[u8],
            _cancel: &dyn CancelSignal,
        ) -> Result<Option<Transcript>, ProviderError> {
            std::thread::sleep(self.push_delay);
            Ok(None)
        }

        fn end_of_turn(
            &self,
            _session_id: &str,
            _cancel: &dyn CancelSignal,
        ) -> Result<Transcript, ProviderError> {
            Ok(Transcript {
                text: "final".into(),
                is_final: true,
            })
        }
    }

    impl TtsProvider for SlowStt {
        fn synthesize(
            &self,
            _text: &str,
            _voice_id: &str,
            _emotion: &str,
            _cancel: &dyn CancelSignal,
            on_chunk: &mut dyn FnMut(Vec<u8>) -> Result<(), ProviderError>,
        ) -> Result<(), ProviderError> {
            on_chunk(vec![9u8; 320])
        }
    }

    impl ProviderAdapter for SlowStt {
        fn label(&self) -> &'static str {
            "slow-test-stt"
        }
    }

    /// A provider whose synthesis genuinely trickles out — `chunk_delay` between each chunk
    /// rather than one hold before all of them (`SlowTts`'s shape). This is what a real streaming
    /// TTS provider looks like, and it is the fixture the true-streaming acceptance test drives:
    /// under the old "return the whole `Vec` at the end" trait, even a provider willing to stream
    /// would have its first chunk held hostage by its last one.
    struct StreamingTts {
        chunk_delay: Duration,
        chunks: usize,
    }

    impl StreamingTts {
        fn new(chunk_delay: Duration, chunks: usize) -> Self {
            Self { chunk_delay, chunks }
        }
    }

    impl TtsProvider for StreamingTts {
        fn synthesize(
            &self,
            _text: &str,
            _voice_id: &str,
            _emotion: &str,
            cancel: &dyn CancelSignal,
            on_chunk: &mut dyn FnMut(Vec<u8>) -> Result<(), ProviderError>,
        ) -> Result<(), ProviderError> {
            for i in 0..self.chunks {
                if cancel.is_cancelled() {
                    return Err(ProviderError::Cancelled);
                }
                std::thread::sleep(self.chunk_delay);
                on_chunk(vec![i as u8; 320])?;
            }
            Ok(())
        }
    }

    impl SttProvider for StreamingTts {
        fn push_audio(
            &self,
            _session_id: &str,
            _frame: &[u8],
            _cancel: &dyn CancelSignal,
        ) -> Result<Option<Transcript>, ProviderError> {
            Ok(None)
        }

        fn end_of_turn(
            &self,
            _session_id: &str,
            _cancel: &dyn CancelSignal,
        ) -> Result<Transcript, ProviderError> {
            Ok(Transcript {
                text: "final".into(),
                is_final: true,
            })
        }
    }

    impl ProviderAdapter for StreamingTts {
        fn label(&self) -> &'static str {
            "streaming-test-tts"
        }
    }

    async fn serve_one(provider: Arc<dyn ProviderAdapter>) -> String {
        serve_one_with_timeout(provider, PROVIDER_TIMEOUT).await
    }

    /// Like `serve_one`, but with an explicit provider-call ceiling — for tests that need a
    /// provider to actually trip the timeout without waiting out the real 30s production value.
    async fn serve_one_with_timeout(
        provider: Arc<dyn ProviderAdapter>,
        provider_timeout: Duration,
    ) -> String {
        silence_dev_logs();
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let addr = listener.local_addr().expect("test listener addr");
        tokio::spawn(async move {
            let (stream, _peer) = listener.accept().await.expect("accept test connection");
            let _ = serve_connection(stream, provider, provider_timeout).await;
        });
        format!("ws://{addr}")
    }

    /// Serves every accepted connection on one listener with the same provider/timeout — lets a
    /// test open a second, independent session against the same relay instance to prove a stuck
    /// first session does not stall it.
    async fn serve_many_with_timeout(
        provider: Arc<dyn ProviderAdapter>,
        provider_timeout: Duration,
    ) -> String {
        silence_dev_logs();
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let addr = listener.local_addr().expect("test listener addr");
        tokio::spawn(async move {
            loop {
                let (stream, _peer) = match listener.accept().await {
                    Ok(accepted) => accepted,
                    Err(_) => break,
                };
                let provider = Arc::clone(&provider);
                tokio::spawn(async move {
                    let _ = serve_connection(stream, provider, provider_timeout).await;
                });
            }
        });
        format!("ws://{addr}")
    }

    type Client = WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

    async fn connect(url: &str) -> Client {
        let (client, _response) = connect_async(url).await.expect("client connects");
        client
    }

    fn control(kind: &str) -> Message {
        Message::Text(format!(
            r#"{{"type":"{kind}","tenant_id":"t1","session_id":"s1"}}"#
        ))
    }

    fn speak(text: &str) -> Message {
        Message::Text(
            serde_json::to_string(&ClientFrame::Speak {
                tenant_id: "t1".into(),
                session_id: "s1".into(),
                text: text.into(),
                voice_id: "v".into(),
                emotion: "warm".into(),
            })
            .expect("speak frame serialises"),
        )
    }

    #[derive(Debug)]
    struct Observed {
        at: Instant,
        message: Message,
    }

    /// Has this turn's delivery actually started or finished? Only then does a quiet gap mean "the
    /// burst ended" rather than "synthesis has not answered yet". `speech_starting` means the
    /// opposite — audio is coming — and gating on it (as this function first did) turns the
    /// wait into a race against the synthesis delay. The e2e harness had the identical defect for
    /// the identical reason; both are fixed the same way.
    fn delivery_underway(seen: &[Observed]) -> bool {
        seen.iter().any(|o| match &o.message {
            Message::Binary(_) | Message::Close(_) => true,
            Message::Text(text) => {
                text.contains("\"type\":\"speech_complete\"")
                    || text.contains("\"type\":\"closing\"")
            }
            _ => false,
        })
    }

    /// Reads until delivery has begun or ended AND the socket has then been quiet for `quiet`, or
    /// `cap` elapses — the same rule the e2e harness uses, so a test failure and a harness failure
    /// mean the same thing.
    async fn drain(client: &mut Client, quiet: Duration, cap: Duration) -> Vec<Observed> {
        let deadline = Instant::now() + cap;
        let mut seen = Vec::new();
        let mut last_change = Instant::now();
        while Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(25), client.next()).await {
                Ok(Some(Ok(message))) => {
                    let closed = matches!(message, Message::Close(_));
                    seen.push(Observed {
                        at: Instant::now(),
                        message,
                    });
                    if closed {
                        break;
                    }
                    last_change = Instant::now();
                }
                Ok(Some(Err(_))) | Ok(None) => break,
                Err(_) => {
                    if delivery_underway(&seen) && last_change.elapsed() >= quiet {
                        break;
                    }
                }
            }
        }
        seen
    }

    fn audio_bytes(seen: &[Observed]) -> usize {
        seen.iter()
            .filter_map(|o| match &o.message {
                Message::Binary(bytes) => Some(bytes.len()),
                _ => None,
            })
            .sum()
    }

    fn first_binary_at(seen: &[Observed]) -> Option<Instant> {
        seen.iter().find_map(|o| match &o.message {
            Message::Binary(_) => Some(o.at),
            _ => None,
        })
    }

    fn first_text_of_type(seen: &[Observed], kind: &str) -> Option<Instant> {
        seen.iter().find_map(|o| match &o.message {
            Message::Text(text) if text.contains(&format!("\"type\":\"{kind}\"")) => Some(o.at),
            _ => None,
        })
    }

    fn count_text_of_type(seen: &[Observed], kind: &str) -> usize {
        seen.iter()
            .filter(|o| match &o.message {
                Message::Text(text) => text.contains(&format!("\"type\":\"{kind}\"")),
                _ => false,
            })
            .count()
    }

    /// A sink that records instead of transmitting, so the writer's drop rule can be asserted
    /// directly. Over loopback a whole TTS burst reaches the client's socket buffer before an
    /// interrupt can physically arrive, so the socket-level tests below never actually exercise
    /// this rule — mutation-testing the writer proved that gap rather than assuming it away.
    #[derive(Clone, Default)]
    struct RecordingSink {
        sent: Arc<std::sync::Mutex<Vec<Message>>>,
    }

    impl RecordingSink {
        fn messages(&self) -> Vec<Message> {
            self.sent.lock().expect("recording sink mutex").clone()
        }
    }

    impl futures_util::Sink<Message> for RecordingSink {
        type Error = std::convert::Infallible;

        fn poll_ready(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn start_send(self: std::pin::Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
            self.sent.lock().expect("recording sink mutex").push(item);
            Ok(())
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn poll_close(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            std::task::Poll::Ready(Ok(()))
        }
    }

    async fn run_writer(live: u64, items: Vec<Outbound>) -> (Vec<Message>, u64, u64) {
        let live_generation = Arc::new(AtomicU64::new(live));
        let dropped = Arc::new(DropCounters::default());
        let sink = RecordingSink::default();
        let (tx, rx) = mpsc::channel::<Outbound>(64);
        let writer = tokio::spawn(write_loop(
            sink.clone(),
            rx,
            Arc::clone(&live_generation),
            Arc::clone(&dropped),
        ));
        for item in items {
            tx.send(item).await.map_err(|_| ()).expect("queue item");
        }
        drop(tx);
        writer.await.expect("writer task completes");
        (
            sink.messages(),
            dropped.frames.load(Ordering::Acquire),
            dropped.bytes.load(Ordering::Acquire),
        )
    }

    fn audio(generation: u64) -> Outbound {
        Outbound::Frame {
            generation: Some(generation),
            message: Message::Binary(vec![1u8; 320]),
        }
    }

    fn control_frame() -> Outbound {
        Outbound::Frame {
            generation: None,
            message: Message::Text("{\"type\":\"speech_complete\"}".into()),
        }
    }

    #[tokio::test]
    async fn the_writer_drops_audio_whose_generation_is_no_longer_live() {
        // Generation 7 was retired before these were dequeued: not one byte may reach the socket,
        // while the untagged acknowledgement still must.
        let (sent, dropped_frames, dropped_bytes) = run_writer(
            NO_LIVE_GENERATION,
            vec![audio(7), audio(7), audio(7), audio(7), audio(7), control_frame()],
        )
        .await;
        assert_eq!(
            sent.len(),
            1,
            "only the untagged control frame may be sent, got {sent:?}"
        );
        assert!(matches!(sent[0], Message::Text(_)));
        assert_eq!(dropped_frames, 5);
        assert_eq!(dropped_bytes, 5 * 320);
    }

    #[tokio::test]
    async fn the_writer_sends_audio_whose_generation_is_still_live() {
        // The other half of the pair: without it, "drops everything" would also pass the test above.
        let (sent, dropped_frames, dropped_bytes) =
            run_writer(7, vec![audio(7), audio(7), control_frame()]).await;
        assert_eq!(sent.len(), 3, "live audio must not be dropped");
        assert_eq!(dropped_frames, 0);
        assert_eq!(dropped_bytes, 0);
    }

    #[tokio::test]
    async fn the_writer_drops_only_the_retired_generation_not_the_live_one() {
        let (sent, dropped_frames, _) =
            run_writer(9, vec![audio(8), audio(9), audio(8), audio(9)]).await;
        assert_eq!(sent.len(), 2, "the live generation's audio still goes out");
        assert_eq!(dropped_frames, 2, "the retired generation's does not");
    }

    /// The control for every interruption test below: with no interrupt, the same utterance is
    /// delivered in full. Without this, "zero bytes after a barge-in" would be equally consistent
    /// with a provider that produces nothing at all.
    #[tokio::test]
    async fn an_uninterrupted_utterance_is_delivered_in_full() {
        let provider = Arc::new(SlowTts::new(Duration::from_millis(300), 50));
        let url = serve_one(provider).await;
        let mut client = connect(&url).await;
        client.send(control("start_listening")).await.expect("send");
        client.send(speak("hello there")).await.expect("send");
        let seen = drain(
            &mut client,
            Duration::from_millis(300),
            Duration::from_secs(20),
        )
        .await;
        assert_eq!(
            audio_bytes(&seen),
            50 * 320,
            "the control must receive every chunk"
        );
        assert_eq!(count_text_of_type(&seen, "speech_starting"), 1);
        assert_eq!(count_text_of_type(&seen, "speech_complete"), 1);
    }

    /// A9/A7 acceptance test: the client must receive the FIRST audio chunk before the provider
    /// has finished synthesising the LAST one — not just "eventually receive every chunk"
    /// (`an_uninterrupted_utterance_is_delivered_in_full` already proves that). `StreamingTts`
    /// takes 200ms per chunk over 5 chunks (~1000ms total); `speech_complete` — sent only once
    /// synthesis of every chunk has returned — is the client-observable proxy for "synthesis
    /// finished". Under the old "return the whole `Vec<Vec<u8>>` at the end" trait this test
    /// fails: the first `Message::Binary` and `speech_complete` land within the same handful of
    /// milliseconds of each other, because nothing could reach the socket until everything did.
    #[tokio::test]
    async fn first_audio_chunk_is_sent_before_the_full_utterance_finishes_synthesizing() {
        let provider = Arc::new(StreamingTts::new(Duration::from_millis(200), 5));
        let url = serve_one(provider).await;
        let mut client = connect(&url).await;
        client.send(control("start_listening")).await.expect("send");
        let sent_at = Instant::now();
        client.send(speak("hello there")).await.expect("send");

        let seen = drain(
            &mut client,
            Duration::from_millis(300),
            Duration::from_secs(20),
        )
        .await;

        let first_chunk_at = first_binary_at(&seen).expect("at least one audio chunk arrived");
        let synthesis_done_at =
            first_text_of_type(&seen, "speech_complete").expect("synthesis finished eventually");
        assert!(
            first_chunk_at < synthesis_done_at,
            "first audio chunk must arrive strictly before synthesis of the whole utterance \
             completes; first chunk at {:?}, synthesis done at {:?}",
            first_chunk_at.duration_since(sent_at),
            synthesis_done_at.duration_since(sent_at),
        );
        // Not just "before" by a hair: the first chunk should land around one chunk_delay (200ms)
        // after the `speak` frame, not after all five (~1000ms) — that's the "true low-latency
        // streaming" property, not merely "the ordering of two events happened to hold".
        let first_chunk_latency = first_chunk_at.duration_since(sent_at);
        assert!(
            first_chunk_latency < Duration::from_millis(600),
            "first chunk took {first_chunk_latency:?} to arrive; a provider streaming one chunk \
             every 200ms should deliver the first one in around 200ms, not after all 5 were \
             synthesized (~1000ms) — the send path is still fully buffering",
        );
    }

    #[tokio::test]
    async fn barge_in_during_synthesis_delivers_no_audio_at_all() {
        // The measured defect, as a test: the interrupt lands while the relay is inside
        // `synthesize()`. Under the old sequential loop this frame could not even be READ until
        // the whole utterance had been synthesised and flushed, so every byte was delivered.
        let provider = Arc::new(SlowTts::new(Duration::from_millis(1500), 50));
        let url = serve_one(provider).await;
        let mut client = connect(&url).await;
        client.send(control("start_listening")).await.expect("send");
        client.send(speak("hello there")).await.expect("send");
        tokio::time::sleep(Duration::from_millis(50)).await;
        let interrupt_at = Instant::now();
        client.send(control("barge_in")).await.expect("send");

        let seen = drain(
            &mut client,
            Duration::from_millis(400),
            Duration::from_secs(20),
        )
        .await;
        assert_eq!(
            audio_bytes(&seen),
            0,
            "a barge-in during synthesis must deliver no audio; got {:?}",
            seen.iter().map(|o| &o.message).collect::<Vec<_>>()
        );
        let acked = first_text_of_type(&seen, "speech_complete")
            .expect("the yield is acknowledged to the client");
        let yield_ms = acked.duration_since(interrupt_at).as_millis() as u64;
        assert!(
            hot_path_within_budget(HotPathBudget::BargeInYield, yield_ms),
            "yield took {yield_ms}ms, budget is {}ms",
            hot_path_budget_ms(HotPathBudget::BargeInYield),
        );
    }

    #[tokio::test]
    async fn no_audio_ever_arrives_after_the_yield_is_acknowledged() {
        // The ordering invariant that makes barge-in safe: whatever was already queued for the
        // retired generation is dropped, so the client cannot be told "the orb stopped" and then
        // hear the rest of the sentence. 4000 chunks so the send loop is provably still running
        // when the interrupt lands.
        let provider = Arc::new(SlowTts::new(Duration::from_millis(120), 2000));
        let url = serve_one(provider).await;
        let mut client = connect(&url).await;
        client.send(control("start_listening")).await.expect("send");
        client.send(speak("a very long sentence")).await.expect("send");
        tokio::time::sleep(Duration::from_millis(130)).await;
        client.send(control("barge_in")).await.expect("send");

        let seen = drain(
            &mut client,
            Duration::from_millis(400),
            Duration::from_secs(20),
        )
        .await;
        let acked = first_text_of_type(&seen, "speech_complete").expect("yield acknowledged");
        let after: usize = seen
            .iter()
            .filter(|o| o.at > acked)
            .filter_map(|o| match &o.message {
                Message::Binary(bytes) => Some(bytes.len()),
                _ => None,
            })
            .sum();
        assert_eq!(
            after, 0,
            "audio from a retired generation reached the client after the yield"
        );
        assert_eq!(
            count_text_of_type(&seen, "speech_complete"),
            1,
            "an interrupted turn must not also announce its own completion"
        );
    }

    #[tokio::test]
    async fn a_second_speak_never_interleaves_the_first_utterances_audio() {
        // `first` fills its chunks with b'f', `second` with b's'. Any b'f' on the wire means a
        // retired generation reached the client.
        let provider = Arc::new(SlowTts::new(Duration::from_millis(400), 30));
        let url = serve_one(provider).await;
        let mut client = connect(&url).await;
        client.send(control("start_listening")).await.expect("send");
        client.send(speak("first")).await.expect("send");
        tokio::time::sleep(Duration::from_millis(50)).await;
        client.send(speak("second")).await.expect("send");

        let seen = drain(
            &mut client,
            Duration::from_millis(400),
            Duration::from_secs(20),
        )
        .await;
        let stale = seen
            .iter()
            .filter_map(|o| match &o.message {
                Message::Binary(bytes) => Some(bytes.iter().filter(|b| **b == b'f').count()),
                _ => None,
            })
            .sum::<usize>();
        assert_eq!(stale, 0, "audio from the superseded utterance was delivered");
        assert_eq!(
            audio_bytes(&seen),
            30 * 320,
            "the live utterance is delivered in full"
        );
    }

    #[tokio::test]
    async fn a_barge_in_is_not_starved_by_a_backlog_of_mic_frames() {
        // The real client sends ~200ms of mic audio BEFORE it qualifies a barge-in. When every one
        // of those frames costs a blocking STT round trip on the read loop, the barge-in queues
        // behind all of them. 20 frames x 60ms = 1.2s of STT work ahead of the interrupt.
        let provider = Arc::new(SlowStt {
            push_delay: Duration::from_millis(60),
        });
        let url = serve_one(provider).await;
        let mut client = connect(&url).await;
        client.send(control("start_listening")).await.expect("send");
        for _ in 0..20 {
            client
                .send(Message::Binary(vec![7u8; 320]))
                .await
                .expect("send mic frame");
        }
        let interrupt_at = Instant::now();
        client.send(control("barge_in")).await.expect("send");

        let seen = drain(
            &mut client,
            Duration::from_millis(300),
            Duration::from_secs(20),
        )
        .await;
        let acked = first_text_of_type(&seen, "speech_complete").expect("yield acknowledged");
        let yield_ms = acked.duration_since(interrupt_at).as_millis() as u64;
        assert!(
            hot_path_within_budget(HotPathBudget::BargeInYield, yield_ms),
            "yield took {yield_ms}ms behind a mic-frame backlog, budget is {}ms",
            hot_path_budget_ms(HotPathBudget::BargeInYield),
        );
    }

    #[tokio::test]
    async fn pause_during_synthesis_closes_without_playing_the_utterance() {
        // §5 in its strictest form, now that speech outlives the frame that started it: a pause
        // must not be followed by the audio the user paused over.
        let provider = Arc::new(SlowTts::new(Duration::from_millis(1000), 50));
        let url = serve_one(provider).await;
        let mut client = connect(&url).await;
        client.send(control("start_listening")).await.expect("send");
        client.send(speak("hello there")).await.expect("send");
        tokio::time::sleep(Duration::from_millis(50)).await;
        client.send(control("pause")).await.expect("send");

        let seen = drain(
            &mut client,
            Duration::from_millis(400),
            Duration::from_secs(20),
        )
        .await;
        assert_eq!(audio_bytes(&seen), 0, "no audio after a pause");
        assert_eq!(
            count_text_of_type(&seen, "closing"),
            1,
            "the client is told the session closed"
        );
    }

    // -----------------------------------------------------------------------------------------
    // A6: provider calls must be bounded, not blocking indefinitely.
    // -----------------------------------------------------------------------------------------

    /// A STT provider that never returns on its own — `push_audio` blocks forever unless
    /// something external stops waiting on it. This is the shape of a genuinely wedged provider
    /// (dead TCP peer, a sidecar that accepted the connection and then hung): a real network read
    /// would eventually get an OS-level error, but nothing here provides that, so the ONLY thing
    /// that can bound this call from the relay's side is `main.rs`'s own timeout.
    struct HangingStt;

    impl SttProvider for HangingStt {
        fn push_audio(
            &self,
            _session_id: &str,
            _frame: &[u8],
            _cancel: &dyn CancelSignal,
        ) -> Result<Option<Transcript>, ProviderError> {
            std::thread::park();
            unreachable!("a parked thread never wakes in this test");
        }

        fn end_of_turn(
            &self,
            _session_id: &str,
            _cancel: &dyn CancelSignal,
        ) -> Result<Transcript, ProviderError> {
            Ok(Transcript {
                text: "final".into(),
                is_final: true,
            })
        }
    }

    impl TtsProvider for HangingStt {
        fn synthesize(
            &self,
            _text: &str,
            _voice_id: &str,
            _emotion: &str,
            _cancel: &dyn CancelSignal,
            on_chunk: &mut dyn FnMut(Vec<u8>) -> Result<(), ProviderError>,
        ) -> Result<(), ProviderError> {
            on_chunk(vec![9u8; 320])
        }
    }

    impl ProviderAdapter for HangingStt {
        fn label(&self) -> &'static str {
            "hanging-test-stt"
        }
    }

    /// `HangingStt`'s `spawn_blocking` closure parks its OS thread forever by design (see its doc
    /// comment) — that thread never returns, on purpose, because a real wedged provider call gives
    /// us no cooperative way to stop it either. `tokio::time::timeout` correctly stops the *async*
    /// side from waiting on it (that's the fix under test), but it cannot cancel the blocking
    /// thread itself — only abandon it. The catch: tokio's default `Runtime::drop` blocks until
    /// every outstanding `spawn_blocking` task completes, including ones the async side has already
    /// abandoned via an elapsed timeout. `#[tokio::test]` creates exactly such a runtime and drops
    /// it right after the test body returns, so with a genuinely-forever-parked thread in flight,
    /// the *test harness's own teardown* hangs even though the code under test already did the
    /// right thing (the "session ended" line prints; the process just never gets back to report
    /// it). Building the runtime by hand and calling `shutdown_background()` — which detaches
    /// instead of joining — sidesteps that: it's safe here because the async body below has
    /// already run every assertion to completion by the time we call it.
    fn run_and_detach<F>(fut: F)
    where
        F: std::future::Future<Output = ()>,
    {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime");
        rt.block_on(fut);
        // Not `drop(rt)`: the default Drop impl joins every spawn_blocking task, which would hang
        // forever on the deliberately-immortal thread(s) this test's fake provider leaves behind.
        rt.shutdown_background();
    }

    /// The defect this task fixes: with no timeout, one mic frame handed to a provider that never
    /// answers parks that session's STT worker forever — the read loop never hears back and the
    /// connection just sits there, silently, until someone kills the process. `HangingStt::push_audio`
    /// truly never returns (a parked OS thread, not a bounded sleep), so this test can only pass if
    /// something OTHER than the provider itself stops the wait.
    ///
    /// Runs on a hand-built runtime via `run_and_detach` rather than `#[tokio::test]` — see that
    /// helper's doc comment for why the ordinary teardown would hang the test binary itself.
    #[test]
    fn a_hung_stt_call_returns_a_bounded_typed_failure_instead_of_hanging_forever() {
        run_and_detach(async {
        let provider_timeout = Duration::from_millis(80);
        let provider = Arc::new(HangingStt);
        let url = serve_one_with_timeout(provider, provider_timeout).await;
        let mut client = connect(&url).await;
        client.send(control("start_listening")).await.expect("send");
        let dispatched_at = Instant::now();
        client
            .send(Message::Binary(vec![1u8; 320]))
            .await
            .expect("send mic frame");

        // Generous relative to `provider_timeout` (80ms) but tight relative to "forever": if the
        // timeout wrapper is missing or a no-op, this provider never returns and nothing arrives
        // before `cap`, so the test times out having observed nothing rather than passing.
        let seen = drain(
            &mut client,
            Duration::from_millis(150),
            Duration::from_secs(5),
        )
        .await;

        let closed_at =
            first_text_of_type(&seen, "closing").expect("a hung provider call must still close the session, not hang it forever");
        let bound_ms = closed_at.duration_since(dispatched_at).as_millis() as u64;
        assert!(
            bound_ms < 2_000,
            "closing arrived after {bound_ms}ms — must be bounded near provider_timeout ({}ms), not left to hang",
            provider_timeout.as_millis(),
        );
        assert!(
            seen.iter().any(|o| matches!(&o.message, Message::Text(t) if t.contains("provider_failure"))),
            "the typed timeout must surface as the existing provider-failure close reason, not silence: {seen:?}",
        );
        });
    }

    /// The other half of the acceptance test: one session wedged on a hung provider call must not
    /// stall the relay for anyone else. A second, independent connection against the same listener
    /// (same provider, same timeout) is served normally while the first is still hung.
    ///
    /// Also runs via `run_and_detach` (see its doc comment) — this test's `stuck` session leaves
    /// its own immortally-parked `spawn_blocking` thread behind, which would hang an ordinary
    /// `#[tokio::test]`'s teardown exactly like the sibling test above.
    #[test]
    fn a_hung_session_does_not_stall_other_sessions_on_the_same_relay() {
        run_and_detach(async {
        let provider_timeout = Duration::from_millis(80);
        let provider = Arc::new(HangingStt);
        let url = serve_many_with_timeout(provider, provider_timeout).await;

        let mut stuck = connect(&url).await;
        stuck
            .send(control("start_listening"))
            .await
            .expect("send");
        stuck
            .send(Message::Binary(vec![1u8; 320]))
            .await
            .expect("send mic frame that hangs this session's STT worker");

        // Give the stuck session's frame time to actually reach `HangingStt::push_audio` before
        // proving the second session is unaffected — otherwise this would pass trivially by
        // racing ahead of the hang ever starting.
        tokio::time::sleep(Duration::from_millis(20)).await;

        let mut other = connect(&url).await;
        let other_started_at = Instant::now();
        other
            .send(control("start_listening"))
            .await
            .expect("send");
        other
            .send(control("end_of_turn"))
            .await
            .expect("send end_of_turn");
        let seen = drain(
            &mut other,
            Duration::from_millis(150),
            Duration::from_secs(5),
        )
        .await;
        let transcript_at = first_text_of_type(&seen, "transcript")
            .expect("the second session must still be served while the first is hung");
        let served_ms = transcript_at.duration_since(other_started_at).as_millis() as u64;
        assert!(
            served_ms < 1_000,
            "second session took {served_ms}ms while another session was hung on a provider call",
        );
        });
    }
}

#[cfg(test)]
mod audio_synthetic_devlog_field {
    use super::*;

    fn telemetry() -> SpeechTelemetry {
        SpeechTelemetry {
            generation: 1,
            user_stopped_at: None,
            speak_received_at: Instant::now(),
            first_chunk_at: Some(Instant::now()),
            audio_chunks: 1,
            audio_bytes: 320,
            cancelled: false,
        }
    }

    // This seam was previously untestable: `audio_synthetic` only existed as an inline
    // expression inside `log_speech_outcome`, a fn that returns () and communicates only by
    // writing to devlog (real file I/O keyed off env vars). Extracting `speech_outcome_fields`
    // makes it a pure, directly assertable computation — a hardcoded `false` here would now
    // fail this test, where before it could not be caught by any unit test.
    #[test]
    fn a_fake_provider_label_is_marked_synthetic() {
        let outcome = SpeechOutcome::Delivered { generation: 1, chars: 5 };
        assert!(speech_outcome_fields(&outcome, &telemetry(), "fake-t0").audio_synthetic);
    }

    #[test]
    fn a_real_provider_label_is_never_marked_synthetic() {
        let outcome = SpeechOutcome::Delivered { generation: 1, chars: 5 };
        assert!(!speech_outcome_fields(&outcome, &telemetry(), "sarvam").audio_synthetic);
        assert!(!speech_outcome_fields(&outcome, &telemetry(), "cartesia").audio_synthetic);
    }
}
