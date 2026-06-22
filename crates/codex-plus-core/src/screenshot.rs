use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::anyhow;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde_json::{json, Value};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

pub fn capture_screenshot_response(payload: &Value) -> Value {
    match capture_screenshot(payload) {
        Ok(value) => value,
        Err(ScreenshotCaptureError::Cancelled(message)) => json!({
            "status": "cancelled",
            "message": message
        }),
        Err(ScreenshotCaptureError::Failed(error)) => json!({
            "status": "failed",
            "message": format!("截图失败：{error}")
        }),
    }
}

fn capture_screenshot(payload: &Value) -> CaptureResult<Value> {
    capture_region_screenshot(payload)
}

type CaptureResult<T> = Result<T, ScreenshotCaptureError>;

#[derive(Debug)]
enum ScreenshotCaptureError {
    Cancelled(String),
    Failed(anyhow::Error),
}

impl From<anyhow::Error> for ScreenshotCaptureError {
    fn from(error: anyhow::Error) -> Self {
        Self::Failed(error)
    }
}

fn capture_region_screenshot(payload: &Value) -> CaptureResult<Value> {
    let captured_at_ms = now_ms();
    let hide_guard = CodexWindowHideGuard::maybe_hide(
        payload
            .get("hideCodexWindow")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    );
    if hide_guard.any_hidden() {
        thread::sleep(Duration::from_millis(240));
    }

    let png = capture_region_with_flameshot(captured_at_ms)?;
    let (width, height) = png_dimensions(&png)?;
    Ok(json!({
        "status": "ok",
        "message": "已截取区域截图",
        "capturedAtMs": captured_at_ms,
        "displayCount": 1,
        "provider": "flameshot",
        "files": [{
            "filename": format!("codex-screenshot-{captured_at_ms}-region.png"),
            "contentType": "image/png",
            "dataBase64": BASE64_STANDARD.encode(&png),
            "sizeBytes": png.len(),
            "width": width,
            "height": height,
            "display": {
                "index": 0,
                "id": "region",
                "isRegion": true
            }
        }],
    }))
}

fn capture_region_with_flameshot(captured_at_ms: u128) -> CaptureResult<Vec<u8>> {
    let output_path = env::temp_dir().join(format!(
        "codex-plus-region-screenshot-{captured_at_ms}-{}.png",
        std::process::id()
    ));
    let _ = fs::remove_file(&output_path);

    let command_path = bundled_flameshot_command().ok_or_else(|| {
        ScreenshotCaptureError::Failed(anyhow!(
            "安装包缺少内置 Flameshot，请重新安装 Codex++ 或重新构建安装包"
        ))
    })?;
    match run_flameshot_gui(&command_path, &output_path) {
        Ok(bytes) => Ok(bytes),
        Err(FlameshotRunError::Cancelled(message)) => {
            let _ = fs::remove_file(&output_path);
            Err(ScreenshotCaptureError::Cancelled(message))
        }
        Err(FlameshotRunError::Unavailable(message)) => Err(ScreenshotCaptureError::Failed(
            anyhow!("内置 Flameshot 启动失败：{message}"),
        )),
    }
}

#[derive(Debug)]
enum FlameshotRunError {
    Cancelled(String),
    Unavailable(String),
}

fn run_flameshot_gui(
    command_path: &Path,
    output_path: &Path,
) -> Result<Vec<u8>, FlameshotRunError> {
    let _ = fs::remove_file(output_path);
    let mut command = Command::new(command_path);
    command.arg("gui").arg("--path").arg(output_path);
    if let Some(dir) = command_path.parent() {
        command.current_dir(dir);
    }
    #[cfg(windows)]
    {
        command.creation_flags(crate::windows_create_no_window());
    }

    let output = command.output().map_err(|error| {
        FlameshotRunError::Unavailable(format!("{}: {}", command_path.display(), error))
    })?;

    if output_path.exists() {
        if let Some(bytes) = read_nonempty_file_after_flush(output_path) {
            let _ = fs::remove_file(output_path);
            return Ok(bytes);
        }
    }

    if output.status.success() {
        return Err(FlameshotRunError::Cancelled(
            "已取消区域截图或未保存截图".to_string(),
        ));
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let details = [stderr, stdout]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("；");
    Err(FlameshotRunError::Unavailable(format!(
        "{} 返回 {}{}",
        command_path.display(),
        output.status,
        if details.is_empty() {
            String::new()
        } else {
            format!("：{details}")
        }
    )))
}

fn read_nonempty_file_after_flush(path: &Path) -> Option<Vec<u8>> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Ok(bytes) = fs::read(path) {
            if !bytes.is_empty() {
                return Some(bytes);
            }
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(80));
    }
}

