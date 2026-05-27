use muda::{Menu, MenuItem, PredefinedMenuItem};
use tray_icon::Icon;
use tray_icon::TrayIcon;
use tray_icon::TrayIconBuilder;

pub struct TrayState {
    pub tray: TrayIcon,
    pub toggle_id: muda::MenuId,
    pub copy_url_id: muda::MenuId,
    pub quit_id: muda::MenuId,
}

fn make_icon_rgba(color: [u8; 4]) -> Vec<u8> {
    let size = 32u32;
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let center = size as f64 / 2.0;
    let outer_r = 14.0;
    let inner_r = 6.0;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f64 - center;
            let dy = y as f64 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = ((y * size + x) * 4) as usize;

            let key_left = 6.0;
            let key_right = 26.0;
            let key_top = 12.0;
            let key_bottom = 24.0;
            let corner_r = 3.0;

            let in_rounded = {
                let cx = x as f64;
                let cy = y as f64;
                let left = key_left + corner_r;
                let right = key_right - corner_r;
                let top = key_top + corner_r;
                let bottom = key_bottom - corner_r;

                if (cx >= left && cx <= right && cy >= key_top && cy <= key_bottom)
                    || (cx >= key_left && cx <= key_right && cy >= top && cy <= bottom)
                {
                    true
                } else {
                    let corners = [
                        (key_left + corner_r, key_top + corner_r),
                        (key_right - corner_r, key_top + corner_r),
                        (key_left + corner_r, key_bottom - corner_r),
                        (key_right - corner_r, key_bottom - corner_r),
                    ];
                    corners.iter().any(|(corner_x, corner_y)| {
                        let ddx = cx - corner_x;
                        let ddy = cy - corner_y;
                        (ddx * ddx + ddy * ddy).sqrt() <= corner_r
                    })
                }
            };

            if in_rounded || (dist < outer_r && dist > inner_r) {
                pixels[idx] = color[0];
                pixels[idx + 1] = color[1];
                pixels[idx + 2] = color[2];
                pixels[idx + 3] = color[3];
            }
        }
    }
    pixels
}

fn qr_lines(url: &str) -> Vec<String> {
    use qrcode::QrCode;
    use qrcode::render::unicode;

    let code = QrCode::new(url).expect("Failed to generate QR code");
    let text = code
        .render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Dark)
        .light_color(unicode::Dense1x2::Light)
        .build();

    text.lines().map(|l| l.to_string()).collect()
}

pub fn make_icon(active: bool) -> Icon {
    let color = if active {
        [0, 220, 100, 255]
    } else {
        [220, 80, 80, 255]
    };
    let rgba = make_icon_rgba(color);
    Icon::from_rgba(rgba, 32, 32).expect("Failed to create tray icon from RGBA data")
}

pub fn build_tray(ip: &str, port: u16) -> TrayState {
    let menu = Menu::new();

    // URL display (disabled)
    let url_item = MenuItem::new(format!("http://{}:{}", ip, port), false, None);
    menu.append(&url_item)
        .unwrap_or_else(|e| tracing::error!("Failed to add URL item: {e}"));

    menu.append(&PredefinedMenuItem::separator())
        .unwrap_or_else(|e| tracing::error!("Failed to add separator: {e}"));

    // QR code rows (disabled, for display only)
    let url = format!("http://{}:{}", ip, port);
    let qr_items: Vec<MenuItem> = qr_lines(&url)
        .into_iter()
        .map(|line| MenuItem::new(line, false, None))
        .collect();

    for item in &qr_items {
        menu.append(item)
            .unwrap_or_else(|e| tracing::error!("Failed to add QR row: {e}"));
    }

    menu.append(&PredefinedMenuItem::separator())
        .unwrap_or_else(|e| tracing::error!("Failed to add separator: {e}"));

    // Action items
    let copy_url = MenuItem::new("Copy URL", true, None);
    let toggle = MenuItem::new("Toggle Typing", true, None);
    let quit = MenuItem::new("Quit", true, None);

    menu.append(&copy_url)
        .unwrap_or_else(|e| tracing::error!("Failed to add Copy URL: {e}"));
    menu.append(&toggle)
        .unwrap_or_else(|e| tracing::error!("Failed to add Toggle: {e}"));
    menu.append(&quit)
        .unwrap_or_else(|e| tracing::error!("Failed to add Quit: {e}"));

    let toggle_id = toggle.id().clone();
    let copy_url_id = copy_url.id().clone();
    let quit_id = quit.id().clone();

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(make_icon(true))
        .with_tooltip(format!("TypeBridge — ON\nhttp://{}:{}", ip, port))
        .build()
        .expect("Failed to create system tray icon");

    TrayState {
        tray,
        toggle_id,
        copy_url_id,
        quit_id,
    }
}
