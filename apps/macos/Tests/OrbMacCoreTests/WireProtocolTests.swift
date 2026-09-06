import XCTest
@testable import OrbMacCore

/// Round-trip and shape tests for `ClientFrame`/`ServerFrame` against the exact wire protocol
/// defined in `backend/relay-rs/src/protocol.rs` (JSON tag = "type", snake_case fields).
///
/// These mirror the assertion style of `protocol.rs`'s own `#[cfg(test)] mod tests`: substring
/// checks on the encoded JSON for the tag/field names (JSON key order is not a contract), plus a
/// full decode -> re-encode -> equal round trip for every case.
final class WireProtocolTests: XCTestCase {
    private let decoder = JSONDecoder()
    private let encoder = JSONEncoder()

    // MARK: - ClientFrame

    func testStartListeningRoundTripsAndMustBeFirstFrameShape() throws {
        let frame = ClientFrame.startListening(tenantId: "t1", sessionId: "s1")
        let data = try encoder.encode(frame)
        let json = String(data: data, encoding: .utf8)!
        XCTAssertTrue(json.contains("\"type\":\"start_listening\""))
        XCTAssertTrue(json.contains("\"tenant_id\":\"t1\""))
        XCTAssertTrue(json.contains("\"session_id\":\"s1\""))
        let decoded = try decoder.decode(ClientFrame.self, from: data)
        XCTAssertEqual(decoded, frame)
    }

    func testEndOfTurnRoundTrips() throws {
        let frame = ClientFrame.endOfTurn(tenantId: "t1", sessionId: "s1")
        let data = try encoder.encode(frame)
        XCTAssertTrue(String(data: data, encoding: .utf8)!.contains("\"type\":\"end_of_turn\""))
        XCTAssertEqual(try decoder.decode(ClientFrame.self, from: data), frame)
    }

    func testSpeakRoundTripsWithAllFields() throws {
        let frame = ClientFrame.speak(
            tenantId: "t1", sessionId: "s1", text: "one down",
            voiceId: "orb.warm.v1", emotion: "celebratory"
        )
        let data = try encoder.encode(frame)
        let json = String(data: data, encoding: .utf8)!
        XCTAssertTrue(json.contains("\"type\":\"speak\""))
        XCTAssertTrue(json.contains("\"text\":\"one down\""))
        XCTAssertTrue(json.contains("\"voice_id\":\"orb.warm.v1\""))
        XCTAssertTrue(json.contains("\"emotion\":\"celebratory\""))
        XCTAssertEqual(try decoder.decode(ClientFrame.self, from: data), frame)
    }

    func testBargeInRoundTrips() throws {
        let frame = ClientFrame.bargeIn(tenantId: "t1", sessionId: "s1")
        let data = try encoder.encode(frame)
        XCTAssertTrue(String(data: data, encoding: .utf8)!.contains("\"type\":\"barge_in\""))
        XCTAssertEqual(try decoder.decode(ClientFrame.self, from: data), frame)
    }

    func testPauseRoundTrips() throws {
        let frame = ClientFrame.pause(tenantId: "t1", sessionId: "s1")
        let data = try encoder.encode(frame)
        XCTAssertTrue(String(data: data, encoding: .utf8)!.contains("\"type\":\"pause\""))
        XCTAssertEqual(try decoder.decode(ClientFrame.self, from: data), frame)
    }

    func testClientFrameDecodesRawJSONExactlyAsRelayRsWouldProduceIt() throws {
        // What a real relay-rs peer would never send (client -> server only), but decoding the
        // exact serde_json shape must still work symmetrically since both sides share one enum.
        let json = #"{"type":"barge_in","tenant_id":"tenant-9","session_id":"sess-9"}"#
        let decoded = try decoder.decode(ClientFrame.self, from: Data(json.utf8))
        XCTAssertEqual(decoded, .bargeIn(tenantId: "tenant-9", sessionId: "sess-9"))
    }

    func testUnknownClientFrameTypeIsRejectedRatherThanSilentlyIgnored() {
        let json = #"{"type":"teleport"}"#
        XCTAssertThrowsError(try decoder.decode(ClientFrame.self, from: Data(json.utf8)))
    }

    // MARK: - ServerFrame

