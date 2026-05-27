mod assets;
mod ip;
mod keyboard;
mod server;
mod tray;

use clap::Parser;
use std::cell::RefCell;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tray_icon::menu::MenuEvent;
use winit::dpi::PhysicalPosition;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoopBuilder;
use winit::event_loop::EventLoopProxy;
use winit::window::{WindowBuilder, WindowLevel};

#[derive(Debug)]
enum TrayEvent {
    Menu(muda::MenuId),
    TrayClick,
}

struct SyncProxy(EventLoopProxy<TrayEvent>);
unsafe impl Sync for SyncProxy {}

struct QrWindow {
    window: winit::window::Window,
}

#[derive(Parser)]
#[command(name = "type-bridge-rs", version, about = "Wi-Fi remote keyboard")]
struct Cli {
    #[arg(short, long, default_value = "12345")]
    port: u16,
}

// ── QR constants ────────────────────────────────────────────────────

const QR_SCALE: u32 = 6;
const QR_PAD: u32 = 28;

fn physical_scale(window: &winit::window::Window) -> u32 {
    window.scale_factor() as u32
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

    let mut event_loop_builder = EventLoopBuilder::<TrayEvent>::with_user_event();

    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        event_loop_builder.with_activation_policy(ActivationPolicy::Accessory);
    }

    let event_loop = event_loop_builder
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

    let proxy3 = proxy.clone();
    tray_icon::TrayIconEvent::set_event_handler(Some(move |event: tray_icon::TrayIconEvent| {
        // Only open QR on actual click, not hover/enter/move/leave
        if matches!(event, tray_icon::TrayIconEvent::Click { .. }) {
            let _ = proxy3.0.send_event(TrayEvent::TrayClick);
        }
    }));

    let qr_state: Rc<RefCell<Option<QrWindow>>> = Rc::new(RefCell::new(None));
    let url_clone = url.clone();
    let mut last_click = Instant::now();

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(200),
        ));

        while let Ok(cmd) = kb_rx.try_recv() {
            keyboard::execute(cmd);
        }

        // Auto-close QR window 3s after menu likely dismissed
        // (timer resets when mouse hovers over QR — user is scanning)
        if qr_state.borrow().is_some() && last_click.elapsed() > Duration::from_secs(3) {
            *qr_state.borrow_mut() = None;
            last_click = Instant::now();
        }

        match event {
            Event::UserEvent(TrayEvent::TrayClick) => {
                last_click = Instant::now();
                open_qr_window(&url_clone, elwt, &qr_state);
            }
            Event::UserEvent(TrayEvent::Menu(id)) => {
                // Any menu click means user is done — close QR together with menu
                *qr_state.borrow_mut() = None;
                last_click = Instant::now();

                if id == toggle_id {
                    let enabled = keyboard::is_enabled();
                    keyboard::set_enabled(!enabled);
                    let new_state = keyboard::is_enabled();
                    let status = if new_state { "ON" } else { "PAUSED" };
                    let state = tray_state.borrow_mut();
                    state.tray.set_icon(Some(crate::tray::make_icon(new_state)))
                        .unwrap_or_else(|e| tracing::error!("Tray icon: {e}"));
                    state.tray.set_tooltip(Some(format!("TypeBridge — {}\n{}", status, url_clone)))
                        .unwrap_or_else(|e| tracing::error!("Tooltip: {e}"));
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
            Event::WindowEvent { window_id, event } => {
                let is_qr = qr_state.borrow().as_ref()
                    .map(|qw| qw.window.id() == window_id).unwrap_or(false);
                if is_qr {
                    match event {
                        WindowEvent::CloseRequested
                        | WindowEvent::KeyboardInput { .. }
                        | WindowEvent::MouseInput { .. } => {
                            *qr_state.borrow_mut() = None;
                        }
                        WindowEvent::CursorEntered { .. } => {
                            // User is hovering over QR — keep it open
                            last_click = Instant::now();
                        }
                        WindowEvent::RedrawRequested => {
                            if let Some(qw) = qr_state.borrow().as_ref() {
                                render_qr(&qw.window, &url_clone);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }).expect("Event loop error");

    std::thread::sleep(Duration::from_millis(300));
}

// ── QR pixel generation ─────────────────────────────────────────────

fn qr_pixels(url: &str, scale: u32) -> (u32, u32, Vec<u32>) {
    use qrcode::QrCode;
    use qrcode::render::unicode;

    let code = QrCode::new(url).expect("QR generation failed");
    let text = code.render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Dark)
        .light_color(unicode::Dense1x2::Light)
        .quiet_zone(false)
        .build();

    let lines: Vec<&str> = text.lines().collect();
    let cw = lines.first().map_or(0, |l| l.chars().count() as u32);
    let ch = lines.len() as u32;
    let pw = cw * scale;
    let ph = ch * 2 * scale;
    let mut pixels = vec![0xFFFFFFFFu32; (pw * ph) as usize];

    for (y, line) in lines.iter().enumerate() {
        for (x, ch) in line.chars().enumerate() {
            let (t, b) = match ch {
                '█' => (true, true),
                '▀' => (true, false),
                '▄' => (false, true),
                _ => (false, false),
            };
            for dy in 0..(2 * scale) {
                for dx in 0..scale {
                    let px = x as u32 * scale + dx;
                    let py = y as u32 * 2 * scale + dy;
                    if if dy < scale { t } else { b } {
                        pixels[(py * pw + px) as usize] = 0xFF000000;
                    }
                }
            }
        }
    }
    (pw, ph, pixels)
}

fn render_qr(window: &winit::window::Window, url: &str) {
    let s = physical_scale(window);
    let (pw, ph, qr) = qr_pixels(url, QR_SCALE * s);
    let pad = QR_PAD * s;
    let ww = pw + pad * 2;
    let wh = ph + pad * 2;

    let ctx = softbuffer::Context::new(window).expect("softbuffer ctx");
    let mut surface = softbuffer::Surface::new(&ctx, window).expect("softbuffer surf");

    // Surface matches window physical size — only resize if window not ready yet
    let phys = window.inner_size();
    if phys.width != ww || phys.height != wh {
        if let (Some(w), Some(h)) = (NonZeroU32::new(ww), NonZeroU32::new(wh)) {
            surface.resize(w, h).ok();
        }
    }

    let mut buf = surface.buffer_mut().expect("softbuffer buf");
    buf.fill(0xFFFFFFFF);

    let buf_len = buf.len();
    for row in 0..ph as usize {
        let src = row * pw as usize;
        let dst = (row + pad as usize) * ww as usize + pad as usize;
        if dst + pw as usize <= buf_len {
            buf[dst..dst + pw as usize].copy_from_slice(&qr[src..src + pw as usize]);
        }
    }
    buf.present().expect("softbuffer present");
}

fn open_qr_window(
    url: &str,
    elwt: &winit::event_loop::EventLoopWindowTarget<TrayEvent>,
    state: &Rc<RefCell<Option<QrWindow>>>,
) {
    *state.borrow_mut() = None;

    let (pw, ph, _) = qr_pixels(url, QR_SCALE);
    let ww = pw + QR_PAD * 2;
    let wh = ph + QR_PAD * 2;

    let window = WindowBuilder::new()
        .with_inner_size(winit::dpi::LogicalSize::new(ww as f64, wh as f64))
        .with_resizable(false)
        .with_decorations(false)
        .with_window_level(WindowLevel::AlwaysOnTop)
        .with_title("TypeBridge QR")
        .build(elwt)
        .expect("Failed to create QR window");

    // Center horizontally, upper vertically
    if let Some(monitor) = elwt.primary_monitor() {
        let screen = monitor.size();
        let x = (screen.width as i32 - ww as i32) / 2;
        let y = ((screen.height as f64) * 0.10) as i32;
        window.set_outer_position(PhysicalPosition::new(x.max(0), y.max(0)));
    }

    window.request_redraw();
    *state.borrow_mut() = Some(QrWindow { window });
}

// ── Terminal QR + clipboard ─────────────────────────────────────────

fn print_qr(url: &str) {
    use qrcode::QrCode;
    use qrcode::render::unicode;
    let code = QrCode::new(url).expect("QR generation failed");
    println!("{}", code.render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Dark)
        .light_color(unicode::Dense1x2::Light)
        .build());
}

fn copy_to_clipboard(url: &str) {
    match arboard::Clipboard::new() {
        Ok(mut c) => { let _ = c.set_text(url); }
        Err(e) => tracing::error!("Clipboard: {e}"),
    }
}
