use futures_util::{SinkExt, StreamExt};
use the_controller_lib::daemon_gateway::{
    connect_daemon_websocket, is_allowed_daemon_stream_origin,
};
use tokio::{net::UnixListener, sync::oneshot};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{
        handshake::server::{Request, Response},
        Message,
    },
};

async fn ok() {}

#[test]
fn stream_paths_are_websocket_gateway_paths() {
    assert!(the_controller_lib::daemon_gateway::is_daemon_stream_path(
        "/api/daemon/chats/c1/stream"
    ));
    assert!(the_controller_lib::daemon_gateway::is_daemon_stream_path(
        "/api/daemon/sessions/s1/stream"
    ));
    assert!(!the_controller_lib::daemon_gateway::is_daemon_stream_path(
        "/api/daemon/chats"
    ));
    assert!(!the_controller_lib::daemon_gateway::is_daemon_stream_path(
        "/api/daemon/chats/c1/messages"
    ));
    assert!(!the_controller_lib::daemon_gateway::is_daemon_stream_path(
        "/api/daemon/other/c1/stream"
    ));
}

#[test]
fn explicit_stream_routes_can_coexist_with_daemon_catch_all() {
    let _router: axum::Router<()> = axum::Router::new()
        .route("/api/daemon/chats/{id}/stream", axum::routing::get(ok))
        .route("/api/daemon/sessions/{id}/stream", axum::routing::get(ok))
        .route("/api/daemon/{*path}", axum::routing::get(ok));
}

#[test]
fn stream_origin_policy_allows_native_and_same_origin_requests() {
    assert!(is_allowed_daemon_stream_origin(
        None,
        Some("localhost:3001")
    ));
    assert!(is_allowed_daemon_stream_origin(
        Some("http://localhost:3001"),
        Some("localhost:3001")
    ));
    assert!(is_allowed_daemon_stream_origin(
        Some("http://127.0.0.1:1934"),
        Some("127.0.0.1:1934")
    ));
}

#[test]
fn stream_origin_policy_rejects_foreign_or_malformed_origins() {
    assert!(!is_allowed_daemon_stream_origin(
        Some("https://evil.example"),
        Some("localhost:3001")
    ));
    assert!(!is_allowed_daemon_stream_origin(
        Some("not a url"),
        Some("localhost:3001")
    ));
    assert!(!is_allowed_daemon_stream_origin(
        Some("http://localhost:1420"),
        None
    ));
    assert!(!is_allowed_daemon_stream_origin(
        Some("http://localhost:1420"),
        Some("localhost:3001")
    ));
    assert!(!is_allowed_daemon_stream_origin(
        Some("http://evil.example:3001"),
        Some("evil.example:3001")
    ));
}

#[tokio::test]
async fn daemon_websocket_connects_over_unix_socket_and_preserves_query() {
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("daemon.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let (target_tx, target_rx) = oneshot::channel();

    let server = tokio::spawn(async move {
        let (stream, _addr) = listener.accept().await.unwrap();
        let mut ws = accept_hdr_async(stream, move |request: &Request, response: Response| {
            let _ = target_tx.send(request.uri().to_string());
            Ok(response)
        })
        .await
        .unwrap();

        let msg = ws.next().await.unwrap().unwrap();
        assert_eq!(msg, Message::Text("hello".into()));
        ws.send(Message::Text("ack".into())).await.unwrap();
    });

    let mut ws = connect_daemon_websocket(
        &socket,
        "/sessions/s1/stream?since=5&channels=stdout%2Cstderr",
    )
    .await
    .unwrap();

    ws.send(Message::Text("hello".into())).await.unwrap();
    let reply = ws.next().await.unwrap().unwrap();

    assert_eq!(
        target_rx.await.unwrap(),
        "/sessions/s1/stream?since=5&channels=stdout%2Cstderr"
    );
    assert_eq!(reply, Message::Text("ack".into()));
    server.await.unwrap();
}
