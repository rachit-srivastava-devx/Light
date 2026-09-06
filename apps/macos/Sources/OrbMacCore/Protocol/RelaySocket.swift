import Foundation

/// Client-side connection lifecycle wrapper for the realtime relay WebSocket, mirroring
/// `apps/mobile/src/voice/RelaySocket.ts` (fixed and verified for defect A3 this session):
///
///   - Open timeout defaults to 10000ms; a stalled open surfaces a typed `RelayOpenTimeoutError`
///     instead of hanging forever, and tears down the stalled transport.
///   - Exactly ONE bounded reconnect attempt on an unexpected close (never a retry storm), with a
///     ~250ms default backoff — and only if the socket had previously opened successfully AND the
///     close was not user-initiated (`close()`).
///   - Frames sent while still connecting are queued and flushed in send order once open, rather
///     than thrown away or dropped.
///
/// `RelaySocket` is transport-agnostic: it drives anything conforming to `SocketLike`, so tests
/// inject a deterministic test double (see `FakeSocket` in `RelaySocketTests.swift`) instead of a
/// real network socket. Production wiring uses `URLSessionWebSocketTransport`.

// MARK: - Transport abstraction

public enum SocketReadyState {
    case connecting
    case open
    case closing
    case closed
}

public struct SocketCloseEvent {
    public let code: Int?
    public let reason: String?
    public let wasClean: Bool?

    public init(code: Int? = nil, reason: String? = nil, wasClean: Bool? = nil) {
        self.code = code
        self.reason = reason
        self.wasClean = wasClean
    }
}

public enum SocketData: Equatable {
    case text(String)
    case binary(Data)
}

/// The minimal transport surface `RelaySocket` needs. A real WebSocket implementation
/// (`URLSessionWebSocketTransport`) and a deterministic test double both conform to this.
public protocol SocketLike: AnyObject {
    var onMessage: ((SocketData) -> Void)? { get set }
    var onOpen: (() -> Void)? { get set }
    var onError: ((Error?) -> Void)? { get set }
    var onClose: ((SocketCloseEvent?) -> Void)? { get set }
    var readyState: SocketReadyState { get }

    func send(_ data: SocketData)
    func close()
}

// MARK: - Errors

public class RelayTransportError: Error, CustomStringConvertible {
    public let message: String
    public let state: RelaySocketState
    public let cause: Error?

    public init(_ message: String, state: RelaySocketState, cause: Error? = nil) {
        self.message = message
        self.state = state
        self.cause = cause
    }

    public var description: String { message }
}

/// The native socket stayed CONNECTING past its explicit opening deadline.
public final class RelayOpenTimeoutError: RelayTransportError {
    public let timeoutMs: Int

    public init(timeoutMs: Int) {
        self.timeoutMs = timeoutMs
        super.init("Relay socket open timeout after \(timeoutMs)ms", state: .failed)
    }
}

/// A previously-open voice session lost its transport without an explicit local pause.
public final class RelayUnexpectedCloseError: RelayTransportError {
    public let reconnectScheduled: Bool

    public init(event: SocketCloseEvent?, reconnectScheduled: Bool, cause: Error? = nil) {
        self.reconnectScheduled = reconnectScheduled
        let detail = event?.reason
        super.init(
            "Relay socket closed unexpectedly" + (detail.map { ": \($0)" } ?? ""),
            state: .closed,
            cause: cause
        )
    }
}

// MARK: - RelaySocket

public enum RelaySocketState: String, Equatable {
    case connecting, open, failed, closed
}

public struct RelaySocketDiagnostic {
    public let event: String
    public let state: RelaySocketState
    public let queuedFrames: Int
    public let error: String?
    public let reconnectAttempt: Int?
    public let backoffMs: Int?
}

public struct RelaySocketOptions {
    public var openTimeoutMs: Int
    /// Creates the replacement transport for the single allowed reconnect attempt. Reconnect is
    /// disabled entirely (never attempted) when this is nil.
    public var reconnectFactory: (() -> SocketLike)?
    public var reconnectDelayMs: Int
    public var onDiagnostic: ((RelaySocketDiagnostic) -> Void)?

    public init(
        openTimeoutMs: Int = 10_000,
        reconnectFactory: (() -> SocketLike)? = nil,
        reconnectDelayMs: Int = 250,
        onDiagnostic: ((RelaySocketDiagnostic) -> Void)? = nil
    ) {
        self.openTimeoutMs = openTimeoutMs
        self.reconnectFactory = reconnectFactory
        self.reconnectDelayMs = reconnectDelayMs
        self.onDiagnostic = onDiagnostic
    }
}

