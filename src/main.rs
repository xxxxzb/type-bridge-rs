mod assets;
mod ip;
mod keyboard;
mod server;
mod tray;
mod tray_icons;

use clap::Parser;
use std::cell::RefCell;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::mpsc;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tray_icon::menu::MenuEvent;
use winit::dpi::PhysicalPosition;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoopBuilder;
use winit::event_loop::EventLoopProxy;
use winit::window::{WindowBuilder, WindowLevel};

/// Max commands drained from the channel per event-loop tick.
const MAX_COMMANDS_PER_TICK: usize = 32;
/// Bounded channel capacity for keyboard commands.
const KB_CHANNEL_BOUND: usize = 256;

#[derive(Debug)]
enum TrayEvent {
    Menu(muda::MenuId),
    TrayClick,
}

/// Wraps `EventLoopProxy` for sharing across threads.
///
/// # Safety
///
/// `EventLoopProxy<T>` is documented by winit as safe to share across threads:
/// only `send_event` is called, which wakes the event loop and is internally
/// synchronized. No other access to the proxy or event loop state is performed
/// through this wrapper.
struct SyncProxy(EventLoopProxy<TrayEvent>);
// SAFETY: EventLoopProxy::send_event is thread-safe per winit docs.
unsafe impl Sync for SyncProxy {}

struct QrWindow {
    window: winit::window::Window,
    /// Cached QR pixel data. Rebuilt when display scale changes.
    qr_pixels: Option<(u32, u32, Vec<u32>)>,
    /// The physical_scale used to build qr_pixels; cache miss on mismatch.
    qr_cached_scale: u32,
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

// ── Token generation ─────────────────────────────────────────────────

fn generate_token() -> String {
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf).expect("Failed to generate random token (getrandom)");
    // base64url without padding — URL-safe, no extra encoding
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(22);
    let mut bits: u32 = 0;
    let mut bit_count: u32 = 0;
    for &byte in &buf {
        bits = (bits << 8) | byte as u32;
        bit_count += 8;
        while bit_count >= 6 {
            bit_count -= 6;
            let idx = (bits >> bit_count) & 0x3F;
            out.push(ALPHABET[idx as usize] as char);
        }
    }
    if bit_count > 0 {
        let idx = (bits << (6 - bit_count)) & 0x3F;
        out.push(ALPHABET[idx as usize] as char);
    }
    out
}

fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let ip = ip::get_local_ip();
    let port = cli.port;
    let token = generate_token();
    let url = format!("http://{}:{}/?token={}", ip, port, token);

    println!("\n⌨️  TypeBridge running!");
    println!("📱 Open on your phone: {url}");
    print_qr(&url);

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let shutdown_tx = std::cell::Cell::new(Some(shutdown_tx));

    let (kb_tx, kb_rx) = mpsc::sync_channel::<keyboard::KeyCommand>(KB_CHANNEL_BOUND);
    keyboard::init_command_queue(kb_tx);

    let app_state = Arc::new(server::AppState {
        token: token.clone(),
        history: Arc::new(Mutex::new(VecDeque::with_capacity(30))),
    });

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async { server::run(port, app_state, shutdown_rx).await });
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

    let url_for_tray = format!("http://{}:{}/?token={}", ip, port, token);
    let tray_state = Rc::new(RefCell::new(tray::build_tray(&url_for_tray)));
    let toggle_id = tray_state.borrow().toggle_id.clone();
    let show_qr_id = tray_state.borrow().show_qr_id.clone();
    let copy_url_id = tray_state.borrow().copy_url_id.clone();
    let quit_id = tray_state.borrow().quit_id.clone();

    let proxy2 = proxy.clone();
    MenuEvent::set_event_handler(Some(move |event: tray_icon::menu::MenuEvent| {
        let _ = proxy2.0.send_event(TrayEvent::Menu(event.id));
    }));

    let proxy3 = proxy.clone();
    tray_icon::TrayIconEvent::set_event_handler(Some(move |event: tray_icon::TrayIconEvent| {
        if matches!(event, tray_icon::TrayIconEvent::Click { .. }) {
            let _ = proxy3.0.send_event(TrayEvent::TrayClick);
        }
    }));

    let qr_state: Rc<RefCell<Option<QrWindow>>> = Rc::new(RefCell::new(None));
    let url_clone = url.clone();
    let mut last_click = Instant::now();

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(200),
            ));

            // Drain commands with tick limit + merge optimization
            let mut pending: Vec<keyboard::KeyCommand> = Vec::new();
            for _ in 0..MAX_COMMANDS_PER_TICK {
                match kb_rx.try_recv() {
                    Ok(cmd) => pending.push(cmd),
                    Err(_) => break,
                }
            }
            if pending.len() == MAX_COMMANDS_PER_TICK {
                tracing::warn!(
                    "Reached per-tick command limit ({MAX_COMMANDS_PER_TICK}); \
                     remaining commands deferred to next tick"
                );
            }
            for cmd in keyboard::merge_commands(pending) {
                keyboard::execute(cmd);
            }

            // Auto-close QR window 5s after last interaction
            if qr_state.borrow().is_some() && last_click.elapsed() > Duration::from_secs(5) {
                *qr_state.borrow_mut() = None;
                last_click = Instant::now();
            }

            match event {
                Event::UserEvent(TrayEvent::TrayClick) => {
                    last_click = Instant::now();
                    open_qr_window(&url_clone, elwt, &qr_state);
                }
                Event::UserEvent(TrayEvent::Menu(id)) => {
                    *qr_state.borrow_mut() = None;
                    last_click = Instant::now();

                    if id == toggle_id {
                        let enabled = keyboard::is_enabled();
                        keyboard::set_enabled(!enabled);
                        let new_state = keyboard::is_enabled();
                        let status = if new_state { "ON" } else { "PAUSED" };
                        let state = tray_state.borrow_mut();
                        state
                            .tray
                            .set_icon(Some(crate::tray::make_icon(new_state)))
                            .unwrap_or_else(|e| tracing::error!("Tray icon: {e}"));
                        state
                            .tray
                            .set_tooltip(Some(format!("TypeBridge — {}\n{}", status, url_for_tray)))
                            .unwrap_or_else(|e| tracing::error!("Tooltip: {e}"));
                        state.status_item.set_text(format!("Typing: {status}"));
                    } else if id == show_qr_id {
                        open_qr_window(&url_clone, elwt, &qr_state);
                    } else if id == copy_url_id {
                        copy_to_clipboard(&url_for_tray);
                    } else if id == quit_id {
                        tracing::info!("Shutting down...");
                        if let Some(tx) = shutdown_tx.take() {
                            let _ = tx.send(());
                        }
                        elwt.exit();
                    }
                }
                Event::WindowEvent { window_id, event } => {
                    let is_qr = qr_state
                        .borrow()
                        .as_ref()
                        .map(|qw| qw.window.id() == window_id)
                        .unwrap_or(false);
                    if is_qr {
                        match event {
                            WindowEvent::CloseRequested
                            | WindowEvent::KeyboardInput { .. }
                            | WindowEvent::MouseInput { .. } => {
                                *qr_state.borrow_mut() = None;
                            }
                            WindowEvent::CursorEntered { .. } => {
                                last_click = Instant::now();
                            }
                            WindowEvent::RedrawRequested => {
                                let mut state = qr_state.borrow_mut();
                                if let Some(qw) = state.as_mut() {
                                    render_qr(&url_clone, qw);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        })
        .expect("Event loop error");

    std::thread::sleep(Duration::from_millis(300));
}

// ── QR pixel generation ─────────────────────────────────────────────

fn qr_pixels(url: &str, scale: u32) -> (u32, u32, Vec<u32>) {
    use qrcode::render::unicode;
    use qrcode::QrCode;

    let code = QrCode::new(url).expect("QR generation failed");
    let text = code
        .render::<unicode::Dense1x2>()
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

fn render_qr(url: &str, qw: &mut QrWindow) {
    let window = &qw.window;
    let s = physical_scale(window);

    // Regenerate QR pixels only when scale changes or not yet cached
    if qw.qr_pixels.is_none() || qw.qr_cached_scale != s {
        qw.qr_pixels = Some(qr_pixels(url, QR_SCALE * s));
        qw.qr_cached_scale = s;
    }

    let (pw, ph) = match &qw.qr_pixels {
        Some((pw, ph, _)) => (*pw, *ph),
        None => return,
    };
    let qr = &qw.qr_pixels.as_ref().unwrap().2;

    let pad = QR_PAD * s;
    let ww = pw + pad * 2;
    let wh = ph + pad * 2;

    let ctx = softbuffer::Context::new(window).expect("softbuffer ctx");
    let mut surface = softbuffer::Surface::new(&ctx, window).expect("softbuffer surf");

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
    // Compute QR at logical scale for initial window sizing
    let (pw, ph, _pixels) = qr_pixels(url, QR_SCALE);
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

    if let Some(monitor) = elwt.primary_monitor() {
        let screen = monitor.size();
        let x = (screen.width as i32 - ww as i32) / 2;
        let y = ((screen.height as f64) * 0.10) as i32;
        window.set_outer_position(PhysicalPosition::new(x.max(0), y.max(0)));
    }

    window.request_redraw();
    *state.borrow_mut() = Some(QrWindow {
        window,
        qr_pixels: None,
        qr_cached_scale: 0,
    });
}

// ── Terminal QR + clipboard ─────────────────────────────────────────

fn print_qr(url: &str) {
    use qrcode::render::unicode;
    use qrcode::QrCode;
    let code = QrCode::new(url).expect("QR generation failed");
    println!(
        "{}",
        code.render::<unicode::Dense1x2>()
            .dark_color(unicode::Dense1x2::Dark)
            .light_color(unicode::Dense1x2::Light)
            .build()
    );
}

fn copy_to_clipboard(url: &str) {
    match arboard::Clipboard::new() {
        Ok(mut c) => {
            let _ = c.set_text(url);
        }
        Err(e) => tracing::error!("Clipboard: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_token_length() {
        let t = generate_token();
        // 128 bits → 22 base64url chars (no padding)
        assert_eq!(t.len(), 22, "token length: {t}");
    }

    #[test]
    fn test_generate_token_charset() {
        let t = generate_token();
        let allowed = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
        for (i, c) in t.chars().enumerate() {
            assert!(
                allowed(c),
                "unexpected char U+{:04X} at pos {i} in {t:?}",
                c as u32
            );
        }
    }

    #[test]
    fn test_generate_token_unique() {
        let a = generate_token();
        let b = generate_token();
        // Extremely unlikely to collide with 128 bits of entropy
        assert_ne!(a, b);
    }

    #[test]
    fn test_generate_token_no_equals() {
        let t = generate_token();
        assert!(!t.contains('='), "token must not contain padding: {t}");
    }
}
