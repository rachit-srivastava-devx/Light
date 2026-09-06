//! The per-session relay decisions: mic frames in, transcripts and TTS audio out.
//!
//! Written as a pure state machine over frames (`handle_client_frame`) rather than inline inside
//! the socket accept loop, so every rule below is unit-testable without a socket:
//!
//!   - the pause contract (§5): a `Pause` closes the session, and NOTHING is buffered or billed
//!     afterwards. A frame arriving after a pause is dropped, not queued.
//!   - barge-in (§3): a `BargeIn` cancels in-flight speech; the yield budget is asserted by the
//!     caller against `latency_budget::BARGE_IN_YIELD_BUDGET_MS`.
//!   - usage counters accumulate in-process and are reported to relay-py out-of-band
//!     (docs/adr/0004): a synchronous HTTP call here would put a network round-trip on the audio
//!     hot path, which is exactly what the Python/Rust split exists to avoid.
//!
//! ## Why this module performs no I/O at all
//!
//! It used to call `tts.synthesize()` inline. That single line is what made barge-in a no-op: the
//! call is seconds long for a real provider, the socket read loop was parked inside it, and the
//! `barge_in` frame sitting in the kernel buffer could not be read until it returned — so a
//! barge-in could only ever cancel the NEXT turn (measured at p50 1897ms against a 100ms budget;
//! diagnosed three weeks earlier as Fix 9 in docs/REVIEW-2026-08-06-END-TO-END-FLOW.md).
//!
//! So provider work is now *described*, not performed: `Action::BeginSpeech` and
//! `Action::DispatchAudio` hand the work to the socket layer, which runs it off the read loop and
//! feeds the result back through `on_speech_outcome` / `on_stt_outcome`. This module stays the
//! single owner of phase and usage, and stays a pure function of the frames it is handed.
//!
//! ## Generations
//!
//! Every `Speak` gets a monotonically increasing generation. It is the identity of one utterance's
//! audio, and it is what makes cancellation safe: audio and completion frames are tagged with it,
//! and anything belonging to a generation that is no longer live is dropped before it reaches the
//! socket instead of being played over the user who just interrupted.

use crate::protocol::{ClientFrame, CloseReason, ServerFrame};
use crate::provider::{ProviderError, Transcript};
use serde_json::{Map, Value};

/// What the socket layer should do after a frame. It executes these; this module decides.
#[derive(Debug, PartialEq)]
pub enum Action {
    Send(Vec<ServerFrame>),
    /// Push the mic frame that produced this action to STT, off the read loop. The result comes
    /// back through `on_stt_outcome`; the read loop must not wait for it.
    DispatchAudio { session_id: String },
    /// Finalise the turn at STT, off the read loop. Result likewise via `on_stt_outcome`.
    DispatchEndOfTurn { session_id: String },
    /// Start synthesising, off the read loop.
    ///
    /// Implies "cancel any older generation first": a second `Speak` while one is still in flight
    /// supersedes it, and the superseded audio must never reach the socket.
    BeginSpeech(SpeechRequest),
    /// Yield the floor: cancel the live speech generation and discard everything it has already
    /// queued, THEN send these frames. The order matters — reversing it lets stale audio play
    /// after the client has been told the orb stopped talking.
    YieldSpeech(Vec<ServerFrame>),
    /// Send these frames, then close the socket. Used for both pause and failure — the frames
    /// carry which one it was. Also cancels any live generation: the session is over.
    SendAndClose(Vec<ServerFrame>),
    /// Send these frames but keep the socket open. The session enters degraded mode: no new
    /// voice work is accepted, but pause and connection lifecycle remain available. Used for
    /// the audio-bed fallback when the voice provider fails — presence is preserved (§5).
    SendFallback(Vec<ServerFrame>),
    /// Frame arrived after the session ended. Dropped deliberately (§5: nothing captured).
    Ignore,
}

/// One utterance handed to the socket layer to synthesise and stream.
#[derive(Debug, PartialEq, Clone)]
pub struct SpeechRequest {
    pub generation: u64,
    pub text: String,
    pub voice_id: String,
    pub emotion: String,
    /// Sent BEFORE synthesis begins, so the client ducks the bed while it waits rather than after
    /// the audio has already started (protocol.rs's `SpeechStarting` contract).
    pub starting: ServerFrame,
    /// Queued after the last chunk, tagged with `generation` so a barge-in drops it — the client
    /// must not be told "speech complete" twice for one interrupted turn.
    pub complete: ServerFrame,
}

