use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::anyhow;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde_json::{Value, json};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "macos")]
use std::process::Stdio;

const COMPLETED_JOB_TTL_MS: u128 = 5 * 60 * 1000;
const RUNNING_JOB_TTL_MS: u128 = 4 * 60 * 1000;
static NEXT_SCREENSHOT_JOB_ID: AtomicU64 = AtomicU64::new(1);
static SCREENSHOT_JOBS: OnceLock<Mutex<HashMap<String, ScreenshotJob>>> = OnceLock::new();

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

pub fn start_screenshot_response(payload: &Value) -> Value {
    let started_at_ms = now_ms();
    let mut jobs = screenshot_jobs()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    cleanup_screenshot_jobs_locked(&mut jobs, started_at_ms);
    if let Some((job_id, job)) = jobs
        .iter()
        .find(|(_, job)| matches!(job.state, ScreenshotJobState::Running))
    {
        return json!({
            "status": "running",
            "jobId": job_id,
            "startedAtMs": job.started_at_ms,
            "message": "已有截图选择窗口正在等待操作"
        });
    }

    let job_id = new_screenshot_job_id(started_at_ms);
    jobs.insert(
        job_id.clone(),
        ScreenshotJob {
            started_at_ms,
            updated_at_ms: started_at_ms,
            state: ScreenshotJobState::Running,
        },
    );
    drop(jobs);

    let job_id_for_thread = job_id.clone();
    let payload_for_thread = payload.clone();
    thread::spawn(move || {
        let result = capture_screenshot_response(&payload_for_thread);
        finish_screenshot_job(&job_id_for_thread, result);
    });

    json!({
        "status": "started",
        "jobId": job_id,
        "startedAtMs": started_at_ms,
        "message": "截图选择窗口已打开"
    })
}

