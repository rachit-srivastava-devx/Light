//! Wire protocol between the mobile client and this relay.
//!
//! The client talks only to us; we talk to the STT/TTS providers (the audio-plane mirror of C9's
//! one-door rule). Frames are JSON control messages plus binary audio — audio never goes through
//! JSON, so a mic frame costs no base64 inflation on the hot path (docs/BUILD-DIGEST.md §3).

use serde::{Deserialize, Serialize};

/// Client -> relay control frames.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientFrame {
    /// Opens the STT stream for a session. Must be the first frame.
    StartListening {
        tenant_id: String,
        session_id: String,
    },
    /// The user stopped speaking (semantic endpointer fired on-device).
    EndOfTurn {
        tenant_id: String,
        session_id: String,
    },
    /// Speak this text. Text, not audio — TTS synthesis is ours to route.
    Speak {
        tenant_id: String,
        session_id: String,
        text: String,
        voice_id: String,
        emotion: String,
    },
    /// The user started speaking over the orb. Must yield within BARGE_IN_YIELD_BUDGET_MS.
    BargeIn {
        tenant_id: String,
        session_id: String,
    },
    /// User-intent pause (§5): background, screen lock, BT disconnect. Total and immediate —
    /// the socket closes, nothing is buffered, nothing further is billed.
    Pause {
        tenant_id: String,
        session_id: String,
    },
}

/// Relay -> client frames.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerFrame {
    /// Delivery receipt for `StartListening`: the relay has accepted the session and opened the
    /// STT stream. This is the only signal the client may treat as "the relay is actually
    /// listening" — local mic capture starting is not a substitute (a dead/unreachable relay can
    /// leave the mic open with nothing on the other end).
    ListeningConfirmed {
        tenant_id: String,
        session_id: String,
    },
    /// Streaming STT partial. `is_final` marks the endpointed transcript.
    Transcript {
        tenant_id: String,
        session_id: String,
        text: String,
        is_final: bool,
    },
    /// TTS audio is following on the binary channel. Sent before the first chunk so the client can
    /// duck the bed (§4: duck, never stop) before audio arrives rather than after.
    SpeechStarting {
        tenant_id: String,
        session_id: String,
    },
    /// TTS finished; the client restores the bed to nominal.
    SpeechComplete {
        tenant_id: String,
        session_id: String,
    },
    /// The relay is releasing this session. `reason` distinguishes a user-intent pause from a
    /// failure, because the two mean opposite things to the presence plane (§5): a pause is
    /// silence the user asked for, a failure must keep the bed alive and announce itself.
    Closing {
        tenant_id: String,
        session_id: String,
        reason: CloseReason,
    },
    /// The voice provider failed but the session stays alive. The client must keep the audio bed
    /// running and say so once (§5: presence > intelligence). The socket is NOT closed — the
    /// session continues in degraded mode so future reconnection or pause is possible.
    AudioBedFallback {
        tenant_id: String,
        session_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseReason {
    /// §5 pause contract: mic released, socket closed, nothing captured.
    UserPause,
    /// Provider or network failure. The client keeps the bed running and says so once.
    ProviderFailure,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_frames_round_trip_through_json() {
        let frame = ClientFrame::Speak {
            tenant_id: "t1".into(),
            session_id: "s1".into(),
            text: "one down".into(),
            voice_id: "orb.warm.v1".into(),
            emotion: "celebratory".into(),
        };
        let encoded = serde_json::to_string(&frame).expect("serializes");
        let decoded: ClientFrame = serde_json::from_str(&encoded).expect("deserializes");
        assert_eq!(frame, decoded);
    }

    #[test]
    fn frames_are_tagged_by_type_so_the_client_can_switch_on_it() {
        let json = serde_json::to_string(&ClientFrame::Pause {
            tenant_id: "t1".into(),
            session_id: "s1".into(),
        })
        .expect("serializes");
        assert!(json.contains("\"type\":\"pause\""));
    }

    #[test]
    fn unknown_frame_type_is_rejected_rather_than_silently_ignored() {
        let result: Result<ClientFrame, _> = serde_json::from_str(r#"{"type":"teleport"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn close_reason_distinguishes_pause_from_failure() {
        // These must not collapse: the presence plane's behaviour is opposite for each (§5).
        assert_ne!(CloseReason::UserPause, CloseReason::ProviderFailure);
    }

    #[test]
    fn audio_bed_fallback_frame_round_trips_through_json() {
        // The client must be able to deserialize the audio-bed fallback frame so it can keep
        // the bed alive and say so once (§5).
        let frame = ServerFrame::AudioBedFallback {
            tenant_id: "t1".into(),
            session_id: "s1".into(),
        };
        let encoded = serde_json::to_string(&frame).expect("serializes");
        assert!(encoded.contains("\"type\":\"audio_bed_fallback\""));
        let decoded: ServerFrame = serde_json::from_str(&encoded).expect("deserializes");
        assert_eq!(frame, decoded);
    }
}