/// A cancellable handle around `DispatchQueue.main.asyncAfter`, since `Timer` requires a run loop
/// that isn't guaranteed to be pumping in every context this runs in.
private final class CancellableTimer {
    private var cancelled = false
    func cancel() { cancelled = true }
    func schedule(afterMs ms: Int, _ block: @escaping () -> Void) {
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(ms)) { [weak self] in
            guard let self, !self.cancelled else { return }
            block()
        }
    }
}

public final class RelaySocket {
    public var onMessage: ((SocketData) -> Void)?
    public var onOpen: (() -> Void)?
    public var onError: ((Error) -> Void)?
    public var onClose: ((SocketCloseEvent?) -> Void)?

    public private(set) var state: RelaySocketState

    /// Exposed for tests only: the current underlying transport, so a test can drive the
    /// *replacement* socket's lifecycle after a reconnect swaps it in.
    public var transportForTesting: SocketLike { socket }

    private var socket: SocketLike
    private let options: RelaySocketOptions
    private var queue: [SocketData] = []
    private var openTimer: CancellableTimer?
    private var reconnectTimer: CancellableTimer?
    private var closeNotified = false
    private var openedAtLeastOnce = false
    private var reconnectAttempted = false
    private var intentionallyClosed = false

    public init(socket: SocketLike, options: RelaySocketOptions = RelaySocketOptions()) {
        self.socket = socket
        self.options = options
        self.state = RelaySocket.initialState(socket.readyState)
        self.openedAtLeastOnce = (state == .open)
        bind(socket)

        if state == .connecting {
            emit(event: "connecting")
            scheduleOpenTimer()
        } else if state == .open {
            emit(event: "open")
        }
    }

    private static func initialState(_ readyState: SocketReadyState) -> RelaySocketState {
        switch readyState {
        case .open: return .open
        case .connecting: return .connecting
        case .closing, .closed: return .closed
        }
    }

    /// Sends now if open; queues (in order) if still connecting; throws a typed error otherwise.
    public func send(_ data: SocketData) throws {
        if state == .connecting {
            queue.append(data)
            emit(event: "send_queued")
            return
        }
        guard state == .open else {
            let error = RelayTransportError("Cannot send on relay socket in \(state.rawValue) state", state: state)
            emit(event: "send_failed", error: error.message)
            throw error
        }
        socket.send(data)
        emit(event: "send_accepted")
    }

    /// User-initiated close. Terminal: never schedules a reconnect, regardless of prior state.
    public func close() {
        guard state != .closed else { return }
        intentionallyClosed = true
        clearOpenTimer()
        clearReconnectTimer()
        queue.removeAll()
        state = .closed
        notifyClose(nil)
        unbind(socket)
        socket.close()
    }

    // MARK: - Transport event handlers

    private func handleOpen() {
        guard state == .connecting else { return }
        clearOpenTimer()
        state = .open
        openedAtLeastOnce = true
        emit(event: reconnectAttempted ? "reconnected" : "open", reconnectAttempt: reconnectAttempted ? 1 : nil)
        onOpen?()

        let queued = queue
        queue.removeAll()
        for pending in queued {
            socket.send(pending)
            emit(event: "send_accepted")
        }
    }

    private func handleError(_ error: Error?) {
        guard state != .closed else { return }
        if canReconnect() {
            scheduleReconnect(closeEvent: nil, closeSocket: true, cause: error)
            return
        }
        markFailed(RelayTransportError(
            "Relay socket error" + (error.map { ": \(describe($0))" } ?? ""),
            state: .failed,
            cause: error
        ))
    }

    private func handleClose(_ event: SocketCloseEvent?) {
        clearOpenTimer()
        if canReconnect() {
            scheduleReconnect(closeEvent: event, closeSocket: false, cause: nil)
            return
        }
        if state == .connecting {
            markFailed(
                RelayTransportError("Relay socket closed before opening", state: .failed),
                closeSocket: false
            )
        } else if state == .open {
            let error = RelayUnexpectedCloseError(event: event, reconnectScheduled: false)
            surfaceError(error)
            state = .closed
            queue.removeAll()
        }
        notifyClose(event)
    }

    private func handleOpenTimeout() {
        guard state == .connecting else { return }
        let error = RelayOpenTimeoutError(timeoutMs: options.openTimeoutMs)
        emit(event: "open_timeout", error: error.message)
        markFailed(error)
    }