pub fn screenshot_status_response(payload: &Value) -> Value {
    let job_id = payload
        .get("jobId")
        .or_else(|| payload.get("job_id"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if job_id.is_empty() {
        return json!({
            "status": "failed",
            "message": "缺少截图任务编号"
        });
    }

    let now = now_ms();
    let mut jobs = screenshot_jobs()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    cleanup_screenshot_jobs_locked(&mut jobs, now);
    match jobs.get(job_id) {
        Some(ScreenshotJob {
            started_at_ms,
            state: ScreenshotJobState::Running,
            ..
        }) => json!({
            "status": "running",
            "jobId": job_id,
            "startedAtMs": started_at_ms,
            "message": "等待选择截图区域"
        }),
        Some(ScreenshotJob {
            started_at_ms,
            state: ScreenshotJobState::Finished(result),
            ..
        }) => {
            let mut result = result.clone();
            if let Some(object) = result.as_object_mut() {
                object.insert("jobId".to_string(), json!(job_id));
                object.insert("startedAtMs".to_string(), json!(started_at_ms));
            }
            result
        }
        None => json!({
            "status": "failed",
            "message": "截图任务不存在或已过期"
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

#[derive(Clone)]
struct ScreenshotJob {
    started_at_ms: u128,
    updated_at_ms: u128,
    state: ScreenshotJobState,
}

#[derive(Clone)]
enum ScreenshotJobState {
    Running,
    Finished(Value),
}

#[derive(Debug, Clone, Copy)]
struct FlameshotOptions {
    delay_ms: u64,
    accept_on_select: bool,
}

impl From<anyhow::Error> for ScreenshotCaptureError {
    fn from(error: anyhow::Error) -> Self {
        Self::Failed(error)
    }
}

fn capture_region_screenshot(payload: &Value) -> CaptureResult<Value> {
    let captured_at_ms = now_ms();
    let flameshot_options = flameshot_options_from_payload(payload);
    let hide_guard = CodexWindowHideGuard::maybe_hide(
        payload
            .get("hideCodexWindow")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    );
    if hide_guard.any_hidden() {
        thread::sleep(Duration::from_millis(240));
    }

    let png = capture_region_with_flameshot(captured_at_ms, flameshot_options)?;
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

fn capture_region_with_flameshot(
    captured_at_ms: u128,
    options: FlameshotOptions,
) -> CaptureResult<Vec<u8>> {
    let output_path = env::temp_dir().join(format!(
        "codex-plus-region-screenshot-{captured_at_ms}-{}.png",
        std::process::id()
    ));
    let _ = fs::remove_file(&output_path);

    let launcher = bundled_flameshot_launcher().ok_or_else(|| {
        ScreenshotCaptureError::Failed(anyhow!(
            "安装包缺少内置 Flameshot，请重新安装 Codex++ 或重新构建安装包"
        ))
    })?;
    match run_flameshot_gui(&launcher, &output_path, options) {
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum FlameshotLauncher {
    Executable(PathBuf),
    #[cfg(target_os = "macos")]
    MacApp(PathBuf),
}

#[derive(Debug)]
enum FlameshotRunError {
    Cancelled(String),
    Unavailable(String),
}

fn run_flameshot_gui(
    launcher: &FlameshotLauncher,
    output_path: &Path,
    options: FlameshotOptions,
) -> Result<Vec<u8>, FlameshotRunError> {
    match launcher {
        FlameshotLauncher::Executable(command_path) => {
            run_flameshot_executable(command_path, output_path, options)
        }
        #[cfg(target_os = "macos")]
        FlameshotLauncher::MacApp(app_bundle) => {
            run_flameshot_macos_app(app_bundle, output_path, options)
        }
    }
}

fn run_flameshot_executable(
    command_path: &Path,
    output_path: &Path,
    options: FlameshotOptions,
) -> Result<Vec<u8>, FlameshotRunError> {
    let _ = fs::remove_file(output_path);
    let mut command = Command::new(command_path);
    command.arg("gui");
    append_flameshot_gui_args(&mut command, output_path, options);
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

    Err(FlameshotRunError::Unavailable(format!(
        "{} 返回 {}{}",
        command_path.display(),
        output.status,
        command_output_details(&output.stdout, &output.stderr)
    )))
}

#[cfg(target_os = "macos")]
fn run_flameshot_macos_app(
    app_bundle: &Path,
    output_path: &Path,
    options: FlameshotOptions,
) -> Result<Vec<u8>, FlameshotRunError> {
    let _ = fs::remove_file(output_path);
    let mut command = Command::new("/usr/bin/open");
    command
        .arg("-W")
        .arg("-n")
        .arg(app_bundle)
        .arg("--args")
        .arg("gui");
    append_flameshot_gui_args(&mut command, output_path, options);
    let mut child = command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            FlameshotRunError::Unavailable(format!(
                "{}: {}。{}",
                app_bundle.display(),
                error,
                macos_screen_recording_hint()
            ))
        })?;

    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        if output_path.exists() {
            if let Some(bytes) = read_nonempty_file_after_flush(output_path) {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(output_path);
                return Ok(bytes);
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if output_path.exists() {
                    if let Some(bytes) = read_nonempty_file_after_flush(output_path) {
                        let _ = fs::remove_file(output_path);
                        return Ok(bytes);
                    }
                }
                if status.success() {
                    return Err(FlameshotRunError::Cancelled(
                        "已取消区域截图或未保存截图".to_string(),
                    ));
                }
                return Err(FlameshotRunError::Unavailable(format!(
                    "{} 返回 {}。{}",
                    app_bundle.display(),
                    status,
                    macos_screen_recording_hint()
                )));
            }
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                return Err(FlameshotRunError::Unavailable(format!(
                    "{} 等待退出失败：{}。{}",
                    app_bundle.display(),
                    error,
                    macos_screen_recording_hint()
                )));
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(FlameshotRunError::Unavailable(format!(
                "等待内置 Flameshot 保存截图超时。{}",
                macos_screen_recording_hint()
            )));
        }
        thread::sleep(Duration::from_millis(120));
    }
}

fn append_flameshot_gui_args(command: &mut Command, output_path: &Path, options: FlameshotOptions) {
    if options.delay_ms > 0 {
        command.arg("--delay").arg(options.delay_ms.to_string());
    }
    if options.accept_on_select {
        command.arg("--accept-on-select");
    }
    command.arg("--path").arg(output_path);
}

fn flameshot_options_from_payload(payload: &Value) -> FlameshotOptions {
    let delay_ms = payload
        .get("delayMs")
        .or_else(|| payload.get("delay_ms"))
        .and_then(Value::as_u64)
        .unwrap_or_else(default_flameshot_delay_ms)
        .min(3_000);
    let accept_on_select = payload
        .get("acceptOnSelect")
        .or_else(|| payload.get("accept_on_select"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    FlameshotOptions {
        delay_ms,
        accept_on_select,
    }
}

#[cfg(target_os = "macos")]
fn default_flameshot_delay_ms() -> u64 {
    900
}

#[cfg(not(target_os = "macos"))]
fn default_flameshot_delay_ms() -> u64 {
    150
}

fn command_output_details(stdout: &[u8], stderr: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    let details = [stderr, stdout]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("；");
    if details.is_empty() {
        String::new()
    } else {
        format!("：{details}")
    }
}

#[cfg(target_os = "macos")]
fn macos_screen_recording_hint() -> &'static str {
    "请在 macOS 系统设置 > 隐私与安全性 > 屏幕与系统音频录制 中允许 Flameshot，授权后退出并重新打开 Codex++"
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

fn bundled_flameshot_launcher() -> Option<FlameshotLauncher> {
    let exe = env::current_exe().ok()?;
    bundled_flameshot_candidates_for_exe(&exe)
        .into_iter()
        .find(|launcher| launcher.exists())
}

fn bundled_flameshot_candidates_for_exe(exe: &Path) -> Vec<FlameshotLauncher> {
    let mut candidates = Vec::new();
    if let Some(dir) = exe.parent() {
        #[cfg(windows)]
        {
            candidates.push(FlameshotLauncher::Executable(
                dir.join("tools")
                    .join("flameshot")
                    .join("flameshot-cli.exe"),
            ));
            candidates.push(FlameshotLauncher::Executable(
                dir.join("tools").join("flameshot").join("flameshot.exe"),
            ));
            candidates.push(FlameshotLauncher::Executable(
                dir.join("tools")
                    .join("flameshot")
                    .join("bin")
                    .join("flameshot-cli.exe"),
            ));
            candidates.push(FlameshotLauncher::Executable(
                dir.join("tools")
                    .join("flameshot")
                    .join("bin")
                    .join("flameshot.exe"),
            ));
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(contents_dir) = dir.parent() {
                candidates.push(FlameshotLauncher::MacApp(
                    contents_dir.join("Helpers").join("Flameshot.app"),
                ));
            }
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            candidates.push(FlameshotLauncher::Executable(
                dir.join("tools").join("flameshot").join("flameshot"),
            ));
        }
    }
    candidates
}

impl FlameshotLauncher {
    fn exists(&self) -> bool {
        match self {
            FlameshotLauncher::Executable(path) => path.is_file(),
            #[cfg(target_os = "macos")]
            FlameshotLauncher::MacApp(path) => path
                .join("Contents")
                .join("MacOS")
                .join("flameshot")
                .is_file(),
        }
    }
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

fn screenshot_jobs() -> &'static Mutex<HashMap<String, ScreenshotJob>> {
    SCREENSHOT_JOBS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn new_screenshot_job_id(now: u128) -> String {
    let sequence = NEXT_SCREENSHOT_JOB_ID.fetch_add(1, Ordering::Relaxed);
    format!("{now:x}-{sequence:x}")
}

fn finish_screenshot_job(job_id: &str, result: Value) {
    let now = now_ms();
    let mut jobs = screenshot_jobs()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if let Some(job) = jobs.get_mut(job_id) {
        job.updated_at_ms = now;
        job.state = ScreenshotJobState::Finished(result);
    }
    cleanup_screenshot_jobs_locked(&mut jobs, now);
}

fn cleanup_screenshot_jobs_locked(jobs: &mut HashMap<String, ScreenshotJob>, now: u128) {
    jobs.retain(|_, job| match job.state {
        ScreenshotJobState::Running => now.saturating_sub(job.started_at_ms) <= RUNNING_JOB_TTL_MS,
        ScreenshotJobState::Finished(_) => {
            now.saturating_sub(job.updated_at_ms) <= COMPLETED_JOB_TTL_MS
        }
    });
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
    fn screenshot_options_default_to_accept_on_select() {
        let options = flameshot_options_from_payload(&json!({}));
        assert!(options.accept_on_select);
        assert_eq!(options.delay_ms, default_flameshot_delay_ms());
    }

    #[test]
    fn screenshot_options_clamp_delay() {
        let options = flameshot_options_from_payload(&json!({
            "delayMs": 9_000,
            "acceptOnSelect": false
        }));
        assert!(!options.accept_on_select);
        assert_eq!(options.delay_ms, 3_000);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn bundled_flameshot_candidates_are_exe_relative() {
        let candidates = bundled_flameshot_candidates_for_exe(Path::new("/tmp/codex-plus-plus"));
        assert!(candidates.iter().all(|launcher| match launcher {
            FlameshotLauncher::Executable(path) => path.starts_with("/tmp/tools"),
        }));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn bundled_flameshot_candidates_are_helper_app_relative() {
        let candidates = bundled_flameshot_candidates_for_exe(Path::new(
            "/tmp/Codex++.app/Contents/MacOS/CodexPlusPlus",
        ));
        assert!(candidates.iter().all(|launcher| match launcher {
            FlameshotLauncher::Executable(path) =>
                path.starts_with("/tmp/Codex++.app/Contents/MacOS/tools"),
            FlameshotLauncher::MacApp(path) =>
                path.starts_with("/tmp/Codex++.app/Contents/Helpers"),
        }));
    }
}
