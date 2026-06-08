use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode};
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

type Resp = axum::response::Response<Body>;

#[derive(Clone)]
pub struct AppState {
    pub token: String,
    pub history: Arc<Mutex<VecDeque<String>>>,
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

fn build_router(state: &Arc<AppState>) -> Router {
    // ── Auth middleware ────────────────────────────────────────

    async fn auth_middleware(
        State(state): State<Arc<AppState>>,
        req: axum::http::Request<Body>,
        next: middleware::Next,
    ) -> Resp {
        if req.uri().query().is_some_and(|q| q.contains("token=")) {
            return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "unauthorized"}))).into_response();
        }
        let auth = req.headers().get("Authorization").and_then(|v| v.to_str().ok()).unwrap_or("");
        if !auth.starts_with("Bearer ") || auth[7..] != state.token {
            return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "unauthorized"}))).into_response();
        }
        next.run(req).await
    }

    // ── Security headers middleware ────────────────────────────

    async fn security_headers(
        req: axum::http::Request<Body>,
        next: middleware::Next,
    ) -> Resp {
        let mut response = next.run(req).await;
        let headers = response.headers_mut();
        headers.insert("Cache-Control", HeaderValue::from_static("no-store"));
        headers.insert("Referrer-Policy", HeaderValue::from_static("no-referrer"));
        headers.insert("X-Content-Type-Options", HeaderValue::from_static("nosniff"));
        headers.insert("Cross-Origin-Resource-Policy", HeaderValue::from_static("same-origin"));
        response
    }

    // ── Handlers ───────────────────────────────────────────────

    async fn index_handler() -> Resp {
        (StatusCode::OK, axum::response::Html(crate::assets::HTML)).into_response()
    }

    async fn status_handler(State(_state): State<Arc<AppState>>) -> Resp {
        let enabled = crate::keyboard::is_enabled();
        Json(serde_json::json!({"enabled": enabled, "version": "0.3.0"})).into_response()
    }

    async fn history_handler(State(state): State<Arc<AppState>>) -> Resp {
        let entries: Vec<String> = state.history.lock().unwrap().iter().cloned().collect();
        Json(entries).into_response()
    }

    #[derive(Deserialize)]
    struct CommandPayload {
        #[serde(rename = "type")]
        cmd_type: String,
        text: Option<String>,
    }

    fn skc(result: crate::keyboard::CommandResult) -> Resp {
        match result {
            crate::keyboard::CommandResult::Queued => (StatusCode::ACCEPTED, Json(serde_json::json!({"status": "queued"}))).into_response(),
            crate::keyboard::CommandResult::Paused => (StatusCode::CONFLICT, Json(serde_json::json!({"error": "paused"}))).into_response(),
            crate::keyboard::CommandResult::Full => (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({"error": "queue full"}))).into_response(),
            crate::keyboard::CommandResult::TooLong => (StatusCode::PAYLOAD_TOO_LARGE, Json(serde_json::json!({"error": "text too long"}))).into_response(),
        }
    }

    fn bad_req(msg: &'static str) -> Resp {
        (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": msg}))).into_response()
    }

    async fn commands_handler(
        State(state): State<Arc<AppState>>,
        Json(payload): Json<CommandPayload>,
    ) -> Resp {
        match payload.cmd_type.as_str() {
            "type_text" => {
                let text = match payload.text {
                    Some(t) => t,
                    None => return bad_req("empty text"),
                };
                if text.is_empty() {
                    return bad_req("empty text");
                }
                if text.len() > 10_000 {
                    return (StatusCode::PAYLOAD_TOO_LARGE, Json(serde_json::json!({"error": "text too long"}))).into_response();
                }
                let result = crate::keyboard::queue_type_text(text.clone());
                if result == crate::keyboard::CommandResult::Queued {
                    let mut guard = state.history.lock().unwrap();
                    add_to_history(&mut guard, &text, HISTORY_MAX);
                }
                skc(result)
            }
            "enter" => skc(crate::keyboard::queue_enter()),
            "backspace" => skc(crate::keyboard::queue_backspace()),
            "clear_pc_field" => {
                match crate::keyboard::queue_select_all() {
                    crate::keyboard::CommandResult::Queued => {}
                    other => return skc(other),
                }
                skc(crate::keyboard::queue_backspace())
            }
            _ => bad_req("unknown command type"),
        }
    }

    // ── Route assembly ─────────────────────────────────────────

    let api_routes = Router::new()
        .route("/status", get(status_handler))
        .route("/history", get(history_handler))
        .route("/commands", post(commands_handler))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    Router::new()
        .route("/", get(index_handler))
        .nest("/api", api_routes)
        .layer(middleware::from_fn(security_headers))
        .with_state(state.clone())
}