/// How a `BeginSpeech` ended. Fed back so this module remains the only place phase and usage move.
#[derive(Debug, PartialEq)]
pub enum SpeechOutcome {
    /// Synthesis returned and every chunk was queued for delivery.
    Delivered { generation: u64, chars: u64 },
    /// Abandoned by a cancel.
    ///
    /// `chars` is 0 when the cancel beat dispatch, and the utterance's length when the request had
    /// already reached the provider — which bills for work it has begun whether or not the bytes
    /// were ever played.
    Cancelled { generation: u64, chars: u64 },
    /// The provider failed. Carries the detail rather than a `ProviderError` on purpose: a cancel
    /// is not a failure, and this variant must not be able to express one.
    Failed { generation: u64, detail: String },
}

/// How a dispatched STT call ended.
#[derive(Debug, PartialEq)]
pub enum SttOutcome {
    /// A streaming push returned, with a partial transcript if the provider emitted one.
    Push(Result<Option<Transcript>, SttFailure>),
    /// An end-of-turn finalise returned.
    EndOfTurn(Result<Transcript, SttFailure>),
}

/// Why a dispatched STT call did not produce a transcript.
#[derive(Debug, PartialEq)]
pub enum SttFailure {
    /// The provider is down. Closes the session (§5: the client keeps the bed alive and says so).
    Unavailable(String),
    /// The call was abandoned because the session was tearing down. Not an outage; not reported.
    Cancelled,
}

