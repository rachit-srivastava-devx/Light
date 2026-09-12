//! Lazy axum app bring-up on `127.0.0.1:0`, SSE + WS routes. Started once, on first `deliver`.

use std::convert::Infallible;
use std::net::SocketAddr;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use super::broadcast::EventBus;

/// Binds an ephemeral localhost port and spawns the axum server, returning its address. Never a
/// resident daemon: torn down with the task that owns it when the pump stops.
pub(super) fn spawn(bus: EventBus) -> Result<SocketAddr, std::io::Error> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let addr = listener.local_addr()?;
    let listener = tokio::net::TcpListener::from_std(listener)?;
    let app = Router::new()
        .route("/events/sse", get(sse_handler))
        .route("/events/ws", get(ws_handler))
        .with_state(bus);
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok(addr)
}

async fn sse_handler(
    State(bus): State<EventBus>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let stream = BroadcastStream::new(bus.subscribe()).filter_map(|item| {
        let event = item.ok()?;
        let json = serde_json::to_string(&event.0).ok()?;
        Some(Ok(Event::default().data(json)))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn ws_handler(ws: WebSocketUpgrade, State(bus): State<EventBus>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_stream(socket, bus))
}

async fn ws_stream(mut socket: WebSocket, bus: EventBus) {
    let mut rx = bus.subscribe();
    while let Ok(event) = rx.recv().await {
        let Ok(json) = serde_json::to_string(&event.0) else {
            continue;
        };
        if socket.send(Message::Text(json.into())).await.is_err() {
            break;
        }
    }
}
