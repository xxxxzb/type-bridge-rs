use axum::body::Body;
use axum::http::StatusCode;
use axum::middleware;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::Deserialize;
use socketioxide::extract::{Data, SocketRef};
use socketioxide::SocketIo;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

#[derive(Clone)]
pub struct AppState {
    pub token: String,
    pub history: Arc<Mutex<VecDeque<String>>>,
}

static SOCKET_IO_JS: &str = include_str!("socket.io.min.js");

#[derive(Deserialize)]
struct TypeTextPayload {
    text: String,
}

#[derive(Deserialize)]
struct PressKeyPayload {
    key: String,
}

const HISTORY_MAX: usize = 30;

fn add_to_history(entries: &mut VecDeque<String>, text: &str, max: usize) -> bool {
    let text = text.trim();
    if text.is_empty() {
        return false;
    }
    if entries.front().is_some_and(|last| last == text) {
        return false;
    }
    if entries.len() >= max {
        entries.pop_back();
    }
    entries.push_front(text.to_string());
    true
}

/// Pure token validation from raw query string.
pub(crate) fn check_token(query: &str, valid: &str) -> bool {
    query
        .split('&')
        .filter_map(|p| p.strip_prefix("token="))
        .next()
        .map(urlencoding)
        .filter(|t| t == valid)
        .is_some()
}

fn build_router(token: &str) -> (Router, SocketIo) {
    let (layer, io) = SocketIo::new_layer();
    let valid_token_for_io = Arc::new(token.to_string());
    let valid_token_for_mw = Arc::clone(&valid_token_for_io);
    let valid_token_for_index = token.to_string();
    let history: Arc<Mutex<VecDeque<String>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(HISTORY_MAX)));

    // ── HTTP-level auth middleware ──
    let auth = middleware::from_fn(
        move |req: axum::http::Request<Body>, next: middleware::Next| {
            let token = Arc::clone(&valid_token_for_mw);
            async move {
                let path = req.uri().path();
                if path == "/socket.io" || path.starts_with("/socket.io/") {
                    let q = req.uri().query().unwrap_or("");
                    // Only check token on the initial handshake (no sid yet).
                    // Subsequent polling requests carry the authenticated sid.
                    let has_sid = q.split('&').any(|p| p.starts_with("sid="));
                    if !has_sid && !check_token(q, &token) {
                        return Response::builder()
                            .status(StatusCode::FORBIDDEN)
                            .body(Body::from("Forbidden"))
                            .unwrap();
                    }
                }
                next.run(req).await
            }
        },
    );

    // ── Socket.IO namespace handlers (defense-in-depth) ──
    io.ns("/", move |socket: SocketRef| {
        let query = socket.req_parts().uri.query().unwrap_or("");
        if !check_token(query, &valid_token_for_io) {
            tracing::warn!(
                "Socket.IO connection rejected — invalid or missing token (id: {})",
                socket.id
            );
            let _ = socket.disconnect();
            return;
        }

        let sid = socket.id;
        tracing::info!("[+] Client connected: {sid}");

        {
            let entries: Vec<String> = history.lock().unwrap().iter().cloned().collect();
            let _ = socket.emit("history", &entries);
        }

        let history_for_handler = history.clone();
        socket.on(
            "type_text",
            move |socket: SocketRef, Data(payload): Data<TypeTextPayload>| {
                let history = history_for_handler.clone();
                async move {
                    let text = payload.text.trim().to_string();
                    {
                        let mut guard = history.lock().unwrap();
                        add_to_history(&mut guard, &text, HISTORY_MAX);
                        let entries: Vec<String> = guard.iter().cloned().collect();
                        drop(guard);
                        let _ = socket.emit("history", &entries);
                    }
                    crate::keyboard::queue_type_text(payload.text);
                }
            },
        );
        socket.on("backspace", |_: SocketRef, Data(()): Data<()>| async move {
            crate::keyboard::queue_backspace();
        });
        socket.on(
            "press_key",
            |_: SocketRef, Data(payload): Data<PressKeyPayload>| async move {
                match payload.key.as_str() {
                    "enter" => crate::keyboard::queue_enter(),
                    other => tracing::warn!("Unknown key requested: {other}"),
                }
            },
        );

        socket.on("clear_input", |_: SocketRef, Data(()): Data<()>| async move {
            crate::keyboard::queue_select_all();
            crate::keyboard::queue_backspace();
        });

        socket.on_disconnect(move |_: SocketRef| {
            tracing::info!("[-] Client disconnected: {sid}");
        });
    });

    // ── Root route: token-gated, rejects missing/wrong token ──
    let app = Router::new()
        .route(
            "/",
            get(move |req: axum::http::Request<Body>| {
                let token = valid_token_for_index.clone();
                async move {
                    let q = req.uri().query().unwrap_or("");
                    if !check_token(q, &token) {
                        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
                    }
                    Html(crate::assets::HTML).into_response()
                }
            }),
        )
        .route(
            "/sio.min.js",
            get(|| async {
                Response::builder()
                    .header("Content-Type", "application/javascript")
                    .body(Body::from(SOCKET_IO_JS))
                    .unwrap()
            }),
        )
        .layer(layer)
        .layer(auth);

    (app, io)
}

