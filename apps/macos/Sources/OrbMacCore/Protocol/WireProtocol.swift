import Foundation

/// Wire protocol between this client and `relay-rs` / `relay-py`.
///
/// This is a byte-for-byte mirror of `backend/relay-rs/src/protocol.rs`'s `ClientFrame`/
/// `ServerFrame`/`CloseReason` (JSON tag = "type", snake_case field names). Swift's `Codable`
/// does not do tagged unions natively, so each enum implements `init(from:)`/`encode(to:)` by
/// hand, switching on a decoded "type" string — an unrecognized tag throws a `DecodingError`
/// rather than being silently dropped, matching `protocol.rs`'s own
/// `unknown_frame_type_is_rejected_rather_than_silently_ignored` test.
///
/// Binary frames (mic audio in, TTS audio out) carry no JSON envelope at all and are not
/// represented here — see `SocketData.binary` in `RelaySocket.swift`.

// MARK: - ClientFrame

public enum ClientFrame: Equatable {
    /// Opens the STT stream for a session. Must be the first frame sent.
    case startListening(tenantId: String, sessionId: String)
    /// The user stopped speaking (semantic endpointer fired on-device).
    case endOfTurn(tenantId: String, sessionId: String)
    /// Speak this text. Text, not audio — TTS synthesis is the relay's to route.
    case speak(tenantId: String, sessionId: String, text: String, voiceId: String, emotion: String)
    /// The user started speaking over the orb.
    case bargeIn(tenantId: String, sessionId: String)
    /// User-intent pause: background, screen lock, BT disconnect.
    case pause(tenantId: String, sessionId: String)
}

extension ClientFrame: Codable {
    private enum CodingKeys: String, CodingKey {
        case type
        case tenantId = "tenant_id"
        case sessionId = "session_id"
        case text
        case voiceId = "voice_id"
        case emotion
    }

    /// `Decodable` conformance on the raw string is what makes an unrecognized "type" throw a
    /// `DecodingError` instead of silently falling through.
    private enum FrameType: String, Codable {
        case startListening = "start_listening"
        case endOfTurn = "end_of_turn"
        case speak
        case bargeIn = "barge_in"
        case pause
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(FrameType.self, forKey: .type)
        let tenantId = try container.decode(String.self, forKey: .tenantId)
        let sessionId = try container.decode(String.self, forKey: .sessionId)
        switch type {
        case .startListening:
            self = .startListening(tenantId: tenantId, sessionId: sessionId)
        case .endOfTurn:
            self = .endOfTurn(tenantId: tenantId, sessionId: sessionId)
        case .speak:
            let text = try container.decode(String.self, forKey: .text)
            let voiceId = try container.decode(String.self, forKey: .voiceId)
            let emotion = try container.decode(String.self, forKey: .emotion)
            self = .speak(tenantId: tenantId, sessionId: sessionId, text: text, voiceId: voiceId, emotion: emotion)
        case .bargeIn:
            self = .bargeIn(tenantId: tenantId, sessionId: sessionId)
        case .pause:
            self = .pause(tenantId: tenantId, sessionId: sessionId)
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .startListening(tenantId, sessionId):
            try container.encode(FrameType.startListening, forKey: .type)
            try container.encode(tenantId, forKey: .tenantId)
            try container.encode(sessionId, forKey: .sessionId)
        case let .endOfTurn(tenantId, sessionId):
            try container.encode(FrameType.endOfTurn, forKey: .type)
            try container.encode(tenantId, forKey: .tenantId)
            try container.encode(sessionId, forKey: .sessionId)
        case let .speak(tenantId, sessionId, text, voiceId, emotion):
            try container.encode(FrameType.speak, forKey: .type)
            try container.encode(tenantId, forKey: .tenantId)
            try container.encode(sessionId, forKey: .sessionId)
            try container.encode(text, forKey: .text)
            try container.encode(voiceId, forKey: .voiceId)
            try container.encode(emotion, forKey: .emotion)
        case let .bargeIn(tenantId, sessionId):
            try container.encode(FrameType.bargeIn, forKey: .type)
            try container.encode(tenantId, forKey: .tenantId)
            try container.encode(sessionId, forKey: .sessionId)
        case let .pause(tenantId, sessionId):
            try container.encode(FrameType.pause, forKey: .type)
            try container.encode(tenantId, forKey: .tenantId)
            try container.encode(sessionId, forKey: .sessionId)
        }
    }
}

