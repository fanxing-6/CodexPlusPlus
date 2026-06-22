fn main() {
    let mut windows = tauri_build::WindowsAttributes::new();
    let profile = std::env::var("PROFILE").unwrap_or_default();
    if profile == "release" || std::env::var_os("CODEX_PLUS_WINDOWS_ADMIN_MANIFEST").is_some() {
        windows = windows.app_manifest(include_str!("windows-app-manifest.xml"));
    }
    let attrs = tauri_build::Attributes::new().windows_attributes(windows);
    tauri_build::try_build(attrs).expect("failed to run Tauri build script");
}