/// Minimal percent-decode for token query params.
fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hi = chars.next().and_then(|h| h.to_digit(16));
            let lo = chars.next().and_then(|l| l.to_digit(16));
            if let (Some(h), Some(l)) = (hi, lo) {
                if let Some(decoded) = char::from_u32(h * 16 + l) {
                    out.push(decoded);
                    continue;
                }
            }
            out.push('%');
        } else if c == '+' {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

pub async fn run(port: u16, token: String, shutdown_rx: oneshot::Receiver<()>) {
    let (app, _io) = build_router(&token);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Server listening on http://0.0.0.0:{}", port);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to port. Is another TypeBridge instance running?");

    serve(listener, app, shutdown_rx).await;
}

/// Accept a pre-bound listener (used by E2E tests to avoid port TOCTOU
/// and bind to 127.0.0.1 instead of 0.0.0.0).
pub async fn serve(
    listener: tokio::net::TcpListener,
    app: Router,
    shutdown_rx: oneshot::Receiver<()>,
) {
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
            tracing::info!("Server shutting down gracefully...");
        })
        .await
        .expect("Server crashed unexpectedly");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    const TEST_TOKEN: &str = "abc123";

    fn test_router() -> Router {
        build_router(TEST_TOKEN).0
    }

    // ── add_to_history unit tests ────────────────────────────────

    #[test]
    fn test_add_to_history_inserts_entry() {
        let mut h = VecDeque::new();
        assert!(add_to_history(&mut h, "hello", 30));
        assert_eq!(h.len(), 1);
        assert_eq!(h.front().unwrap(), "hello");
    }

    #[test]
    fn test_add_to_history_dedup_consecutive() {
        let mut h = VecDeque::new();
        assert!(add_to_history(&mut h, "hello", 30));
        assert!(!add_to_history(&mut h, "hello", 30));
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn test_add_to_history_allows_different_after_same() {
        let mut h = VecDeque::new();
        assert!(add_to_history(&mut h, "hello", 30));
        assert!(!add_to_history(&mut h, "hello", 30));
        assert!(add_to_history(&mut h, "world", 30));
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn test_add_to_history_respects_max() {
        let mut h = VecDeque::new();
        for i in 0..30 {
            assert!(add_to_history(&mut h, &format!("text-{i}"), 30));
        }
        assert_eq!(h.len(), 30);
        assert_eq!(h.back().unwrap(), "text-0");
        assert!(add_to_history(&mut h, "new-text", 30));
        assert_eq!(h.len(), 30);
        assert_eq!(h.front().unwrap(), "new-text");
        assert_eq!(h.back().unwrap(), "text-1");
    }

    #[test]
    fn test_add_to_history_rejects_empty() {
        let mut h = VecDeque::new();
        assert!(!add_to_history(&mut h, "", 30));
        assert!(!add_to_history(&mut h, "   ", 30));
        assert!(h.is_empty());
    }

    // ── check_token unit tests ──────────────────────────────────

    #[test]
    fn test_check_token_valid() {
        assert!(check_token("token=abc123&other=x", TEST_TOKEN));
        assert!(check_token("token=abc123", TEST_TOKEN));
        assert!(check_token("a=1&token=abc123", TEST_TOKEN));
    }

    #[test]
    fn test_check_token_invalid() {
        assert!(!check_token("token=wrong", TEST_TOKEN));
        assert!(!check_token("token=", TEST_TOKEN));
        assert!(!check_token("", TEST_TOKEN));
        assert!(!check_token("other=abc123", TEST_TOKEN));
    }

    #[test]
    fn test_check_token_url_encoded() {
        assert!(check_token("token=abc%31%32%33", "abc123"));
    }

    // ── index page auth ─────────────────────────────────────────

    #[tokio::test]
    async fn test_index_rejected_without_token() {
        let app = test_router();
        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), 403);
    }

    #[tokio::test]
    async fn test_index_rejected_with_wrong_token() {
        let app = test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/?token=wrong")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 403);
    }

    #[tokio::test]
    async fn test_index_served_with_correct_token() {
        let app = test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/?token={TEST_TOKEN}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);

        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("<!DOCTYPE html>"));
        assert!(body_str.contains("TypeBridge"));
    }

    #[tokio::test]
    async fn test_index_page_does_not_embed_token() {
        let app = test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/?token={TEST_TOKEN}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);

        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        // Token must NOT appear as a literal in the HTML
        assert!(!body_str.contains(TEST_TOKEN));
    }

    // ── socket.io auth ──────────────────────────────────────────

    #[tokio::test]
    async fn test_socket_io_rejected_without_token() {
        let app = test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/socket.io/?EIO=4&transport=polling")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 403);
    }

    #[tokio::test]
    async fn test_socket_io_rejected_without_trailing_slash() {
        let app = test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/socket.io?EIO=4&transport=polling")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 403);
    }

    #[tokio::test]
    async fn test_socket_io_rejected_with_wrong_token() {
        let app = test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/socket.io/?EIO=4&transport=polling&token=wrong-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 403);
    }

    #[tokio::test]
    async fn test_socket_io_accepted_with_correct_token() {
        let app = test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/socket.io/?EIO=4&transport=polling&token={TEST_TOKEN}"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }

    #[tokio::test]
    async fn test_socket_io_polling_with_sid_bypasses_token_check() {
        let app = test_router();
        // Once a handshake succeeds, subsequent polling requests carry a sid
        // and should NOT require the token query param.
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/socket.io/?EIO=4&transport=polling&sid=existing-session-id")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // Should be accepted (200 or 400 from engine.io, NOT 403)
        assert_ne!(response.status(), 403);
    }

    // ── other HTTP tests ────────────────────────────────────────

    #[tokio::test]
    async fn test_index_content_type_html() {
        let app = test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/?token={TEST_TOKEN}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let content_type = response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(content_type.contains("text/html"));
    }

    #[tokio::test]
    async fn test_404_on_unknown_route() {
        let app = test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/nonexistent?token=abc123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 404);
    }

    #[tokio::test]
    async fn test_html_no_cdn_socket_io() {
        let app = test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/?token={TEST_TOKEN}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(!body_str.contains("https://cdn.socket.io"));
    }

    #[tokio::test]
    async fn test_html_no_google_fonts() {
        let app = test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/?token={TEST_TOKEN}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(!body_str.contains("fonts.googleapis.com"));
    }

    #[tokio::test]
    async fn test_socket_io_js_served() {
        let app = test_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/sio.min.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let content_type = response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(content_type.contains("javascript"));
    }

    #[test]
    fn test_urlencoding_basic() {
        assert_eq!(urlencoding("abc%20def"), "abc def");
        assert_eq!(urlencoding("hello+world"), "hello world");
        assert_eq!(urlencoding("noescape"), "noescape");
        assert_eq!(urlencoding("%21%40%23"), "!@#");
    }
}