    func testListeningConfirmedDecodesExactlyAsRelayRsWouldProduceIt() throws {
        let json = #"{"type":"listening_confirmed","tenant_id":"t1","session_id":"s1"}"#
        let decoded = try decoder.decode(ServerFrame.self, from: Data(json.utf8))
        XCTAssertEqual(decoded, .listeningConfirmed(tenantId: "t1", sessionId: "s1"))
    }

    func testTranscriptDecodesWithIsFinalFlag() throws {
        let json = #"{"type":"transcript","tenant_id":"t1","session_id":"s1","text":"hello","is_final":true}"#
        let decoded = try decoder.decode(ServerFrame.self, from: Data(json.utf8))
        XCTAssertEqual(decoded, .transcript(tenantId: "t1", sessionId: "s1", text: "hello", isFinal: true))
    }

    func testTranscriptRoundTripsWithIsFinalFalse() throws {
        let frame = ServerFrame.transcript(tenantId: "t1", sessionId: "s1", text: "partial", isFinal: false)
        let data = try encoder.encode(frame)
        XCTAssertTrue(String(data: data, encoding: .utf8)!.contains("\"is_final\":false"))
        XCTAssertEqual(try decoder.decode(ServerFrame.self, from: data), frame)
    }

    func testSpeechStartingRoundTrips() throws {
        let frame = ServerFrame.speechStarting(tenantId: "t1", sessionId: "s1")
        let data = try encoder.encode(frame)
        XCTAssertTrue(String(data: data, encoding: .utf8)!.contains("\"type\":\"speech_starting\""))
        XCTAssertEqual(try decoder.decode(ServerFrame.self, from: data), frame)
    }

    func testSpeechCompleteRoundTrips() throws {
        let frame = ServerFrame.speechComplete(tenantId: "t1", sessionId: "s1")
        let data = try encoder.encode(frame)
        XCTAssertTrue(String(data: data, encoding: .utf8)!.contains("\"type\":\"speech_complete\""))
        XCTAssertEqual(try decoder.decode(ServerFrame.self, from: data), frame)
    }

    func testAudioBedFallbackRoundTrips() throws {
        let frame = ServerFrame.audioBedFallback(tenantId: "t1", sessionId: "s1")
        let data = try encoder.encode(frame)
        XCTAssertTrue(String(data: data, encoding: .utf8)!.contains("\"type\":\"audio_bed_fallback\""))
        XCTAssertEqual(try decoder.decode(ServerFrame.self, from: data), frame)
    }

    func testClosingRoundTripsWithUserPauseReason() throws {
        let frame = ServerFrame.closing(tenantId: "t1", sessionId: "s1", reason: .userPause)
        let data = try encoder.encode(frame)
        let json = String(data: data, encoding: .utf8)!
        XCTAssertTrue(json.contains("\"type\":\"closing\""))
        XCTAssertTrue(json.contains("\"reason\":\"user_pause\""))
        XCTAssertEqual(try decoder.decode(ServerFrame.self, from: data), frame)
    }

    func testClosingRoundTripsWithProviderFailureReason() throws {
        let frame = ServerFrame.closing(tenantId: "t1", sessionId: "s1", reason: .providerFailure)
        let data = try encoder.encode(frame)
        XCTAssertTrue(String(data: data, encoding: .utf8)!.contains("\"reason\":\"provider_failure\""))
        XCTAssertEqual(try decoder.decode(ServerFrame.self, from: data), frame)
    }

    func testCloseReasonDistinguishesPauseFromFailure() {
        // These must not collapse: the presence plane's behaviour is opposite for each (protocol.rs §5).
        XCTAssertNotEqual(CloseReason.userPause, CloseReason.providerFailure)
    }

    func testUnknownServerFrameTypeIsRejectedRatherThanSilentlyIgnored() {
        let json = #"{"type":"teleport","tenant_id":"t1","session_id":"s1"}"#
        XCTAssertThrowsError(try decoder.decode(ServerFrame.self, from: Data(json.utf8)))
    }

    func testServerFrameMissingTypeFieldIsRejected() {
        let json = #"{"tenant_id":"t1","session_id":"s1"}"#
        XCTAssertThrowsError(try decoder.decode(ServerFrame.self, from: Data(json.utf8)))
    }
}
