use crate::assets::HTML;
use axum::{response::Html, routing::get, Router};
use serde::Deserialize;
use socketioxide::extract::{Data, SocketRef};
use socketioxide::SocketIo;
use tokio::sync::oneshot;

#[derive(Deserialize)]
struct TypeTextPayload {
    text: String,
}

#[derive(Deserialize)]
struct PressKeyPayload {
    key: String,
}

fn build_router() -> (Router, SocketIo) {
    let (layer, io) = SocketIo::new_layer();

    io.ns("/", |socket: SocketRef| {
        let sid = socket.id;
        tracing::info!("[+] Client connected: {sid}");

        socket.on(
            "type_text",
            |_: SocketRef, Data(payload): Data<TypeTextPayload>| async move {
                crate::keyboard::queue_type_text(payload.text);
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

        socket.on_disconnect(move |_: SocketRef| {
            tracing::info!("[-] Client disconnected: {sid}");
        });
    });

    let app = Router::new()
        .route("/", get(|| async { Html(HTML) }))
        .layer(layer);

    (app, io)
}

pub async fn run(port: u16, shutdown_rx: oneshot::Receiver<()>) {
    let (app, _io) = build_router();

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Server listening on http://0.0.0.0:{}", port);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to port. Is another TypeBridge instance running?");

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

    #[tokio::test]
    async fn test_index_returns_html() {
        let (app, _io) = build_router();
        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("<!DOCTYPE html>"));
        assert!(body_str.contains("TypeBridge"));
        assert!(body_str.contains("textarea"));
    }

    #[tokio::test]
    async fn test_index_content_type_html() {
        let (app, _io) = build_router();
        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
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
        let (app, _io) = build_router();
        let response = app
            .oneshot(Request::builder().uri("/nonexistent").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), 404);
    }
}