// ── E2E Socket.IO integration tests ─────────────────────────────────
//
// These tests start a real server on a pre-bound 127.0.0.1 listener and
// exercise the Engine.IO v4 polling protocol via raw TCP, without any
// external client crates.

#[cfg(test)]
mod e2e_tests {
    use super::*;
    use crate::keyboard::{self, KeyCommand};
    use std::sync::mpsc;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    const E2E_TOKEN: &str = "e2e-test-token-42";
    /// How long we poll for a command before failing the test.
    const CMD_TIMEOUT: Duration = Duration::from_secs(2);

    /// Pre-bind a 127.0.0.1:0 listener, start the server on it, and return
    /// the port so the test can connect.  No TOCTOU race because the port is
    /// already held by the listener.
    fn spawn_server(token: String) -> (u16, tokio::sync::oneshot::Sender<()>) {
        let rt = tokio::runtime::Handle::current();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        // Bind synchronously so we know the port before spawning
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).ok();
        let listener = tokio::net::TcpListener::from_std(listener).expect("from_std");

        rt.spawn(async move {
            let (app, _) = build_router(&token);
            crate::server::serve(listener, app, shutdown_rx).await;
        });

        (port, shutdown_tx)
    }

    /// Poll until a TCP connect to `127.0.0.1:port` succeeds, or timeout.
    async fn wait_for_server(port: u16, timeout: Duration) {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            match TcpStream::connect(format!("127.0.0.1:{port}")).await {
                Ok(_) => return,
                Err(_) if tokio::time::Instant::now() < deadline => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(e) => panic!("server did not become reachable within {timeout:?}: {e}"),
            }
        }
    }

    /// Read from a TCP stream until EOF (server closes after `Connection: close`).
    /// Wrapped in a timeout so a misbehaving server doesn't hang the test.
    async fn read_http_response(stream: &mut TcpStream) -> String {
        tokio::time::timeout(Duration::from_secs(2), async {
            let mut buf = Vec::with_capacity(8192);
            let mut chunk = [0u8; 2048];
            loop {
                match stream.read(&mut chunk).await {
                    Ok(0) => break, // EOF
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    Err(e) => panic!("read error: {e}"),
                }
            }
            String::from_utf8_lossy(&buf).into_owned()
        })
        .await
        .expect("HTTP response read timed out after 2s")
    }

    /// Send an HTTP GET with `Connection: close`, read full response.
    async fn http_get(port: u16, path: &str) -> (u16, String) {
        let mut s = TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .expect("tcp connect");
        let req =
            format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
        s.write_all(req.as_bytes()).await.expect("write");
        let raw = read_http_response(&mut s).await;
        parse_http(&raw)
    }

    /// Send an HTTP POST, read full response.
    async fn http_post(port: u16, path: &str, body: &str) -> (u16, String) {
        let mut s = TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .expect("tcp connect");
        let req = format!(
            "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        s.write_all(req.as_bytes()).await.expect("write");
        let raw = read_http_response(&mut s).await;
        parse_http(&raw)
    }

    /// Minimal HTTP response parser: (status, body_after_headers).
    fn parse_http(raw: &str) -> (u16, String) {
        let status = raw
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let body = raw.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
        (status, body)
    }

    /// Poll `test_rx` with timeout; panics if no command arrives.
    fn recv_cmd(test_rx: &mpsc::Receiver<KeyCommand>) -> KeyCommand {
        match test_rx.recv_timeout(CMD_TIMEOUT) {
            Ok(cmd) => cmd,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("no command received within {CMD_TIMEOUT:?}")
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => panic!("command channel disconnected"),
        }
    }

    /// Engine.IO v4 handshake; returns the `sid` on success.
    async fn eio_handshake(port: u16, token: &str) -> Result<String, String> {
        let (status, body) = http_get(
            port,
            &format!("/socket.io/?EIO=4&transport=polling&token={token}"),
        )
        .await;
        if status != 200 {
            return Err(format!("handshake rejected: status {status}"));
        }
        let payload = body.trim().strip_prefix('0').ok_or("missing 0 prefix")?;
        let open: serde_json::Value =
            serde_json::from_str(payload).map_err(|e| format!("json parse: {e}"))?;
        open["sid"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "no sid field".to_string())
    }

    /// Socket.IO CONNECT (Engine.IO message 4, Socket.IO type 0).
    async fn sio_connect(port: u16, sid: &str) -> Result<String, String> {
        let _ = http_post(
            port,
            &format!("/socket.io/?EIO=4&transport=polling&sid={sid}"),
            "40",
        )
        .await;

        let (status, body) = http_get(
            port,
            &format!("/socket.io/?EIO=4&transport=polling&sid={sid}"),
        )
        .await;
        if status != 200 {
            return Err(format!("poll rejected: status {status}"));
        }
        let sio_packet = body
            .trim()
            .split('\x1e')
            .find(|p| p.starts_with("40"))
            .ok_or_else(|| format!("no 40 packet in poll response: {body:?}"))?;
        Ok(sio_packet.to_string())
    }

    /// Emit a Socket.IO event (Engine.IO message 4, Socket.IO type 2).
    async fn sio_emit(port: u16, sid: &str, event: &str, payload: &str) -> Result<(), String> {
        let body = format!("42[\"{event}\",{payload}]");
        let (status, _) = http_post(
            port,
            &format!("/socket.io/?EIO=4&transport=polling&sid={sid}"),
            &body,
        )
        .await;
        if status != 200 {
            return Err(format!("emit rejected: status {status}"));
        }
        Ok(())
    }

    // ── the E2E test ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_e2e_socket_io_auth_and_events() {
        let (port, shutdown_tx) = spawn_server(E2E_TOKEN.to_string());
        wait_for_server(port, Duration::from_secs(3)).await;

        // ── 1. no token → handshake rejected ──────────────────────
        let (status, _) = http_get(port, "/socket.io/?EIO=4&transport=polling").await;
        assert_eq!(status, 403, "no token → 403");

        // ── 2. wrong token → handshake rejected ──────────────────
        let (status, _) = http_get(port, "/socket.io/?EIO=4&transport=polling&token=bad").await;
        assert_eq!(status, 403, "wrong token → 403");

        // ── 3. correct token → handshake succeeds ─────────────────
        let sid = eio_handshake(port, E2E_TOKEN)
            .await
            .expect("valid token handshake");
        assert!(!sid.is_empty());

        // ── 4. Socket.IO CONNECT / namespace join ────────────────
        let ack = sio_connect(port, &sid).await.expect("sio connect");
        assert!(ack.starts_with("40"), "connect ack starts with 40");

        // ── 5. type_text event → verify command queued ────────────
        // TestGuard captures the current global state and restores it
        // on drop, so this test cannot leak ENABLED or COMMAND_TX to
        // other tests (even after a panic).
        let mut guard = keyboard::TestGuard::new();
        crate::keyboard::set_enabled(true);

        let (test_tx, test_rx) = mpsc::sync_channel::<KeyCommand>(16);
        guard.replace_command_tx(test_tx);

        sio_emit(port, &sid, "type_text", "{\"text\":\"hello e2e\"}")
            .await
            .expect("type_text emit");

        match recv_cmd(&test_rx) {
            KeyCommand::TypeText(text) => assert_eq!(text, "hello e2e"),
            other => panic!("expected TypeText, got {other:?}"),
        }

        // ── 6. backspace event ────────────────────────────────────
        sio_emit(port, &sid, "backspace", "null")
            .await
            .expect("backspace emit");

        match recv_cmd(&test_rx) {
            KeyCommand::Backspace => {}
            other => panic!("expected Backspace, got {other:?}"),
        }

        // ── 7. press_key enter event ──────────────────────────────
        sio_emit(port, &sid, "press_key", "{\"key\":\"enter\"}")
            .await
            .expect("press_key emit");

        match recv_cmd(&test_rx) {
            KeyCommand::Enter => {}
            other => panic!("expected Enter, got {other:?}"),
        }

        // ── clean shutdown ───────────────────────────────────────
        let _ = shutdown_tx.send(());
    }

    #[tokio::test]
    async fn test_e2e_history_flow() {
        // Isolate keyboard state so this test doesn't interfere with
        // other E2E tests that share the global COMMAND_TX / ENABLED.
        let mut guard = keyboard::TestGuard::new();
        crate::keyboard::set_enabled(true);
        let (test_tx, test_rx) = mpsc::sync_channel::<KeyCommand>(16);
        guard.replace_command_tx(test_tx);

        let (port, shutdown_tx) = spawn_server(E2E_TOKEN.to_string());
        wait_for_server(port, Duration::from_secs(3)).await;

        let sid = eio_handshake(port, E2E_TOKEN).await.expect("handshake");

        // Manually connect — check the full poll body for the history event
        let _ = http_post(
            port,
            &format!("/socket.io/?EIO=4&transport=polling&sid={sid}"),
            "40",
        )
        .await;

        let (status, body) = http_get(
            port,
            &format!("/socket.io/?EIO=4&transport=polling&sid={sid}"),
        )
        .await;
        assert_eq!(status, 200);
        assert!(
            body.contains("42[\"history\""),
            "initial history event missing: {body}"
        );

        // Send type_text and poll for updated history
        sio_emit(port, &sid, "type_text", "{\"text\":\"e2e-history-text\"}")
            .await
            .expect("type_text emit");

        let (status, body) = http_get(
            port,
            &format!("/socket.io/?EIO=4&transport=polling&sid={sid}"),
        )
        .await;
        assert_eq!(status, 200);
        assert!(
            body.contains("e2e-history-text"),
            "history should contain sent text: {body}"
        );

        // Also verify the keyboard command was queued
        match recv_cmd(&test_rx) {
            KeyCommand::TypeText(text) => assert_eq!(text, "e2e-history-text"),
            other => panic!("expected TypeText, got {other:?}"),
        }

        let _ = shutdown_tx.send(());
    }
}
