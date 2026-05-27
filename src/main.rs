mod assets;
mod ip;
mod keyboard;
mod server;
mod tray;

use clap::Parser;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tray_icon::menu::MenuEvent;
use winit::event::Event;
use winit::event_loop::EventLoopBuilder;
use winit::event_loop::EventLoopProxy;

#[derive(Debug)]
enum TrayEvent {
    Menu(muda::MenuId),
}

struct SyncProxy(EventLoopProxy<TrayEvent>);
unsafe impl Sync for SyncProxy {}

#[derive(Parser)]
#[command(name = "type-bridge-rs", version, about = "Wi-Fi remote keyboard — type on your PC from your phone browser")]
struct Cli {
    #[arg(short, long, default_value = "12345")]
    port: u16,
}

fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let ip = ip::get_local_ip();
    let port = cli.port;
    let url = format!("http://{}:{}", ip, port);

    println!("\n⌨️  TypeBridge running!");
    println!("📱 Open on your phone: {url}");
    print_qr(&url);

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let shutdown_tx = std::cell::Cell::new(Some(shutdown_tx));

    let (kb_tx, kb_rx) = mpsc::channel::<keyboard::KeyCommand>();
    keyboard::init_command_queue(kb_tx);

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async { server::run(port, shutdown_rx).await });
    });

    let event_loop = EventLoopBuilder::<TrayEvent>::with_user_event()
        .build()
        .expect("Failed to create event loop");
    let proxy = Arc::new(SyncProxy(event_loop.create_proxy()));

    let tray_state = Rc::new(RefCell::new(tray::build_tray(&ip, port)));
    let toggle_id = tray_state.borrow().toggle_id.clone();
    let copy_url_id = tray_state.borrow().copy_url_id.clone();
    let quit_id = tray_state.borrow().quit_id.clone();

    let proxy2 = proxy.clone();
    MenuEvent::set_event_handler(Some(move |event: tray_icon::menu::MenuEvent| {
        let _ = proxy2.0.send_event(TrayEvent::Menu(event.id));
    }));

    let url_clone = url.clone();

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(100),
        ));

        while let Ok(cmd) = kb_rx.try_recv() {
            keyboard::execute(cmd);
        }

        if let Event::UserEvent(TrayEvent::Menu(id)) = event {
            if id == toggle_id {
                let enabled = keyboard::is_enabled();
                keyboard::set_enabled(!enabled);
                let new_state = keyboard::is_enabled();
                let status = if new_state { "ON" } else { "PAUSED" };
                let state = tray_state.borrow_mut();
                state
                    .tray
                    .set_icon(Some(crate::tray::make_icon(new_state)))
                    .unwrap_or_else(|e| tracing::error!("Failed to update tray icon: {e}"));
                state
                    .tray
                    .set_tooltip(Some(format!(
                        "TypeBridge — {}\n{}",
                        status, url_clone
                    )))
                    .unwrap_or_else(|e| tracing::error!("Failed to update tray tooltip: {e}"));
            } else if id == copy_url_id {
                copy_to_clipboard(&url_clone);
            } else if id == quit_id {
                tracing::info!("Shutting down...");
                if let Some(tx) = shutdown_tx.take() {
                    let _ = tx.send(());
                }
                elwt.exit();
            }
        }
    })
    .expect("Event loop error");

    std::thread::sleep(Duration::from_millis(300));
}

fn print_qr(url: &str) {
    use qrcode::QrCode;
    use qrcode::render::unicode;

    let code = QrCode::new(url).expect("Failed to generate QR code");
    let image = code
        .render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Dark)
        .light_color(unicode::Dense1x2::Light)
        .build();
    println!("{image}");
}

fn copy_to_clipboard(url: &str) {
    match arboard::Clipboard::new() {
        Ok(mut c) => {
            if let Err(e) = c.set_text(url) {
                tracing::error!("Failed to copy to clipboard: {e}");
            }
        }
        Err(e) => tracing::error!("Failed to open clipboard: {e}"),
    }
}
