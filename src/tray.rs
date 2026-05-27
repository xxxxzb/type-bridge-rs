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

    let url_item = MenuItem::new(format!("http://{}:{}", ip, port), false, None);
    menu.append(&url_item)
        .unwrap_or_else(|e| tracing::error!("Failed to add URL item: {e}"));

    menu.append(&PredefinedMenuItem::separator())
        .unwrap_or_else(|e| tracing::error!("Failed to add separator: {e}"));

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

#[cfg(test)]
#[allow(clippy::needless_range_loop)]
mod tests {
    use qrcode::QrCode;
    use qrcode::render::unicode;

    fn qr_lines(url: &str) -> Vec<String> {
        let code = QrCode::new(url).expect("Failed to generate QR code");
        let text = code
            .render::<unicode::Dense1x2>()
            .dark_color(unicode::Dense1x2::Dark)
            .light_color(unicode::Dense1x2::Light)
            .build();
        text.lines().map(|l| l.to_string()).collect()
    }

    fn is_qr_char(c: char) -> bool {
        matches!(c, ' ' | '█' | '▀' | '▄')
    }

    fn qr_to_modules(lines: &[String]) -> Vec<Vec<bool>> {
        let char_h = lines.len();
        let char_w = lines[0].chars().count();
        let mod_h = char_h * 2;
        let mod_w = char_w;
        let mut modules = vec![vec![false; mod_w]; mod_h];

        for (y, line) in lines.iter().enumerate() {
            for (x, ch) in line.chars().enumerate() {
                let (top, bottom) = match ch {
                    '█' => (true, true),
                    '▀' => (true, false),
                    '▄' => (false, true),
                    _ => (false, false),
                };
                modules[y * 2][x] = top;
                modules[y * 2 + 1][x] = bottom;
            }
        }
        modules
    }

    fn assert_finder(modules: &[Vec<bool>], row: usize, col: usize) {
        for i in 0..7 {
            assert!(modules[row][col + i]);
            assert!(modules[row + 6][col + i]);
            assert!(modules[row + i][col]);
            assert!(modules[row + i][col + 6]);
        }
        for c in 1..6 {
            assert!(!modules[row + 1][col + c]);
            assert!(!modules[row + 5][col + c]);
        }
        for r in 1..6 {
            assert!(!modules[row + r][col + 1]);
            assert!(!modules[row + r][col + 5]);
        }
        for r in 2..5 {
            for c in 2..5 {
                assert!(modules[row + r][col + c]);
            }
        }
    }

    #[test]
    fn test_qr_lines_count() {
        let lines = qr_lines("http://192.168.1.1:12345");
        assert!(lines.len() >= 12 && lines.len() <= 18);
    }

    #[test]
    fn test_qr_all_lines_same_width() {
        let lines = qr_lines("http://192.168.1.1:12345");
        let first = lines[0].chars().count();
        for (i, l) in lines.iter().enumerate() {
            assert_eq!(l.chars().count(), first, "line {i} width mismatch");
        }
    }

    #[test]
    fn test_qr_only_valid_chars() {
        for (i, line) in qr_lines("http://192.168.1.1:12345").iter().enumerate() {
            for (j, ch) in line.chars().enumerate() {
                assert!(is_qr_char(ch), "invalid U+{:04X} at ({i},{j})", ch as u32);
            }
        }
    }

    #[test]
    fn test_qr_idempotent() {
        assert_eq!(
            qr_lines("http://192.168.1.1:12345"),
            qr_lines("http://192.168.1.1:12345")
        );
    }

    #[test]
    fn test_qr_different_urls() {
        assert_ne!(
            qr_lines("http://192.168.1.1:1111"),
            qr_lines("http://192.168.1.1:2222")
        );
    }

    #[test]
    fn test_qr_finder_patterns() {
        let lines = qr_lines("http://192.168.1.1:12345");
        let w = lines[0].chars().count();
        let third = w / 3;
        let first = lines.iter().position(|l| l.contains('█') || l.contains('▀')).unwrap();
        let last = lines.iter().rposition(|l| l.contains('█') || l.contains('▀')).unwrap();
        let row0: Vec<char> = lines[first].chars().collect();
        let row_last: Vec<char> = lines[last].chars().collect();
        assert!(row0[..third].iter().any(|&c| c != ' '), "top-left missing");
        assert!(row0[2 * third..].iter().any(|&c| c != ' '), "top-right missing");
        assert!(row_last[..third].iter().any(|&c| c != ' '), "bottom-left missing");
    }

    #[test]
    fn test_qr_module_grid_correctness() {
        let lines = qr_lines("http://192.168.1.1:12345");
        let modules = qr_to_modules(&lines);
        let h = modules.len();
        let w = modules[0].len();
        let n = w - 8;

        for r in 0..4 {
            for c in 0..w {
                assert!(!modules[r][c], "top quiet at ({r},{c})");
            }
        }
        for c in 0..4 {
            for r in 0..h {
                assert!(!modules[r][c], "left quiet at ({r},{c})");
            }
        }

        assert_finder(&modules, 4, 4);
        assert_finder(&modules, 4, 4 + n - 7);
        assert_finder(&modules, 4 + n - 7, 4);

        assert!((0..w).any(|c| modules[10][c]));
        assert!((0..h).any(|r| modules[r][10]));

        assert_eq!(w % 2, 1);
        let black = modules.iter().flatten().filter(|&&m| m).count();
        assert!(black > 100, "too few black: {black}");
    }

    #[test]
    fn test_qr_grid_consistency() {
        let a = qr_to_modules(&qr_lines("http://192.168.1.1:12345"));
        let b = qr_to_modules(&qr_lines("http://192.168.1.1:12345"));
        assert_eq!(a, b);
    }

    #[test]
    fn test_qr_grid_different_urls() {
        assert_ne!(
            qr_to_modules(&qr_lines("http://192.168.1.1:1111")),
            qr_to_modules(&qr_lines("http://192.168.1.1:2222"))
        );
    }
}
