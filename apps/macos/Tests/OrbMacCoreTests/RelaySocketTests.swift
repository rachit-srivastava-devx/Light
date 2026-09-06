import XCTest
@testable import OrbMacCore

/// A controllable test double standing in for a real transport (URLSessionWebSocketTransport in
/// production). Tests drive its lifecycle explicitly (`triggerOpen()`, `triggerClose()`, ...)
/// rather than touching a real socket, so these are deterministic and need no network / relay.
final class FakeSocket: SocketLike {
    var onMessage: ((SocketData) -> Void)?
    var onOpen: (() -> Void)?
    var onError: ((Error?) -> Void)?
    var onClose: ((SocketCloseEvent?) -> Void)?
    private(set) var readyState: SocketReadyState
    private(set) var sent: [SocketData] = []
    private(set) var closeCallCount = 0

    init(readyState: SocketReadyState = .connecting) {
        self.readyState = readyState
    }

    func send(_ data: SocketData) {
        sent.append(data)
    }

    func close() {
        closeCallCount += 1
        readyState = .closed
    }

    func triggerOpen() {
        readyState = .open
        onOpen?()
    }

    func triggerClose(_ event: SocketCloseEvent? = nil) {
        readyState = .closed
        onClose?(event)
    }
}

final class RelaySocketTests: XCTestCase {
    // MARK: - Open timeout

    func testOpenTimeoutSurfacesATypedErrorInsteadOfHangingForever() {
        let fake = FakeSocket(readyState: .connecting)
        let expectation = expectation(description: "onerror fires with RelayOpenTimeoutError")
        let relay = RelaySocket(socket: fake, options: RelaySocketOptions(openTimeoutMs: 30))
        relay.onError = { error in
            XCTAssertTrue(error is RelayOpenTimeoutError)
            XCTAssertEqual((error as? RelayOpenTimeoutError)?.timeoutMs, 30)
            expectation.fulfill()
        }
        wait(for: [expectation], timeout: 1.0)
        XCTAssertEqual(relay.state, .failed)
        // The stalled transport must be torn down, not left open in the background.
        XCTAssertEqual(fake.closeCallCount, 1)
    }