    // MARK: - Reconnect

    /// Reconnect only if: not a user-initiated close, the socket had opened at least once before,
    /// no reconnect has been attempted yet (bounded to exactly one), a factory is configured, and
    /// the transport was actually open (not still mid-handshake, which fails outright instead).
    private func canReconnect() -> Bool {
        !intentionallyClosed
            && openedAtLeastOnce
            && !reconnectAttempted
            && options.reconnectFactory != nil
            && state == .open
    }

    private func scheduleReconnect(closeEvent: SocketCloseEvent?, closeSocket: Bool, cause: Error?) {
        let failedSocket = socket
        clearOpenTimer()
        unbind(failedSocket)
        state = .connecting

        let error = RelayUnexpectedCloseError(event: closeEvent, reconnectScheduled: true, cause: cause)
        surfaceError(error)
        let backoffMs = options.reconnectDelayMs
        emit(event: "reconnect_scheduled", error: error.message, reconnectAttempt: 1, backoffMs: backoffMs)

        if closeSocket {
            failedSocket.close()
        }

        let timer = CancellableTimer()
        reconnectTimer = timer
        timer.schedule(afterMs: backoffMs) { [weak self] in self?.attemptReconnect() }
    }

    private func attemptReconnect() {
        reconnectTimer = nil
        guard !intentionallyClosed, state == .connecting else { return }
        reconnectAttempted = true
        emit(event: "reconnect_attempt", reconnectAttempt: 1)

        guard let factory = options.reconnectFactory else { return }
        let replacement = factory()
        socket = replacement
        bind(replacement)

        switch replacement.readyState {
        case .connecting:
            state = .connecting
            scheduleOpenTimer()
        case .open:
            // Route an already-open replacement (e.g. a test double, or a socket that opened
            // synchronously) through the same "reconnected" open path.
            state = .connecting
            handleOpen()
        case .closing, .closed:
            markFailed(
                RelayTransportError("Relay reconnect socket was already closed", state: .failed),
                closeSocket: false
            )
            notifyClose(nil)
        }
    }

    // MARK: - Shared teardown

    private func markFailed(_ error: RelayTransportError, closeSocket: Bool = true) {
        guard state != .failed, state != .closed else { return }
        clearOpenTimer()
        queue.removeAll()
        state = .failed
        emit(event: "error", error: error.message)
        onError?(error)
        if closeSocket {
            socket.close()
        }
    }

    private func notifyClose(_ event: SocketCloseEvent?) {
        guard !closeNotified else { return }
        closeNotified = true
        emit(event: "closed")
        onClose?(event)
    }

    private func surfaceError(_ error: RelayTransportError) {
        emit(event: "error", error: error.message)
        onError?(error)
    }

    // MARK: - Binding / timers / diagnostics

    private func bind(_ target: SocketLike) {
        target.onMessage = { [weak self, weak target] data in
            guard let self, let target, self.socket === target else { return }
            self.onMessage?(data)
        }
        target.onOpen = { [weak self, weak target] in
            guard let self, let target, self.socket === target else { return }
            self.handleOpen()
        }
        target.onError = { [weak self, weak target] error in
            guard let self, let target, self.socket === target else { return }
            self.handleError(error)
        }
        target.onClose = { [weak self, weak target] event in
            guard let self, let target, self.socket === target else { return }
            self.handleClose(event)
        }
    }

    private func unbind(_ target: SocketLike) {
        target.onMessage = nil
        target.onOpen = nil
        target.onError = nil
        target.onClose = nil
    }

    private func scheduleOpenTimer() {
        let timer = CancellableTimer()
        openTimer = timer
        timer.schedule(afterMs: options.openTimeoutMs) { [weak self] in self?.handleOpenTimeout() }
    }

    private func clearOpenTimer() {
        openTimer?.cancel()
        openTimer = nil
    }

    private func clearReconnectTimer() {
        reconnectTimer?.cancel()
        reconnectTimer = nil
    }

    private func emit(
        event: String,
        error: String? = nil,
        reconnectAttempt: Int? = nil,
        backoffMs: Int? = nil
    ) {
        let diagnostic = RelaySocketDiagnostic(
            event: event,
            state: state,
            queuedFrames: queue.count,
            error: error,
            reconnectAttempt: reconnectAttempt,
            backoffMs: backoffMs
        )
        options.onDiagnostic?(diagnostic)
    }
}

private func describe(_ error: Error) -> String {
    (error as? RelayTransportError)?.message ?? String(describing: error)
}
