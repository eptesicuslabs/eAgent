//! Configuration manager — load, save, and resolve config/data directories for eAgent.

use anyhow::{Context, Result};
use eagent_contracts::config::AgentConfig;
use std::path::{Path, PathBuf};
use tracing::info;

/// Configuration manager for eAgent.
pub struct ConfigManager {
    config_path: PathBuf,
    config: AgentConfig,
}

impl ConfigManager {
    /// Load configuration from the default location.
    pub fn load() -> Result<Self> {
        let config_dir = Self::config_dir()?;
        std::fs::create_dir_all(&config_dir)?;
        let config_path = config_dir.join("config.toml");

        let config = if config_path.exists() {
            let content =
                std::fs::read_to_string(&config_path).context("Failed to read config file")?;
            toml::from_str(&content).context("Failed to parse config file")?
        } else {
            let default = AgentConfig::default();
            let content =
                toml::to_string_pretty(&default).context("Failed to serialize default config")?;
            std::fs::write(&config_path, &content).context("Failed to write default config")?;
            info!(path = %config_path.display(), "Created default config file");
            default
        };

        Ok(Self {
            config_path,
            config,
        })
    }

    /// Load configuration from a specific path.
    pub fn load_from(path: PathBuf) -> Result<Self> {
        let config = if path.exists() {
            let content = std::fs::read_to_string(&path).context("Failed to read config file")?;
            toml::from_str(&content).context("Failed to parse config file")?
        } else {
            AgentConfig::default()
        };

        Ok(Self {
            config_path: path,
            config,
        })
    }

    /// Save the current configuration to disk.
    pub fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(&self.config).context("Failed to serialize config")?;
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.config_path, &content).context("Failed to write config file")?;
        info!(path = %self.config_path.display(), "Saved config");
        Ok(())
    }

    /// Get a reference to the current config.
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    /// Get a mutable reference to the current config.
    pub fn config_mut(&mut self) -> &mut AgentConfig {
        &mut self.config
    }

    /// Replace the entire configuration and save to disk.
    pub fn update(&mut self, config: AgentConfig) -> Result<()> {
        self.config = config;
        self.save()
    }

    /// Get the path to the config file.
    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }

    /// Get the config directory.
    ///
    /// Portable-first: uses a directory next to the executable if writable,
    /// otherwise falls back to the system config directory.
    pub fn config_dir() -> Result<PathBuf> {
        if let Some(root) = Self::portable_root() {
            let dir = root.join("config");
            std::fs::create_dir_all(&dir)?;
            return Ok(dir);
        }
        let dir = dirs::config_dir()
            .context("Failed to locate system config directory")?
            .join("eAgent");
        Ok(dir)
    }

    /// Get the data directory for eAgent.
    ///
    /// Portable-first: uses a directory next to the executable if writable,
    /// otherwise falls back to the system data directory.
    pub fn data_dir() -> Result<PathBuf> {
        if let Some(root) = Self::portable_root() {
            let dir = root.join("data");
            std::fs::create_dir_all(&dir)?;
            return Ok(dir);
        }
        let dir = dirs::data_local_dir()
            .context("Failed to locate system data directory")?
            .join("eAgent");
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Get the path to the event store database.
    pub fn event_store_path() -> Result<PathBuf> {
        Ok(Self::data_dir()?.join("events.db"))
    }

    /// Resolve the portable application root if the executable directory is writable.
    pub fn portable_root() -> Option<PathBuf> {
        if let Ok(root) = std::env::var("EAGENT_PORTABLE_ROOT") {
            let path = PathBuf::from(root);
            if std::fs::create_dir_all(&path).is_ok() {
                return Some(path);
            }
        }

        let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
        Self::portable_root_from_exe_dir(&exe_dir)
    }

    fn portable_root_from_exe_dir(exe_dir: &Path) -> Option<PathBuf> {
        let root = exe_dir.join("eAgent-data");
        if std::fs::create_dir_all(&root).is_err() {
            return None;
        }

        let probe = root.join(".write-test");
        match std::fs::write(&probe, b"portable") {
            Ok(()) => {
                let _ = std::fs::remove_file(probe);
                Some(root)
            }
            Err(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_default_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mgr = ConfigManager::load_from(path).unwrap();
        assert_eq!(mgr.config().general.theme, "dark");
        assert_eq!(mgr.config().general.font_size, 14.0);
    }

    #[test]
    fn test_save_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut mgr = ConfigManager::load_from(path.clone()).unwrap();
        mgr.config_mut().general.theme = "light".to_string();
        mgr.save().unwrap();

        let mgr2 = ConfigManager::load_from(path).unwrap();
        assert_eq!(mgr2.config().general.theme, "light");
    }

    #[test]
    fn test_update_replaces_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut mgr = ConfigManager::load_from(path.clone()).unwrap();
        let mut new_config = AgentConfig::default();
        new_config.general.font_size = 18.0;
        mgr.update(new_config).unwrap();

        let mgr2 = ConfigManager::load_from(path).unwrap();
        assert_eq!(mgr2.config().general.font_size, 18.0);
    }

    #[test]
    fn test_portable_root_from_exe_dir() {
        let dir = tempfile::tempdir().unwrap();
        let root = ConfigManager::portable_root_from_exe_dir(dir.path()).unwrap();
        assert_eq!(root, dir.path().join("eAgent-data"));
        assert!(root.exists());
    }

    #[test]
    fn test_portable_root_via_env() {
        let dir = tempfile::tempdir().unwrap();
        let root_path = dir.path().join("custom-root");
        // SAFETY: test is single-threaded; no other thread reads this env var concurrently.
        unsafe {
            std::env::set_var("EAGENT_PORTABLE_ROOT", root_path.to_str().unwrap());
        }

        let root = ConfigManager::portable_root();
        assert!(root.is_some());
        assert_eq!(root.unwrap(), root_path);

        // SAFETY: cleanup — same single-threaded test context.
        unsafe {
            std::env::remove_var("EAGENT_PORTABLE_ROOT");
        }
    }
}
