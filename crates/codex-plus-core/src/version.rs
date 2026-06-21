pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const VERSION: &str = match option_env!("CODEX_PLUS_RELEASE_VERSION") {
    Some(value) => value,
    None => PACKAGE_VERSION,
};

#[cfg(test)]
mod tests {
    use super::{PACKAGE_VERSION, VERSION};

    #[test]
    fn exposes_workspace_version() {
        assert_eq!(PACKAGE_VERSION, env!("CARGO_PKG_VERSION"));
        assert!(!VERSION.trim().is_empty());
    }
}