// MARK: - ServerFrame

public enum ServerFrame: Equatable {
    /// Delivery receipt for `StartListening`: the relay has accepted the session and opened the
    /// STT stream. The only signal the client may treat as "the relay is actually listening."
    case listeningConfirmed(tenantId: String, sessionId: String)
    /// Streaming STT partial. `isFinal` marks the endpointed transcript.
    case transcript(tenantId: String, sessionId: String, text: String, isFinal: Bool)
    /// TTS audio is following on the binary channel.
    case speechStarting(tenantId: String, sessionId: String)
    /// TTS finished; the client restores the bed to nominal.
    case speechComplete(tenantId: String, sessionId: String)
    /// The relay is releasing this session. `reason` distinguishes a user-intent pause from a
    /// provider failure.
    case closing(tenantId: String, sessionId: String, reason: CloseReason)
    /// The voice provider failed but the session stays alive; the client must keep the audio bed
    /// running and say so once. The socket is NOT closed for this frame.
    case audioBedFallback(tenantId: String, sessionId: String)
}

public enum CloseReason: String, Codable, Equatable {
    /// Pause contract: mic released, socket closed, nothing captured.
    case userPause = "user_pause"
    /// Provider or network failure. The client keeps the bed running and says so once.
    case providerFailure = "provider_failure"
}

extension ServerFrame: Codable {
    private enum CodingKeys: String, CodingKey {
        case type
        case tenantId = "tenant_id"
        case sessionId = "session_id"
        case text
        case isFinal = "is_final"
        case reason
    }

    private enum FrameType: String, Codable {
        case listeningConfirmed = "listening_confirmed"
        case transcript
        case speechStarting = "speech_starting"
        case speechComplete = "speech_complete"
        case closing
        case audioBedFallback = "audio_bed_fallback"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(FrameType.self, forKey: .type)
        let tenantId = try container.decode(String.self, forKey: .tenantId)
        let sessionId = try container.decode(String.self, forKey: .sessionId)
        switch type {
        case .listeningConfirmed:
            self = .listeningConfirmed(tenantId: tenantId, sessionId: sessionId)
        case .transcript:
            let text = try container.decode(String.self, forKey: .text)
            let isFinal = try container.decode(Bool.self, forKey: .isFinal)
            self = .transcript(tenantId: tenantId, sessionId: sessionId, text: text, isFinal: isFinal)
        case .speechStarting:
            self = .speechStarting(tenantId: tenantId, sessionId: sessionId)
        case .speechComplete:
            self = .speechComplete(tenantId: tenantId, sessionId: sessionId)
        case .closing:
            let reason = try container.decode(CloseReason.self, forKey: .reason)
            self = .closing(tenantId: tenantId, sessionId: sessionId, reason: reason)
        case .audioBedFallback:
            self = .audioBedFallback(tenantId: tenantId, sessionId: sessionId)
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .listeningConfirmed(tenantId, sessionId):
            try container.encode(FrameType.listeningConfirmed, forKey: .type)
            try container.encode(tenantId, forKey: .tenantId)
            try container.encode(sessionId, forKey: .sessionId)
        case let .transcript(tenantId, sessionId, text, isFinal):
            try container.encode(FrameType.transcript, forKey: .type)
            try container.encode(tenantId, forKey: .tenantId)
            try container.encode(sessionId, forKey: .sessionId)
            try container.encode(text, forKey: .text)
            try container.encode(isFinal, forKey: .isFinal)
        case let .speechStarting(tenantId, sessionId):
            try container.encode(FrameType.speechStarting, forKey: .type)
            try container.encode(tenantId, forKey: .tenantId)
            try container.encode(sessionId, forKey: .sessionId)
        case let .speechComplete(tenantId, sessionId):
            try container.encode(FrameType.speechComplete, forKey: .type)
            try container.encode(tenantId, forKey: .tenantId)
            try container.encode(sessionId, forKey: .sessionId)
        case let .closing(tenantId, sessionId, reason):
            try container.encode(FrameType.closing, forKey: .type)
            try container.encode(tenantId, forKey: .tenantId)
            try container.encode(sessionId, forKey: .sessionId)
            try container.encode(reason, forKey: .reason)
        case let .audioBedFallback(tenantId, sessionId):
            try container.encode(FrameType.audioBedFallback, forKey: .type)
            try container.encode(tenantId, forKey: .tenantId)
            try container.encode(sessionId, forKey: .sessionId)
        }
    }
}
