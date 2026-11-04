//! Draws Abode's app icon and writes it in the formats the installers need.
//!
//! ```sh
//! cargo run -p abode-app --example make_icon
//! ```
//!
//! Writes `assets/icon/icon.png` (1024²), `icon.ico` (Windows) and, on macOS,
//! `icon.icns` (via `iconutil`).

use image::{Rgba, RgbaImage, imageops::FilterType};
use std::path::Path;

const FOREST: [f32; 3] = [14.0, 74.0, 66.0];
const FOREST_LIGHT: [f32; 3] = [30.0, 104.0, 92.0];
const CREAM: [f32; 3] = [250.0, 246.0, 236.0];
const CORAL: [f32; 3] = [234.0, 90.0, 54.0];
const MINT: [f32; 3] = [221.0, 236.0, 231.0];

fn inside_rounded_rect(x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> bool {
    if x < x0 || x > x1 || y < y0 || y > y1 {
        return false;
    }
    let cx = x.clamp(x0 + r, x1 - r);
    let cy = y.clamp(y0 + r, y1 - r);
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}

fn inside_polygon(x: f32, y: f32, poly: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[(i + n - 1) % n];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
    }
    inside
}

/// Colour at a point in a 1024×1024 canvas, or None for transparent.
fn shade(x: f32, y: f32) -> Option<[f32; 3]> {
    // macOS-style tile: 824 px square with ~22% corner radius, centred.
    let (m, r) = (100.0, 185.0);
    if !inside_rounded_rect(x, y, m, m, 1024.0 - m, 1024.0 - m, r) {
        return None;
    }
    let t = ((y - m) / (1024.0 - 2.0 * m)).clamp(0.0, 1.0);
    let bg = [0, 1, 2].map(|i| FOREST_LIGHT[i] + (FOREST[i] - FOREST_LIGHT[i]) * t);

    // House: a pitched roof over a body, in cream.
    let roof = [(512.0, 250.0), (790.0, 480.0), (734.0, 480.0), (512.0, 296.0), (290.0, 480.0), (234.0, 480.0)];
    let body = [(318.0, 455.0), (512.0, 296.0), (706.0, 455.0), (706.0, 770.0), (318.0, 770.0)];
    let door = (472.0, 600.0, 552.0, 770.0);
    let window = (580.0, 520.0, 650.0, 590.0);
    let chimney = [(640.0, 300.0), (690.0, 300.0), (690.0, 400.0), (640.0, 359.0)];

    if x >= door.0 && x <= door.2 && y >= door.1 && y <= door.3 && inside_rounded_rect(x, y, door.0, door.1, door.2, door.3 + 40.0, 30.0) {
        return Some(CORAL);
    }
    if inside_rounded_rect(x, y, window.0, window.1, window.2, window.3, 12.0) {
        return Some(MINT);
    }
    if inside_polygon(x, y, &roof) || inside_polygon(x, y, &chimney) {
        return Some(CORAL);
    }
    if inside_polygon(x, y, &body) {
        return Some(CREAM);
    }
    // Ground line
    if (790.0..=812.0).contains(&y) && (250.0..=774.0).contains(&x) {
        return Some(MINT);
    }
    Some(bg)
}

fn render() -> RgbaImage {
    let size = 1024u32;
    let ss = 4; // supersampling per axis
    let mut img = RgbaImage::new(size, size);
    for py in 0..size {
        for px in 0..size {
            let mut acc = [0.0f32; 4];
            for sy in 0..ss {
                for sx in 0..ss {
                    let x = px as f32 + (sx as f32 + 0.5) / ss as f32;
                    let y = py as f32 + (sy as f32 + 0.5) / ss as f32;
                    if let Some(c) = shade(x, y) {
                        acc[0] += c[0];
                        acc[1] += c[1];
                        acc[2] += c[2];
                        acc[3] += 1.0;
                    }
                }
            }
            let n = (ss * ss) as f32;
            let a = acc[3] / n;
            let px_col = if acc[3] > 0.0 {
                [acc[0] / acc[3], acc[1] / acc[3], acc[2] / acc[3]]
            } else {
                [0.0; 3]
            };
            img.put_pixel(px, py, Rgba([px_col[0] as u8, px_col[1] as u8, px_col[2] as u8, (a * 255.0).round() as u8]));
        }
    }
    img
}

fn main() -> anyhow::Result<()> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icon");
    std::fs::create_dir_all(&dir)?;
    let img = render();
    img.save(dir.join("icon.png"))?;

    // Windows .ico with the usual sizes.
    let sizes = [16u32, 24, 32, 48, 64, 128, 256];
    let frames: Vec<image::codecs::ico::IcoFrame> = sizes
        .iter()
        .map(|&s| {
            let small = image::imageops::resize(&img, s, s, FilterType::Lanczos3);
            image::codecs::ico::IcoFrame::as_png(small.as_raw(), s, s, image::ExtendedColorType::Rgba8).unwrap()
        })
        .collect();
    let file = std::fs::File::create(dir.join("icon.ico"))?;
    image::codecs::ico::IcoEncoder::new(file).encode_images(&frames)?;

    // macOS .icns via iconutil.
    if cfg!(target_os = "macos") {
        let set = std::env::temp_dir().join("abode.iconset");
        let _ = std::fs::remove_dir_all(&set);
        std::fs::create_dir_all(&set)?;
        for s in [16u32, 32, 128, 256, 512] {
            image::imageops::resize(&img, s, s, FilterType::Lanczos3).save(set.join(format!("icon_{s}x{s}.png")))?;
            image::imageops::resize(&img, s * 2, s * 2, FilterType::Lanczos3).save(set.join(format!("icon_{s}x{s}@2x.png")))?;
        }
        let status = std::process::Command::new("iconutil")
            .args(["-c", "icns", "-o"])
            .arg(dir.join("icon.icns"))
            .arg(&set)
            .status()?;
        anyhow::ensure!(status.success(), "iconutil failed");
    }
    println!("Wrote icons to {}", dir.display());
    Ok(())
}
