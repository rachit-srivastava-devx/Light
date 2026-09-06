import Foundation

/// Production `SocketLike` transport backed by Apple's native `URLSessionWebSocketTask`
/// (part of Foundation/URLSession) — no third-party WebSocket dependency needed.
///
/// This is deliberately NOT covered by the unit test suite: doing so would require a live
/// `relay-rs`/`relay-py` endpoint (or a local TCP WebSocket echo server), which is out of scope
/// for this unit (see the task report's "honest gaps" section). `RelaySocketTests.swift` proves
/// the connection-lifecycle contract (open timeout, single bounded reconnect, queue-then-flush)
/// against the `SocketLike` protocol using a deterministic test double; this class only has to
/// satisfy that same protocol correctly, which is a much smaller, mechanical surface.
public final class URLSessionWebSocketTransport: NSObject, SocketLike {
    public var onMessage: ((SocketData) -> Void)?
    public var onOpen: (() -> Void)?
    public var onError: ((Error?) -> Void)?
    public var onClose: ((SocketCloseEvent?) -> Void)?

    public private(set) var readyState: SocketReadyState = .connecting

    private let session: URLSession
    private let task: URLSessionWebSocketTask
    private let delegateProxy = WebSocketDelegateProxy()

    public init(url: URL, urlSessionConfiguration: URLSessionConfiguration = .default) {
        self.session = URLSession(configuration: urlSessionConfiguration, delegate: delegateProxy, delegateQueue: nil)
        self.task = session.webSocketTask(with: url)
        super.init()

        delegateProxy.onOpen = { [weak self] in
            self?.readyState = .open
            self?.onOpen?()
        }
        delegateProxy.onClose = { [weak self] closeCode, reasonData in
            self?.readyState = .closed
            let reason = reasonData.flatMap { String(data: $0, encoding: .utf8) }
            self?.onClose?(SocketCloseEvent(code: closeCode.rawValue, reason: reason, wasClean: true))
        }

        task.resume()
        listen()
    }

    public func send(_ data: SocketData) {
        let message: URLSessionWebSocketTask.Message
        switch data {
        case .text(let string):
            message = .string(string)
        case .binary(let payload):
            message = .data(payload)
        }
        task.send(message) { [weak self] error in
            guard let error else { return }
            self?.onError?(error)
        }
    }

    public func close() {
        readyState = .closing
        task.cancel(with: .goingAway, reason: nil)
    }

    private func listen() {
        task.receive { [weak self] result in
            guard let self else { return }
            switch result {
            case .failure(let error):
                // A receive failure after a normal close is expected noise, not a real error.
                if self.readyState != .closed && self.readyState != .closing {
                    self.onError?(error)
                }
            case .success(let message):
                switch message {
                case .string(let text):
                    self.onMessage?(.text(text))
                case .data(let payload):
                    self.onMessage?(.binary(payload))
                @unknown default:
                    break
                }
                self.listen()
            }
        }
    }
}

/// `URLSessionWebSocketDelegate` must live on an `NSObject` the session retains; this proxy
/// forwards the two lifecycle callbacks `RelaySocket` actually needs (open/close) as plain
/// closures so `URLSessionWebSocketTransport` doesn't have to conform to the delegate itself.
private final class WebSocketDelegateProxy: NSObject, URLSessionWebSocketDelegate {
    var onOpen: (() -> Void)?
    var onClose: ((URLSessionWebSocketTask.CloseCode, Data?) -> Void)?

    func urlSession(
        _ session: URLSession,
        webSocketTask: URLSessionWebSocketTask,
        didOpenWithProtocol protocol: String?
    ) {
        onOpen?()
    }

    func urlSession(
        _ session: URLSession,
        webSocketTask: URLSessionWebSocketTask,
        didCloseWith closeCode: URLSessionWebSocketTask.CloseCode,
        reason: Data?
    ) {
        onClose?(closeCode, reason)
    }
}
