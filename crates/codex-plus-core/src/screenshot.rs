use std::io::Cursor;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use screenshots::Screen;
use screenshots::display_info::DisplayInfo;
use screenshots::image::{DynamicImage, ImageOutputFormat, RgbaImage};
use serde_json::{Value, json};

pub fn capture_screenshot_response(payload: &Value) -> Value {
    match capture_screenshot(payload) {
        Ok(value) => value,
        Err(error) => json!({
            "status": "failed",
            "message": format!("截图失败：{error}")
        }),
    }
}

fn capture_screenshot(payload: &Value) -> anyhow::Result<Value> {
    let all_displays = payload
        .get("allDisplays")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let screens = if all_displays {
        sorted_screens(Screen::all().context("未找到可截图的显示器")?)
    } else {
        vec![screen_for_payload_point(payload)?]
    };
    if screens.is_empty() {
        return Err(anyhow!("未找到可截图的显示器"));
    }

    let captured_at_ms = now_ms();
    let files = screens
        .iter()
        .enumerate()
        .map(|(index, screen)| screenshot_file_value(screen, index, captured_at_ms))
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(json!({
        "status": "ok",
        "message": if files.len() > 1 { "已截取全部显示器" } else { "已截取当前显示器" },
        "capturedAtMs": captured_at_ms,
        "displayCount": files.len(),
        "files": files,
    }))
}

fn screen_for_payload_point(payload: &Value) -> anyhow::Result<Screen> {
    let screen_x = payload
        .get("screenX")
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok());
    let screen_y = payload
        .get("screenY")
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok());
    if let (Some(x), Some(y)) = (screen_x, screen_y) {
        if let Ok(screen) = Screen::from_point(x, y) {
            return Ok(screen);
        }
    }

    let screens = sorted_screens(Screen::all().context("未找到可截图的显示器")?);
    screens
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("未找到可截图的显示器"))
}

fn sorted_screens(mut screens: Vec<Screen>) -> Vec<Screen> {
    screens.sort_by(|left, right| {
        right
            .display_info
            .is_primary
            .cmp(&left.display_info.is_primary)
            .then(left.display_info.x.cmp(&right.display_info.x))
            .then(left.display_info.y.cmp(&right.display_info.y))
            .then(left.display_info.id.cmp(&right.display_info.id))
    });
    screens
}

fn screenshot_file_value(
    screen: &Screen,
    index: usize,
    captured_at_ms: u128,
) -> anyhow::Result<Value> {
    let image = screen
        .capture()
        .with_context(|| format!("无法截取显示器 {}", display_label(&screen.display_info, index)))?;
    let width = image.width();
    let height = image.height();
    let png = rgba_image_to_png(image)?;
    Ok(json!({
        "filename": screenshot_filename(&screen.display_info, index, captured_at_ms),
        "contentType": "image/png",
        "dataBase64": BASE64_STANDARD.encode(&png),
        "sizeBytes": png.len(),
        "width": width,
        "height": height,
        "display": display_info_value(&screen.display_info, index),
    }))
}

fn rgba_image_to_png(image: RgbaImage) -> anyhow::Result<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut cursor, ImageOutputFormat::Png)
        .context("PNG 编码失败")?;
    Ok(cursor.into_inner())
}

fn screenshot_filename(display: &DisplayInfo, index: usize, captured_at_ms: u128) -> String {
    let name = display_label(display, index)
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    format!(
        "codex-screenshot-{}-{}.png",
        captured_at_ms,
        if name.is_empty() {
            format!("display-{}", index + 1)
        } else {
            name
        }
    )
}

fn display_label(display: &DisplayInfo, index: usize) -> String {
    let name = if display.friendly_name.trim().is_empty() {
        display.name.trim()
    } else {
        display.friendly_name.trim()
    };
    if name.is_empty() {
        format!("display-{}", index + 1)
    } else {
        name.to_string()
    }
}

fn display_info_value(display: &DisplayInfo, index: usize) -> Value {
    json!({
        "index": index,
        "id": display.id,
        "name": display.name.as_str(),
        "friendlyName": display.friendly_name.as_str(),
        "x": display.x,
        "y": display.y,
        "width": display.width,
        "height": display.height,
        "scaleFactor": display.scale_factor,
        "rotation": display.rotation,
        "isPrimary": display.is_primary,
        "isBuiltin": display.is_builtin,
    })
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
