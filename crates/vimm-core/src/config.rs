//! Configuration file loading and format preference resolution.
//!
//! Loads `~/.config/vimm-downloader/config.toml` (or a custom path) and
//! resolves format preferences with the order: CLI flag > config > site default.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Configuration loaded from a TOML file.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    /// Per-system format preferences: `{ "GameCube": "rvz", "Wii": "ciso" }`.
    #[serde(default)]
    pub formats: HashMap<String, String>,
}

impl Config {
    /// Load configuration from the default path.
    ///
    /// Returns an empty config if the file doesn't exist or is malformed
    /// (soft failure, not a hard error).
    #[must_use]
    pub fn load() -> Self {
        let path = default_config_path();
        Self::load_from_path(&path)
    }

    /// Load configuration from a specific path.
    ///
    /// Returns an empty config if the file doesn't exist or is malformed.
    #[must_use]
    pub fn load_from_path(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(contents) => match toml::from_str(&contents) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("Warning: malformed config file at {}: {e}", path.display());
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    /// Resolve the format key for a given system.
    ///
    /// Priority: `cli_flag` > config per-system > site default (empty string).
    #[must_use]
    pub fn resolve_format<'a>(&'a self, system: &str, cli_flag: Option<&'a str>) -> &'a str {
        if let Some(flag) = cli_flag {
            return flag;
        }
        if let Some(config_fmt) = self.formats.get(system) {
            return config_fmt;
        }
        ""
    }

    /// Returns the default config file path.
    #[must_use]
    pub fn default_path() -> PathBuf {
        default_config_path()
    }
}

fn default_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("vimm-downloader")
        .join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_config(dir: &Path, contents: &str) -> PathBuf {
        let path = dir.join("config.toml");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        path
    }

    #[test]
    fn load_from_valid_toml() {
        let tmp = TempDir::new().unwrap();
        let path = write_config(
            tmp.path(),
            r#"
[formats]
GameCube = "rvz"
Wii = "ciso"
"#,
        );
        let config = Config::load_from_path(&path);
        assert_eq!(config.formats.get("GameCube"), Some(&"rvz".to_string()));
        assert_eq!(config.formats.get("Wii"), Some(&"ciso".to_string()));
    }

    #[test]
    fn load_from_missing_file_returns_default() {
        let path = PathBuf::from("/nonexistent/config.toml");
        let config = Config::load_from_path(&path);
        assert!(config.formats.is_empty());
    }

    #[test]
    fn load_from_malformed_toml_returns_default() {
        let tmp = TempDir::new().unwrap();
        let path = write_config(tmp.path(), "this is not toml {{{");
        let config = Config::load_from_path(&path);
        assert!(config.formats.is_empty());
    }

    #[test]
    fn resolve_format_cli_takes_priority() {
        let tmp = TempDir::new().unwrap();
        let path = write_config(
            tmp.path(),
            r#"
[formats]
GameCube = "rvz"
"#,
        );
        let config = Config::load_from_path(&path);
        assert_eq!(config.resolve_format("GameCube", Some("ciso")), "ciso");
    }

    #[test]
    fn resolve_format_config_fallback() {
        let tmp = TempDir::new().unwrap();
        let path = write_config(
            tmp.path(),
            r#"
[formats]
GameCube = "rvz"
"#,
        );
        let config = Config::load_from_path(&path);
        assert_eq!(config.resolve_format("GameCube", None), "rvz");
    }

    #[test]
    fn resolve_format_site_default() {
        let config = Config::default();
        assert_eq!(config.resolve_format("GameCube", None), "");
    }

    #[test]
    fn resolve_format_unknown_system() {
        let tmp = TempDir::new().unwrap();
        let path = write_config(
            tmp.path(),
            r#"
[formats]
GameCube = "rvz"
"#,
        );
        let config = Config::load_from_path(&path);
        assert_eq!(config.resolve_format("NES", None), "");
    }
}