fn png_dimensions(bytes: &[u8]) -> anyhow::Result<(u32, u32)> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[..8] != PNG_SIGNATURE {
        return Err(anyhow!("Flameshot 未返回有效 PNG"));
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap_or_default());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap_or_default());
    if width == 0 || height == 0 {
        return Err(anyhow!("Flameshot 返回的 PNG 尺寸无效"));
    }
    Ok((width, height))
}

fn bundled_flameshot_command() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    bundled_flameshot_candidates_for_exe(&exe)
        .into_iter()
        .find(|path| path.is_file())
}

fn bundled_flameshot_candidates_for_exe(exe: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(dir) = exe.parent() {
        #[cfg(windows)]
        {
            candidates.push(
                dir.join("tools")
                    .join("flameshot")
                    .join("flameshot-cli.exe"),
            );
            candidates.push(dir.join("tools").join("flameshot").join("flameshot.exe"));
            candidates.push(
                dir.join("tools")
                    .join("flameshot")
                    .join("bin")
                    .join("flameshot-cli.exe"),
            );
            candidates.push(
                dir.join("tools")
                    .join("flameshot")
                    .join("bin")
                    .join("flameshot.exe"),
            );
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(contents_dir) = dir.parent() {
                candidates.push(
                    contents_dir
                        .join("Resources")
                        .join("tools")
                        .join("flameshot")
                        .join("flameshot.app")
                        .join("Contents")
                        .join("MacOS")
                        .join("flameshot"),
                );
            }
            candidates.push(
                dir.join("tools")
                    .join("flameshot")
                    .join("flameshot.app")
                    .join("Contents")
                    .join("MacOS")
                    .join("flameshot"),
            );
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            candidates.push(dir.join("tools").join("flameshot").join("flameshot"));
        }
    }
    candidates
}

struct CodexWindowHideGuard {
    #[cfg(windows)]
    hidden_windows: Vec<crate::windows_integration::HiddenWindow>,
}

impl CodexWindowHideGuard {
    #[cfg(windows)]
    fn maybe_hide(enabled: bool) -> Self {
        if !enabled {
            return Self {
                hidden_windows: Vec::new(),
            };
        }
        let process_ids = crate::watcher::find_codex_processes();
        Self {
            hidden_windows: crate::windows_integration::hide_process_windows(&process_ids),
        }
    }

    #[cfg(not(windows))]
    fn maybe_hide(_enabled: bool) -> Self {
        Self {}
    }

    #[cfg(windows)]
    fn any_hidden(&self) -> bool {
        !self.hidden_windows.is_empty()
    }

    #[cfg(not(windows))]
    fn any_hidden(&self) -> bool {
        false
    }
}

impl Drop for CodexWindowHideGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            crate::windows_integration::restore_hidden_windows(&self.hidden_windows);
        }
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_png_dimensions_from_header() {
        let bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x02\0\0\0\x03";
        assert_eq!(png_dimensions(bytes).unwrap(), (2, 3));
    }

    #[test]
    fn rejects_invalid_png_header() {
        assert!(png_dimensions(b"not-a-png").is_err());
    }

    #[test]
    fn bundled_flameshot_candidates_are_app_relative() {
        let candidates = bundled_flameshot_candidates_for_exe(Path::new("/tmp/codex-plus-plus"));
        assert!(candidates.iter().all(|path| path.starts_with("/tmp/tools")));
    }
}
