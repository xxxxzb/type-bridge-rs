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

pub async fn run(port: u16, shutdown_rx: oneshot::Receiver<()>) {
    let (layer, io) = SocketIo::new_layer();

    io.ns("/", |socket: SocketRef| {
        let sid = socket.id;
        tracing::info!("[+] Client connected: {sid}");

        socket.on(
            "type_text",
            |_: SocketRef, Data(payload): Data<TypeTextPayload>| async move {
                crate::keyboard::type_text(&payload.text);
            },
        );
        socket.on("backspace", |_: SocketRef, Data(()): Data<()>| async move {
            crate::keyboard::press_backspace();
        });
        socket.on(
            "press_key",
            |_: SocketRef, Data(payload): Data<PressKeyPayload>| async move {
                match payload.key.as_str() {
                    "enter" => crate::keyboard::press_enter(),
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