pub async fn run(port: u16, state: Arc<AppState>, shutdown_rx: oneshot::Receiver<()>) {
    let app = build_router(&state);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Server listening on http://0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to port. Is another TypeBridge instance running?");
    serve(listener, app, shutdown_rx).await;
}

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
    use axum::http::Request;
    use std::sync::mpsc;
    use tower::ServiceExt;

    const TEST_TOKEN: &str = "abc123";

    fn init_test_globals() {
        use std::sync::OnceLock;
        static BRIDGE: OnceLock<mpsc::Sender<crate::keyboard::KeyCommand>> = OnceLock::new();
        BRIDGE.get_or_init(|| {
            let (tx, rx) = mpsc::channel::<crate::keyboard::KeyCommand>();
            std::thread::spawn(move || { while rx.recv().is_ok() {} });
            tx
        });
        let bridge_tx = BRIDGE.get().unwrap();
        let (sync_tx, sync_rx) = mpsc::sync_channel::<crate::keyboard::KeyCommand>(100_000);
        let bt = bridge_tx.clone();
        std::thread::spawn(move || {
            while let Ok(cmd) = sync_rx.recv() {
                if bt.send(cmd).is_err() { break; }
            }
        });
        let mut guard = crate::keyboard::COMMAND_TX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *guard = Some(sync_tx);
    }

    fn test_router() -> Router {
        init_test_globals();
        let state = Arc::new(AppState {
            token: TEST_TOKEN.to_string(),
            history: Arc::new(Mutex::new(VecDeque::new())),
        });
        build_router(&state)
    }

    // ── add_to_history ─────────────────────────────────────────

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

    #[tokio::test]
    async fn test_index_returns_html_no_auth() {
        let response = test_router().oneshot(Request::builder().uri("/").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), 200);
        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        assert!(String::from_utf8(body.to_vec()).unwrap().contains("TypeBridge"));
    }

    #[tokio::test]
    async fn test_api_status_unauthorized_without_header() {
        assert_eq!(test_router().oneshot(Request::builder().uri("/api/status").body(Body::empty()).unwrap()).await.unwrap().status(), 401);
    }

    #[tokio::test]
    async fn test_api_status_unauthorized_with_wrong_token() {
        assert_eq!(test_router().oneshot(Request::builder().uri("/api/status").header("Authorization", "Bearer wrong").body(Body::empty()).unwrap()).await.unwrap().status(), 401);
    }

    #[tokio::test]
    async fn test_api_status_unauthorized_with_query_token() {
        assert_eq!(test_router().oneshot(Request::builder().uri("/api/status?token=abc123").header("Authorization", "Bearer abc123").body(Body::empty()).unwrap()).await.unwrap().status(), 401);
    }

    #[tokio::test]
    async fn test_api_status_returns_enabled() {
        let response = test_router().oneshot(Request::builder().uri("/api/status").header("Authorization", "Bearer abc123").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), 200);
        let json: serde_json::Value = serde_json::from_str(&String::from_utf8(axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap().to_vec()).unwrap()).unwrap();
        assert!(json.get("enabled").is_some());
        assert_eq!(json["version"], "0.3.0");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_api_commands_type_text_returns_202() {
        assert_eq!(test_router().oneshot(Request::builder().method("POST").uri("/api/commands").header("Authorization", "Bearer abc123").header("Content-Type", "application/json").body(Body::from(r#"{"type":"type_text","text":"hello"}"#)).unwrap()).await.unwrap().status(), 202);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_api_commands_returns_400_for_empty_text() {
        assert_eq!(test_router().oneshot(Request::builder().method("POST").uri("/api/commands").header("Authorization", "Bearer abc123").header("Content-Type", "application/json").body(Body::from(r#"{"type":"type_text","text":""}"#)).unwrap()).await.unwrap().status(), 400);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_api_commands_returns_413_for_long_text() {
        assert_eq!(test_router().oneshot(Request::builder().method("POST").uri("/api/commands").header("Authorization", "Bearer abc123").header("Content-Type", "application/json").body(Body::from(serde_json::json!({"type":"type_text","text":"x".repeat(10_001)}).to_string())).unwrap()).await.unwrap().status(), 413);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_api_commands_returns_409_when_paused() {
        let s = test_router().oneshot(Request::builder().method("POST").uri("/api/commands").header("Authorization", "Bearer abc123").header("Content-Type", "application/json").body(Body::from(r#"{"type":"type_text","text":"test"}"#)).unwrap()).await.unwrap().status();
        if crate::keyboard::is_enabled() {
            assert!(s == 202 || s == 409);
        } else {
            assert_eq!(s, 409);
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_api_commands_backspace_returns_202() {
        assert_eq!(test_router().oneshot(Request::builder().method("POST").uri("/api/commands").header("Authorization", "Bearer abc123").header("Content-Type", "application/json").body(Body::from(r#"{"type":"backspace"}"#)).unwrap()).await.unwrap().status(), 202);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_api_commands_enter_returns_202() {
        assert_eq!(test_router().oneshot(Request::builder().method("POST").uri("/api/commands").header("Authorization", "Bearer abc123").header("Content-Type", "application/json").body(Body::from(r#"{"type":"enter"}"#)).unwrap()).await.unwrap().status(), 202);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_api_commands_clear_pc_field_returns_202() {
        assert_eq!(test_router().oneshot(Request::builder().method("POST").uri("/api/commands").header("Authorization", "Bearer abc123").header("Content-Type", "application/json").body(Body::from(r#"{"type":"clear_pc_field"}"#)).unwrap()).await.unwrap().status(), 202);
    }

    #[tokio::test]
    async fn test_api_history_returns_401_without_auth() {
        assert_eq!(test_router().oneshot(Request::builder().uri("/api/history").body(Body::empty()).unwrap()).await.unwrap().status(), 401);
    }

    #[tokio::test]
    async fn test_security_headers_present() {
        let res = test_router().oneshot(Request::builder().uri("/").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(res.headers().get("Cache-Control").unwrap(), "no-store");
        assert_eq!(res.headers().get("Referrer-Policy").unwrap(), "no-referrer");
        assert_eq!(res.headers().get("X-Content-Type-Options").unwrap(), "nosniff");
        assert_eq!(res.headers().get("Cross-Origin-Resource-Policy").unwrap(), "same-origin");
    }

    #[tokio::test]
    async fn test_no_cors_headers() {
        let res = test_router().oneshot(Request::builder().uri("/").body(Body::empty()).unwrap()).await.unwrap();
        for name in res.headers().keys() {
            assert!(!name.as_str().to_lowercase().starts_with("access-control-"));
        }
    }

    #[tokio::test]
    async fn test_index_content_type_html() {
        let ct = test_router().oneshot(Request::builder().uri("/").body(Body::empty()).unwrap()).await.unwrap().headers().get("content-type").unwrap().to_str().unwrap().to_string();
        assert!(ct.contains("text/html"));
    }

    #[tokio::test]
    async fn test_html_no_cdn_socket_io() {
        let body = axum::body::to_bytes(test_router().oneshot(Request::builder().uri("/").body(Body::empty()).unwrap()).await.unwrap().into_body(), 1024 * 1024).await.unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(!body_str.contains("https://cdn.socket.io"));
    }

    #[tokio::test]
    async fn test_html_no_google_fonts() {
        let body = axum::body::to_bytes(test_router().oneshot(Request::builder().uri("/").body(Body::empty()).unwrap()).await.unwrap().into_body(), 1024 * 1024).await.unwrap();
        assert!(!String::from_utf8(body.to_vec()).unwrap().contains("fonts.googleapis.com"));
    }

    #[tokio::test]
    async fn test_404_on_unknown_route() {
        assert_eq!(test_router().oneshot(Request::builder().uri("/nonexistent").body(Body::empty()).unwrap()).await.unwrap().status(), 404);
    }
}

#[cfg(test)]
mod e2e_tests {
    use super::*;
    use crate::keyboard::{self, KeyCommand};
    use std::sync::mpsc;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    const E2E_TOKEN: &str = "e2e-test-token-42";
    const CMD_TIMEOUT: Duration = Duration::from_secs(2);

    fn spawn_server(token: String) -> (u16, tokio::sync::oneshot::Sender<()>) {
        let rt = tokio::runtime::Handle::current();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).ok();
        let listener = tokio::net::TcpListener::from_std(listener).expect("from_std");
        let state = Arc::new(AppState {
            token: token.clone(),
            history: Arc::new(Mutex::new(VecDeque::new())),
        });
        let app = build_router(&state);
        rt.spawn(async move { crate::server::serve(listener, app, shutdown_rx).await; });
        (port, shutdown_tx)
    }

    async fn wait_for_server(port: u16, timeout: Duration) {
        let deadline = tokio::time::Instant::now() + timeout;
        loop { match TcpStream::connect(format!("127.0.0.1:{port}")).await { Ok(_) => return, Err(_) if tokio::time::Instant::now() < deadline => tokio::time::sleep(Duration::from_millis(10)).await, Err(e) => panic!("server not reachable: {e}") } }
    }

    async fn read_response(stream: &mut TcpStream) -> String {
        tokio::time::timeout(Duration::from_secs(2), async {
            let mut b = Vec::new();
            let mut c = [0u8; 2048];
            loop { match stream.read(&mut c).await { Ok(0) => break, Ok(n) => b.extend_from_slice(&c[..n]), Err(e) => panic!("read: {e}") } }
            String::from_utf8_lossy(&b).into_owned()
        }).await.expect("read timeout")
    }

    async fn http_get(port: u16, path: &str, auth: Option<&str>) -> (u16, String) {
        let mut s = TcpStream::connect(format!("127.0.0.1:{port}")).await.expect("tcp");
        let ah = auth.map(|t| format!("Authorization: Bearer {t}\r\n")).unwrap_or_default();
        s.write_all(format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{ah}Connection: close\r\n\r\n").as_bytes()).await.expect("write");
        let raw = read_response(&mut s).await;
        let status = raw.lines().next().and_then(|l| l.split_whitespace().nth(1)).and_then(|s| s.parse().ok()).unwrap_or(0);
        (status, raw.split("\r\n\r\n").nth(1).unwrap_or("").to_string())
    }

    async fn http_post(port: u16, path: &str, auth: Option<&str>, ct: &str, body: &str) -> (u16, String) {
        let mut s = TcpStream::connect(format!("127.0.0.1:{port}")).await.expect("tcp");
        let ah = auth.map(|t| format!("Authorization: Bearer {t}\r\n")).unwrap_or_default();
        s.write_all(format!("POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{ah}Content-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.expect("write");
        let raw = read_response(&mut s).await;
        let status = raw.lines().next().and_then(|l| l.split_whitespace().nth(1)).and_then(|s| s.parse().ok()).unwrap_or(0);
        (status, raw.split("\r\n\r\n").nth(1).unwrap_or("").to_string())
    }

    fn recv_cmd(rx: &mpsc::Receiver<KeyCommand>) -> KeyCommand {
        match rx.recv_timeout(CMD_TIMEOUT) {
            Ok(c) => c,
            Err(mpsc::RecvTimeoutError::Timeout) => panic!("no command"),
            Err(mpsc::RecvTimeoutError::Disconnected) => panic!("disconnected"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_e2e_auth_rejection() {
        let _guard = crate::keyboard::TestGuard::new();
        let (port, tx) = spawn_server(E2E_TOKEN.to_string());
        wait_for_server(port, Duration::from_secs(3)).await;
        assert_eq!(http_get(port, "/api/status", None).await.0, 401);
        assert_eq!(http_get(port, "/api/status", Some("wrong")).await.0, 401);
        assert_eq!(http_get(port, "/api/status", Some(E2E_TOKEN)).await.0, 200);
        let _ = tx.send(());
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_e2e_command_flow() {
        let mut g = keyboard::TestGuard::new();
        crate::keyboard::set_enabled(true);
        let (ttx, trx) = mpsc::sync_channel::<KeyCommand>(16);
        g.replace_command_tx(ttx);
        let (port, tx) = spawn_server(E2E_TOKEN.to_string());
        wait_for_server(port, Duration::from_secs(3)).await;

        assert_eq!(http_post(port, "/api/commands", Some(E2E_TOKEN), "application/json", r#"{"type":"type_text","text":"hello e2e"}"#).await.0, 202);
        match recv_cmd(&trx) { KeyCommand::TypeText(t) => assert_eq!(t, "hello e2e"), o => panic!("expected TypeText, got {o:?}") }

        assert_eq!(http_post(port, "/api/commands", Some(E2E_TOKEN), "application/json", r#"{"type":"backspace"}"#).await.0, 202);
        match recv_cmd(&trx) { KeyCommand::Backspace => {}, o => panic!("expected Backspace, got {o:?}") }

        assert_eq!(http_post(port, "/api/commands", Some(E2E_TOKEN), "application/json", r#"{"type":"enter"}"#).await.0, 202);
        match recv_cmd(&trx) { KeyCommand::Enter => {}, o => panic!("expected Enter, got {o:?}") }
        let _ = tx.send(());
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_e2e_history_flow() {
        let mut g = keyboard::TestGuard::new();
        crate::keyboard::set_enabled(true);
        let (ttx, trx) = mpsc::sync_channel::<KeyCommand>(16);
        g.replace_command_tx(ttx);
        let (port, tx) = spawn_server(E2E_TOKEN.to_string());
        wait_for_server(port, Duration::from_secs(3)).await;

        let (s, b) = http_get(port, "/api/history", Some(E2E_TOKEN)).await;
        assert_eq!(s, 200);
        assert_eq!(b.trim(), "[]");

        assert_eq!(http_post(port, "/api/commands", Some(E2E_TOKEN), "application/json", r#"{"type":"type_text","text":"e2e-history-text"}"#).await.0, 202);
        match recv_cmd(&trx) { KeyCommand::TypeText(t) => assert_eq!(t, "e2e-history-text"), o => panic!("expected TypeText, got {o:?}") }

        let (s, b) = http_get(port, "/api/history", Some(E2E_TOKEN)).await;
        assert_eq!(s, 200);
        assert!(b.contains("e2e-history-text"));
        let _ = tx.send(());
    }
}