    func testOpenBeforeTimeoutDoesNotFireTheTimeoutError() {
        let fake = FakeSocket(readyState: .connecting)
        let relay = RelaySocket(socket: fake, options: RelaySocketOptions(openTimeoutMs: 200))
        var sawTimeout = false
        relay.onError = { error in
            if error is RelayOpenTimeoutError { sawTimeout = true }
        }
        fake.triggerOpen()
        let notTooSoon = expectation(description: "wait past the original timeout window")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) { notTooSoon.fulfill() }
        wait(for: [notTooSoon], timeout: 1.0)
        XCTAssertFalse(sawTimeout)
        XCTAssertEqual(relay.state, .open)
    }

    // MARK: - Queue while connecting, flush on open

    func testFramesSentWhileConnectingQueueAndFlushInOrderOnceOpen() throws {
        let fake = FakeSocket(readyState: .connecting)
        let relay = RelaySocket(socket: fake, options: RelaySocketOptions(openTimeoutMs: 5_000))

        try relay.send(.text("first"))
        try relay.send(.text("second"))
        try relay.send(.binary(Data([0x01, 0x02])))

        // Nothing reaches the real transport before open — queued, not dropped or thrown.
        XCTAssertEqual(fake.sent.count, 0)

        fake.triggerOpen()

        XCTAssertEqual(fake.sent.count, 3)
        guard case .text(let a) = fake.sent[0], case .text(let b) = fake.sent[1],
              case .binary(let c) = fake.sent[2] else {
            return XCTFail("expected text, text, binary in original send order")
        }
        XCTAssertEqual(a, "first")
        XCTAssertEqual(b, "second")
        XCTAssertEqual(c, Data([0x01, 0x02]))
    }

    func testSendOnceOpenGoesStraightThroughWithoutQueueing() throws {
        let fake = FakeSocket(readyState: .open)
        let relay = RelaySocket(socket: fake)
        try relay.send(.text("hello"))
        XCTAssertEqual(fake.sent.count, 1)
    }

    // MARK: - Single bounded reconnect

    func testUnexpectedCloseAfterOpenGetsExactlyOneBoundedReconnectAttempt() {
        let fake1 = FakeSocket(readyState: .connecting)
        var factoryCallCount = 0
        let fake2 = FakeSocket(readyState: .connecting)
        let relay = RelaySocket(
            socket: fake1,
            options: RelaySocketOptions(
                openTimeoutMs: 5_000,
                reconnectFactory: {
                    factoryCallCount += 1
                    return fake2
                },
                reconnectDelayMs: 20
            )
        )
        fake1.triggerOpen()
        XCTAssertEqual(relay.state, .open)

        fake1.triggerClose(SocketCloseEvent(code: 1006, reason: "lost", wasClean: false))

        let reconnected = expectation(description: "reconnect factory invoked once and new socket opens")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            XCTAssertEqual(factoryCallCount, 1)
            XCTAssertEqual(relay.state, .connecting)
            fake2.triggerOpen()
            XCTAssertEqual(relay.state, .open)
            reconnected.fulfill()
        }
        wait(for: [reconnected], timeout: 1.0)
    }

    func testASecondUnexpectedCloseNeverTriggersASecondReconnectAttempt() {
        let fake1 = FakeSocket(readyState: .connecting)
        var factoryCallCount = 0
        let relay = RelaySocket(
            socket: fake1,
            options: RelaySocketOptions(
                openTimeoutMs: 5_000,
                reconnectFactory: {
                    factoryCallCount += 1
                    return FakeSocket(readyState: .open)
                },
                reconnectDelayMs: 10
            )
        )
        fake1.triggerOpen()
        fake1.triggerClose(SocketCloseEvent(code: 1006, reason: "first drop", wasClean: false))

        let firstReconnectDone = expectation(description: "first reconnect completes and opens")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            XCTAssertEqual(factoryCallCount, 1)
            XCTAssertEqual(relay.state, .open)
            firstReconnectDone.fulfill()
        }
        wait(for: [firstReconnectDone], timeout: 1.0)

        // The reconnected socket also drops. This must NOT trigger a second reconnect
        // (never a retry storm) — the socket becomes terminally closed instead.
        let finalClose = expectation(description: "onclose fires once, no second reconnect")
        relay.onClose = { _ in finalClose.fulfill() }
        (relay.transportForTesting as? FakeSocket)?.triggerClose(SocketCloseEvent(code: 1006, reason: "second drop", wasClean: false))
        wait(for: [finalClose], timeout: 1.0)

        XCTAssertEqual(factoryCallCount, 1, "must be exactly one bounded reconnect attempt, never a retry storm")
        XCTAssertEqual(relay.state, .closed)
    }

    func testReconnectNeverAttemptedIfTheSocketHadNeverPreviouslyOpened() {
        let fake = FakeSocket(readyState: .connecting)
        var factoryCallCount = 0
        let relay = RelaySocket(
            socket: fake,
            options: RelaySocketOptions(
                openTimeoutMs: 5_000,
                reconnectFactory: {
                    factoryCallCount += 1
                    return FakeSocket(readyState: .open)
                },
                reconnectDelayMs: 10
            )
        )
        // Never opened once. A close now must be terminal, not reconnected.
        fake.triggerClose(SocketCloseEvent(code: 1006, reason: "never opened", wasClean: false))

        let terminal = expectation(description: "settle")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { terminal.fulfill() }
        wait(for: [terminal], timeout: 1.0)

        XCTAssertEqual(factoryCallCount, 0)
        XCTAssertEqual(relay.state, .failed)
    }

    func testUserInitiatedCloseNeverTriggersAReconnect() {
        let fake = FakeSocket(readyState: .connecting)
        var factoryCallCount = 0
        let relay = RelaySocket(
            socket: fake,
            options: RelaySocketOptions(
                openTimeoutMs: 5_000,
                reconnectFactory: {
                    factoryCallCount += 1
                    return FakeSocket(readyState: .open)
                },
                reconnectDelayMs: 10
            )
        )
        fake.triggerOpen()
        relay.close()

        let settled = expectation(description: "settle past the backoff window")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { settled.fulfill() }
        wait(for: [settled], timeout: 1.0)

        XCTAssertEqual(factoryCallCount, 0, "a user-initiated close must never schedule a reconnect")
        XCTAssertEqual(relay.state, .closed)
        XCTAssertEqual(fake.closeCallCount, 1)
    }
}