impl From<ProviderError> for SttFailure {
    fn from(err: ProviderError) -> Self {
        match err {
            ProviderError::Unavailable(detail) => SttFailure::Unavailable(detail),
            ProviderError::Cancelled => SttFailure::Cancelled,
            // Bounded, not silent: a hung STT call surfaces the same way an outage would (closes
            // the session so the client keeps its bed alive per §5) rather than parking the read
            // loop forever waiting for a provider that will never answer.
            ProviderError::TimedOut => SttFailure::Unavailable("provider timed out".into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionIdentity {
    pub tenant_id: String,
    pub session_id: String,
}

/// Billable work accumulated in-process, drained by the out-of-band reporter.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct UsageCounters {
    pub stt_frames: u64,
    pub tts_chars: u64,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum Phase {
    /// Before `StartListening`, or after the session ended.
    Idle,
    Listening,
    Speaking,
    /// Voice provider failed but the session stays alive. Audio bed is preserved; no new
    /// voice work is accepted. Only pause is allowed to close from here.
    Degraded,
    /// Terminal. Reached by pause or explicit end; never leaves.
    Ended,
}

pub struct Session {
    identity: Option<SessionIdentity>,
    phase: Phase,
    /// Next generation to hand out. Starts at 1 so 0 can mean "no speech is live" in the socket
    /// layer's atomic. u64 at one utterance per millisecond overflows in ~580 million years, so
    /// the `+= 1` below is left to panic in debug rather than wrap into a colliding generation.
    next_generation: u64,
    /// The generation whose audio is currently allowed on the wire, if any.
    speaking_generation: Option<u64>,
    pub usage: UsageCounters,
}

impl Session {
    pub fn new() -> Self {
        Self {
            identity: None,
            phase: Phase::Idle,
            next_generation: 1,
            speaking_generation: None,
            usage: UsageCounters::default(),
        }
    }

    #[cfg(test)]
    pub fn has_ended(&self) -> bool {
        self.phase == Phase::Ended
    }

    #[cfg(test)]
    pub fn is_speaking(&self) -> bool {
        self.phase == Phase::Speaking
    }

    #[cfg(test)]
    pub fn is_degraded(&self) -> bool {
        self.phase == Phase::Degraded
    }

    /// Decides what to do with one binary mic frame. Performs no I/O: `DispatchAudio` tells the
    /// socket layer to push the frame it just read to STT off the read loop.
    ///
    /// Returns `Ignore` once the session has ended or is degraded — the §5 pause contract is
    /// "nothing is captured while paused: not buffered, not held for context", and a degraded
    /// session has lost its voice path so further audio cannot produce transcripts.
    ///
    /// Audio is accepted while the orb is SPEAKING as well as while listening. That is not an
    /// oversight: a user talking over the orb is exactly the barge-in case, and their first
    /// ~200ms of speech arrives before the on-device detector has qualified it and sent
    /// `barge_in` (docs/BUILD-DIGEST.md §3). Dropping those frames would silently truncate the
    /// interrupting turn.
    pub fn on_audio_frame(&mut self) -> Action {
        if !matches!(
            self.phase,
            Phase::Listening | Phase::Speaking
        ) {
            let reason = match self.phase {
                Phase::Ended => "session_ended",
                Phase::Degraded => "voice_degraded",
                _ => "audio_while_not_listening",
            };
            return self.ignore(reason, None);
        }
        let Some(identity) = self.identity.as_ref() else {
            return self.ignore("identity_not_established", None);
        };
        Action::DispatchAudio {
            session_id: identity.session_id.clone(),
        }
    }

    /// Folds a dispatched STT result back into the session.
    pub fn on_stt_outcome(&mut self, outcome: SttOutcome) -> Action {
        if self.phase == Phase::Ended || self.phase == Phase::Degraded {
            // A result for work dispatched just before a pause or provider failure. §5: nothing
            // is captured, so it is not billed and its transcript is not delivered.
            return self.ignore("session_ended", None);
        }
        let Some(identity) = self.identity.clone() else {
            return self.ignore("identity_not_established", None);
        };
        match outcome {
            SttOutcome::Push(Ok(transcript)) => {
                self.usage.stt_frames += 1;
                match transcript {
                    Some(transcript) => Action::Send(vec![ServerFrame::Transcript {
                        tenant_id: identity.tenant_id,
                        session_id: identity.session_id,
                        text: transcript.text,
                        is_final: transcript.is_final,
                    }]),
                    None => Action::Send(vec![]),
                }
            }
            SttOutcome::EndOfTurn(Ok(transcript)) => Action::Send(vec![ServerFrame::Transcript {
                tenant_id: identity.tenant_id,
                session_id: identity.session_id,
                text: transcript.text,
                is_final: true,
            }]),
            SttOutcome::Push(Err(failure)) | SttOutcome::EndOfTurn(Err(failure)) => {
                match failure {
                    SttFailure::Cancelled => self.ignore("stt_cancelled", None),
                    SttFailure::Unavailable(detail) => self.fail(&detail),
                }
            }
        }
    }

    /// Folds a `BeginSpeech`'s outcome back into the session.
    pub fn on_speech_outcome(&mut self, outcome: SpeechOutcome) -> Action {
        match outcome {
            // Billed even when the generation is already stale: those characters were synthesised
            // and charged for, and a barge-in that lands after delivery does not refund them.
            SpeechOutcome::Delivered { generation, chars }
            | SpeechOutcome::Cancelled { generation, chars } => {
                self.usage.tts_chars += chars;
                self.settle_generation(generation);
                Action::Send(vec![])
            }
            SpeechOutcome::Failed { generation, detail } => {
                if self.speaking_generation != Some(generation) {
                    // A superseded utterance failing on its way out is not this session's problem:
                    // the client has already moved on, and closing here would drop a live turn.
                    return self.ignore("stale_speech_failure", None);
                }
                self.speaking_generation = None;
                self.fail(&detail)
            }
        }
    }

    /// Retires `generation` if it is still the live one. A stale generation changes nothing —
    /// the phase it would return to has already been claimed by whatever superseded it.
    fn settle_generation(&mut self, generation: u64) {
        if self.speaking_generation != Some(generation) {
            return;
        }
        self.speaking_generation = None;
        if self.phase == Phase::Speaking {
            self.phase = Phase::Listening;
        }
    }

    pub fn handle_client_frame(&mut self, frame: ClientFrame) -> Action {
        if self.phase == Phase::Ended {
            return self.ignore("session_ended", None);
        }
        if self.phase == Phase::Degraded {
            // In degraded mode (provider failed, audio bed preserved), only Pause is allowed
            // to close the connection. All other frames are ignored — voice work cannot be
            // accepted without a working provider.
            return match frame {
                ClientFrame::Pause {
                    tenant_id,
                    session_id,
                } if self.identity_matches(&tenant_id, &session_id) => {
                    let identity = self.identity.clone().unwrap_or(SessionIdentity {
                        tenant_id: "unknown".to_string(),
                        session_id: "unknown".to_string(),
                    });
                    self.speaking_generation = None;
                    self.phase = Phase::Ended;
                    Action::SendAndClose(vec![ServerFrame::Closing {
                        tenant_id: identity.tenant_id,
                        session_id: identity.session_id,
                        reason: CloseReason::UserPause,
                    }])
                }
                _ => self.ignore("voice_degraded", None),
            };
        }
        match frame {
            ClientFrame::StartListening {
                tenant_id,
                session_id,
            } => {
                if tenant_id.is_empty() || session_id.is_empty() {
                    return self.ignore("identity_fields_empty", Some((&tenant_id, &session_id)));
                }
                if let Some(identity) = &self.identity {
                    if identity.tenant_id != tenant_id || identity.session_id != session_id {
                        return self.ignore("identity_mismatch", Some((&tenant_id, &session_id)));
                    }
                }
                self.identity = Some(SessionIdentity {
                    tenant_id: tenant_id.clone(),
                    session_id: session_id.clone(),
                });
                self.phase = Phase::Listening;
                // The delivery receipt: the client must not present a "listening" state on local
                // mic-open alone, only once the relay has actually accepted the session.
                Action::Send(vec![ServerFrame::ListeningConfirmed {
                    tenant_id,
                    session_id,
                }])
            }
            ClientFrame::EndOfTurn {
                tenant_id,
                session_id,
            } if self.identity_matches(&tenant_id, &session_id) => {
                let Some(identity) = self.identity.as_ref() else {
                    return self
                        .ignore("identity_not_established", Some((&tenant_id, &session_id)));
                };
                Action::DispatchEndOfTurn {
                    session_id: identity.session_id.clone(),
                }
            }
            ClientFrame::Speak {
                tenant_id,
                session_id,
                text,
                voice_id,
                emotion,
            } if self.identity_matches(&tenant_id, &session_id) => {
                let Some(identity) = self.identity.clone() else {
                    return self
                        .ignore("identity_not_established", Some((&tenant_id, &session_id)));
                };
                let generation = self.next_generation;
                self.next_generation += 1;
                self.speaking_generation = Some(generation);
                self.phase = Phase::Speaking;
                // Nothing is billed here. The socket layer reports back what the provider was
                // actually asked to synthesise, so a `Speak` cancelled before dispatch costs
                // nothing and one cancelled mid-flight still costs what the provider charges.
                Action::BeginSpeech(SpeechRequest {
                    generation,
                    text,
                    voice_id,
                    emotion,
                    starting: ServerFrame::SpeechStarting {
                        tenant_id: identity.tenant_id.clone(),
                        session_id: identity.session_id.clone(),
                    },
                    complete: ServerFrame::SpeechComplete {
                        tenant_id: identity.tenant_id,
                        session_id: identity.session_id,
                    },
                })
            }
            ClientFrame::BargeIn {
                tenant_id,
                session_id,
            } if self.identity_matches(&tenant_id, &session_id) => {
                // Yield immediately and return to listening. The live generation is retired here,
                // so its audio — synthesised or not, queued or not — can no longer reach the
                // socket, and its own `speech_complete` is dropped in favour of this one.
                let Some(identity) = self.identity.clone() else {
                    return self
                        .ignore("identity_not_established", Some((&tenant_id, &session_id)));
                };
                self.speaking_generation = None;
                self.phase = Phase::Listening;
                Action::YieldSpeech(vec![ServerFrame::SpeechComplete {
                    tenant_id: identity.tenant_id,
                    session_id: identity.session_id,
                }])
            }
            ClientFrame::Pause {
                tenant_id,
                session_id,
            } if self.identity_matches(&tenant_id, &session_id) => {
                let Some(identity) = self.identity.clone() else {
                    self.phase = Phase::Ended;
                    return self
                        .ignore("identity_not_established", Some((&tenant_id, &session_id)));
                };
                self.speaking_generation = None;
                self.phase = Phase::Ended;
                Action::SendAndClose(vec![ServerFrame::Closing {
                    tenant_id: identity.tenant_id,
                    session_id: identity.session_id,
                    reason: CloseReason::UserPause,
                }])
            }
            ClientFrame::EndOfTurn {
                tenant_id,
                session_id,
            }
            | ClientFrame::BargeIn {
                tenant_id,
                session_id,
            }
            | ClientFrame::Pause {
                tenant_id,
                session_id,
            } => self.ignore_identity_mismatch(&tenant_id, &session_id),
            ClientFrame::Speak {
                tenant_id,
                session_id,
                ..
            } => self.ignore_identity_mismatch(&tenant_id, &session_id),
        }
    }

    fn identity_matches(&self, tenant_id: &str, session_id: &str) -> bool {
        self.identity.as_ref().is_some_and(|identity| {
            identity.tenant_id == tenant_id && identity.session_id == session_id
        })
    }

    fn ignore_identity_mismatch(&self, tenant_id: &str, session_id: &str) -> Action {
        let reason = if self.identity.is_some() {
            "identity_mismatch"
        } else {
            "identity_not_established"
        };
        self.ignore(reason, Some((tenant_id, session_id)))
    }

    fn ignore(&self, reason: &'static str, actual: Option<(&str, &str)>) -> Action {
        let fields = self.ignore_fields(reason, actual);
        #[cfg(not(test))]
        crate::devlog::dev_log("frame.ignored", "warn", fields);
        #[cfg(test)]
        let _ = fields;
        Action::Ignore
    }

    fn ignore_fields(
        &self,
        reason: &'static str,
        actual: Option<(&str, &str)>,
    ) -> Map<String, Value> {
        let mut fields = Map::new();
        fields.insert("reason".into(), Value::String(reason.into()));
        fields.insert(
            "phase".into(),
            Value::String(format!("{:?}", self.phase).to_lowercase()),
        );
        fields.insert(
            "expected_tenant_id".into(),
            self.identity
                .as_ref()
                .map(|identity| Value::String(identity.tenant_id.clone()))
                .unwrap_or(Value::Null),
        );
        fields.insert(
            "expected_session_id".into(),
            self.identity
                .as_ref()
                .map(|identity| Value::String(identity.session_id.clone()))
                .unwrap_or(Value::Null),
        );
        fields.insert(
            "actual_tenant_id".into(),
            actual
                .map(|(tenant_id, _)| Value::String(tenant_id.to_string()))
                .unwrap_or(Value::Null),
        );
        fields.insert(
            "actual_session_id".into(),
            actual
                .map(|(_, session_id)| Value::String(session_id.to_string()))
                .unwrap_or(Value::Null),
        );
        fields
    }

    fn fail(&mut self, detail: &str) -> Action {
        // Logged rather than sent: the client is told *that* the provider failed (so it keeps the
        // bed alive and says so once, §5) but not why — a provider's error text is operator
        // information, not something to read aloud to a user mid-session.
        //
        // The session enters Degraded, not Ended: the WebSocket stays open so the client can
        // still receive the audio-bed fallback frame and continue the presence contract. Only
        // an explicit pause (§5) tears down the connection from here.
        let identity = self.identity.clone().unwrap_or(SessionIdentity {
            tenant_id: "unknown".to_string(),
            session_id: "unknown".to_string(),
        });
        eprintln!(
            "session {}: provider unavailable (degraded): {detail}",
            identity.session_id
        );
        self.phase = Phase::Degraded;
        self.speaking_generation = None;
        Action::SendFallback(vec![ServerFrame::AudioBedFallback {
            tenant_id: identity.tenant_id,
            session_id: identity.session_id,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started() -> Session {
        let mut session = Session::new();
        session.handle_client_frame(ClientFrame::StartListening {
            tenant_id: "t1".into(),
            session_id: "s1".into(),
        });
        session
    }

    fn speak(session: &mut Session, text: &str) -> SpeechRequest {
        match session.handle_client_frame(ClientFrame::Speak {
            tenant_id: "t1".into(),
            session_id: "s1".into(),
            text: text.into(),
            voice_id: "v".into(),
            emotion: "warm".into(),
        }) {
            Action::BeginSpeech(request) => request,
            other => panic!("expected BeginSpeech, got {other:?}"),
        }
    }

    fn barge_in(session: &mut Session) -> Action {
        session.handle_client_frame(ClientFrame::BargeIn {
            tenant_id: "t1".into(),
            session_id: "s1".into(),
        })
    }

    fn pause(session: &mut Session) -> Action {
        session.handle_client_frame(ClientFrame::Pause {
            tenant_id: "t1".into(),
            session_id: "s1".into(),
        })
    }

    fn partial(text: &str) -> SttOutcome {
        SttOutcome::Push(Ok(Some(Transcript {
            text: text.into(),
            is_final: false,
        })))
    }

    #[test]
    fn start_listening_sends_a_listening_confirmed_delivery_receipt() {
        // The defect this guards against: the client used to get `Action::Send(vec![])` back for
        // `StartListening`, so it had no relay acknowledgment at all and had to infer "listening"
        // from local mic capture opening. That is silently wrong when the relay never actually
        // accepted the session (e.g. an identity mismatch, or the socket dying right after).
        let mut session = Session::new();
        let action = session.handle_client_frame(ClientFrame::StartListening {
            tenant_id: "t1".into(),
            session_id: "s1".into(),
        });
        match action {
            Action::Send(frames) => {
                assert_eq!(
                    frames,
                    vec![ServerFrame::ListeningConfirmed {
                        tenant_id: "t1".into(),
                        session_id: "s1".into(),
                    }]
                );
            }
            other => panic!("expected Send([ListeningConfirmed]), got {other:?}"),
        }
    }

    #[test]
    fn a_rejected_start_listening_sends_no_delivery_receipt() {
        // An identity mismatch never opens the STT stream, so no receipt should be sent — the
        // client must not be told "confirmed" for a frame the relay dropped.
        let mut session = started();
        let action = session.handle_client_frame(ClientFrame::StartListening {
            tenant_id: "other-tenant".into(),
            session_id: "other-session".into(),
        });
        assert_eq!(action, Action::Ignore);
    }

    #[test]
    fn audio_before_start_listening_is_ignored() {
        let mut session = Session::new();
        assert_eq!(session.on_audio_frame(), Action::Ignore);
        assert_eq!(session.usage.stt_frames, 0);
    }

    #[test]
    fn audio_frames_dispatch_to_stt_and_surface_partials() {
        let mut session = started();
        assert_eq!(
            session.on_audio_frame(),
            Action::DispatchAudio {
                session_id: "s1".into()
            }
        );
        session.on_stt_outcome(SttOutcome::Push(Ok(None)));
        session.on_stt_outcome(SttOutcome::Push(Ok(None)));
        let third = session.on_stt_outcome(partial("partial after 3 frames"));
        assert_eq!(session.usage.stt_frames, 3);
        match third {
            Action::Send(frames) => assert_eq!(frames.len(), 1, "the partial is forwarded"),
            other => panic!("expected Send, got {other:?}"),
        }
    }

    #[test]
    fn mic_audio_is_still_accepted_while_the_orb_is_speaking() {
        // The user talking over the orb IS the barge-in: their first ~200ms arrives before the
        // on-device detector qualifies it. Dropping it here truncates the interrupting turn.
        let mut session = started();
        speak(&mut session, "a long sentence");
        assert!(session.is_speaking());
        assert_eq!(
            session.on_audio_frame(),
            Action::DispatchAudio {
                session_id: "s1".into()
            }
        );
    }

    #[test]
    fn pause_closes_the_session_with_the_user_pause_reason() {
        let mut session = started();
        let action = pause(&mut session);
        assert!(session.has_ended());
        match action {
            Action::SendAndClose(frames) => assert_eq!(
                frames,
                vec![ServerFrame::Closing {
                    tenant_id: "t1".into(),
                    session_id: "s1".into(),
                    reason: CloseReason::UserPause,
                }]
            ),
            other => panic!("expected SendAndClose, got {other:?}"),
        }
    }

    #[test]
    fn nothing_is_captured_or_billed_after_a_pause() {
        // The §5 contract in its strictest form: not buffered, not held for context, not billed.
        let mut session = started();
        session.on_audio_frame();
        session.on_stt_outcome(SttOutcome::Push(Ok(None)));
        let before = session.usage.stt_frames;
        pause(&mut session);
        assert_eq!(session.on_audio_frame(), Action::Ignore);
        // even a result for work dispatched just before the pause
        assert_eq!(
            session.on_stt_outcome(partial("late partial")),
            Action::Ignore
        );
        assert_eq!(
            session.usage.stt_frames, before,
            "no usage accrues after pause"
        );
    }

    #[test]
    fn frames_after_a_pause_are_ignored_not_processed() {
        let mut session = started();
        pause(&mut session);
        let action = session.handle_client_frame(ClientFrame::Speak {
            tenant_id: "t1".into(),
            session_id: "s1".into(),
            text: "hello".into(),
            voice_id: "v".into(),
            emotion: "warm".into(),
        });
        assert_eq!(action, Action::Ignore, "no synthesis is even requested");
    }

    #[test]
    fn speak_requests_synthesis_and_bills_characters_only_once_delivered() {
        let mut session = started();
        let request = speak(&mut session, "one down");
        assert_eq!(request.text, "one down");
        assert_eq!(
            request.starting,
            ServerFrame::SpeechStarting {
                tenant_id: "t1".into(),
                session_id: "s1".into(),
            }
        );
        assert_eq!(
            request.complete,
            ServerFrame::SpeechComplete {
                tenant_id: "t1".into(),
                session_id: "s1".into(),
            }
        );
        assert_eq!(
            session.usage.tts_chars, 0,
            "nothing is billed until the provider has actually been asked"
        );
        assert!(session.is_speaking());

        session.on_speech_outcome(SpeechOutcome::Delivered {
            generation: request.generation,
            // Counted in characters, not chunks: §2's cost event meters `tts_chars_novel`, and
            // chunk sizing is a provider detail that must not affect the bill.
            chars: 8,
        });
        assert_eq!(session.usage.tts_chars, 8);
        assert!(!session.is_speaking());
    }

    #[test]
    fn accepts_the_next_user_turn_after_speech_completes() {
        let mut session = started();
        let request = speak(&mut session, "hello");
        session.on_speech_outcome(SpeechOutcome::Delivered {
            generation: request.generation,
            chars: 5,
        });
        assert_eq!(
            session.on_audio_frame(),
            Action::DispatchAudio {
                session_id: "s1".into()
            }
        );
    }

    #[test]
    fn mismatched_tenant_frame_is_ignored() {
        let mut session = started();
        let fields = session.ignore_fields(
            "identity_mismatch",
            Some(("other-tenant", "foreign-session")),
        );
        assert_eq!(fields["reason"], "identity_mismatch");
        assert_eq!(fields["expected_tenant_id"], "t1");
        assert_eq!(fields["expected_session_id"], "s1");
        assert_eq!(fields["actual_tenant_id"], "other-tenant");
        assert_eq!(fields["actual_session_id"], "foreign-session");
        let action = session.handle_client_frame(ClientFrame::Pause {
            tenant_id: "other-tenant".into(),
            session_id: "s1".into(),
        });
        assert_eq!(action, Action::Ignore);
        assert!(!session.has_ended());
    }

    #[test]
    fn barge_in_yields_the_floor_and_retires_the_live_generation() {
        let mut session = started();
        let request = speak(&mut session, "a long sentence");
        let action = barge_in(&mut session);
        assert_eq!(
            action,
            Action::YieldSpeech(vec![ServerFrame::SpeechComplete {
                tenant_id: "t1".into(),
                session_id: "s1".into(),
            }]),
            "the yield must cancel, not merely announce"
        );
        assert!(!session.is_speaking(), "barge-in yields the floor");
        assert!(!session.has_ended(), "and keeps listening rather than ending");

        // The cancelled generation's own completion must not resurrect the speaking phase.
        session.on_speech_outcome(SpeechOutcome::Delivered {
            generation: request.generation,
            chars: 15,
        });
        assert!(!session.is_speaking());
    }

    #[test]
    fn a_barge_in_that_beats_dispatch_bills_nothing() {
        let mut session = started();
        let request = speak(&mut session, "a long sentence");
        barge_in(&mut session);
        session.on_speech_outcome(SpeechOutcome::Cancelled {
            generation: request.generation,
            chars: 0,
        });
        assert_eq!(
            session.usage.tts_chars, 0,
            "the provider was never asked, so nothing is billable"
        );
    }

    #[test]
    fn a_barge_in_after_dispatch_still_bills_what_the_provider_was_asked_for() {
        // The honest half of the pair above: the request reached the paid provider, which charges
        // for work it has begun. Reporting 0 here would understate real cost (C12).
        let mut session = started();
        let request = speak(&mut session, "a long sentence");
        barge_in(&mut session);
        session.on_speech_outcome(SpeechOutcome::Cancelled {
            generation: request.generation,
            chars: 15,
        });
        assert_eq!(session.usage.tts_chars, 15);
    }

    #[test]
    fn a_second_speak_supersedes_the_first_generation() {
        let mut session = started();
        let first = speak(&mut session, "one");
        let second = speak(&mut session, "two");
        assert!(
            second.generation > first.generation,
            "generations are monotonic: {} then {}",
            first.generation,
            second.generation
        );
        // The superseded utterance settling must not end the live one.
        session.on_speech_outcome(SpeechOutcome::Cancelled {
            generation: first.generation,
            chars: 3,
        });
        assert!(
            session.is_speaking(),
            "the second utterance is still the live one"
        );
        session.on_speech_outcome(SpeechOutcome::Delivered {
            generation: second.generation,
            chars: 3,
        });
        assert!(!session.is_speaking());
        assert_eq!(session.usage.tts_chars, 6, "both were dispatched, both bill");
    }

    #[test]
    fn provider_failure_falls_back_to_audio_bed_and_keeps_session_alive() {
        // Defect A4 fix: provider failure must NOT tear down the session. The client keeps the
        // bed alive (§5) and the session continues in degraded mode so the user is not left in
        // unexplained silence.
        let mut session = started();
        session.on_audio_frame();
        let action = session.on_stt_outcome(SttOutcome::Push(Err(SttFailure::Unavailable(
            "fake stt down".into(),
        ))));
        match action {
            Action::SendFallback(frames) => {
                assert_eq!(frames.len(), 1);
                match &frames[0] {
                    ServerFrame::AudioBedFallback {
                        tenant_id,
                        session_id,
                    } => {
                        assert_eq!(tenant_id, "t1");
                        assert_eq!(session_id, "s1");
                    }
                    other => panic!("expected AudioBedFallback, got {other:?}"),
                }
            }
            other => panic!("expected SendFallback, got {other:?}"),
        }
        assert!(
            session.is_degraded(),
            "session must be degraded, not ended — the bed must stay alive"
        );
        assert!(
            !session.has_ended(),
            "session must NOT be ended — only a pause can end it from degraded"
        );
    }

    #[test]
    fn tts_provider_failure_falls_back_to_audio_bed_and_keeps_session_alive() {
        // TTS failure mid-speech also falls back to the audio bed instead of closing.
        let mut session = started();
        let request = speak(&mut session, "hello");
        let action = session.on_speech_outcome(SpeechOutcome::Failed {
            generation: request.generation,
            detail: "tts down".into(),
        });
        assert!(
            matches!(action, Action::SendFallback(_)),
            "expected SendFallback, got {action:?}"
        );
        assert!(session.is_degraded());
        assert!(!session.has_ended());
    }

    #[test]
    fn a_cancelled_stt_call_is_not_an_outage() {
        let mut session = started();
        session.on_audio_frame();
        assert_eq!(
            session.on_stt_outcome(SttOutcome::Push(Err(SttFailure::Cancelled))),
            Action::Ignore
        );
        assert!(
            !session.has_ended(),
            "a cancel must never close the session or drop the user's bed"
        );
        assert_eq!(session.usage.stt_frames, 0);
    }

    #[test]
    fn a_failed_synthesis_for_the_live_generation_falls_back_to_audio_bed() {
        // Defect A4: a live-synthesis provider failure must not kill the session. It degrades to
        // the audio-bed fallback and the connection stays open.
        let mut session = started();
        let request = speak(&mut session, "hello");
        let action = session.on_speech_outcome(SpeechOutcome::Failed {
            generation: request.generation,
            detail: "tts down".into(),
        });
        match action {
            Action::SendFallback(frames) => assert!(matches!(
                frames[0],
                ServerFrame::AudioBedFallback { .. }
            )),
            other => panic!("expected SendFallback, got {other:?}"),
        }
        assert!(session.is_degraded());
        assert!(!session.has_ended());
    }

    #[test]
    fn a_failed_synthesis_for_a_superseded_generation_does_not_close_the_session() {
        // A cancelled request erroring out on its way to the bin is not an outage. Closing here
        // would drop a live turn because of a turn the user already interrupted.
        let mut session = started();
        let first = speak(&mut session, "one");
        barge_in(&mut session);
        let action = session.on_speech_outcome(SpeechOutcome::Failed {
            generation: first.generation,
            detail: "connection reset by peer".into(),
        });
        assert_eq!(action, Action::Ignore);
        assert!(!session.has_ended());
        assert!(!session.is_degraded());
    }

    #[test]
    fn degraded_session_ignores_voice_work_but_allows_pause_to_close() {
        // Once degraded, the bed is preserved but no new voice work is accepted. Only an explicit
        // pause (§5) can tear the connection down.
        let mut session = started();
        session.on_audio_frame();
        session.on_stt_outcome(SttOutcome::Push(Err(SttFailure::Unavailable("stt down".into()))));
        assert!(session.is_degraded());

        assert_eq!(
            session.on_audio_frame(),
            Action::Ignore,
            "no new mic audio is accepted while degraded"
        );
        assert_eq!(
            session.handle_client_frame(ClientFrame::Speak {
                tenant_id: "t1".into(),
                session_id: "s1".into(),
                text: "still here".into(),
                voice_id: "v".into(),
                emotion: "warm".into(),
            }),
            Action::Ignore,
            "no new synthesis is requested while degraded"
        );

        let action = pause(&mut session);
        assert!(
            matches!(action, Action::SendAndClose(_)),
            "a pause from degraded must close, got {action:?}"
        );
        assert!(session.has_ended(), "pause ends the degraded session");
    }

    #[test]
    fn end_of_turn_dispatches_the_finalise_off_the_read_loop() {
        let mut session = started();
        assert_eq!(
            session.handle_client_frame(ClientFrame::EndOfTurn {
                tenant_id: "t1".into(),
                session_id: "s1".into(),
            }),
            Action::DispatchEndOfTurn {
                session_id: "s1".into()
            }
        );
        let action = session.on_stt_outcome(SttOutcome::EndOfTurn(Ok(Transcript {
            text: "final transcript".into(),
            is_final: true,
        })));
        assert_eq!(
            action,
            Action::Send(vec![ServerFrame::Transcript {
                tenant_id: "t1".into(),
                session_id: "s1".into(),
                text: "final transcript".into(),
                is_final: true,
            }])
        );
    }

    #[test]
    fn provider_error_maps_cancellation_apart_from_outage() {
        assert_eq!(
            SttFailure::from(ProviderError::Cancelled),
            SttFailure::Cancelled
        );
        assert_eq!(
            SttFailure::from(ProviderError::Unavailable("boom".into())),
            SttFailure::Unavailable("boom".into())
        );
        assert_eq!(
            SttFailure::from(ProviderError::TimedOut),
            SttFailure::Unavailable("provider timed out".into()),
            "a timeout must still close the session as a typed failure, not hang or be silent"
        );
    }
}
