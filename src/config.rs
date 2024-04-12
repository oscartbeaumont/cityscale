use std::{
    io,
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    branches: Vec<Branch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    name: String,
}

type ConfigManagerInner = Arc<(PathBuf, RwLock<Config>)>;

#[derive(Debug, Clone)]
pub struct ConfigManager(ConfigManagerInner);

impl ConfigManager {
    pub fn new(path: PathBuf) -> io::Result<Self> {
        let config = if !path.exists() {
            let config = Config::default();
            let data = serde_json::to_string_pretty(&config).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to serialize default config: {err}"),
                )
            })?;
            let path = &path;
            std::fs::write(path, data).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("Failed to write default config to {path:?}: {err}"),
                )
            })?;

            config
        } else {
            let data = std::fs::read_to_string(&path).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to read config from {path:?}: {err}"),
                )
            })?;
            serde_json::from_str(&data).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to deserialize config from {path:?}: {err}"),
                )
            })?
        };

        Ok(Self(Arc::new((path, RwLock::new(config)))))
    }

    pub fn get(&self) -> RwLockReadGuard<Config> {
        self.0 .1.read().unwrap_or_else(PoisonError::into_inner)
    }

    #[must_use = "You must call 'EditLock::commit' to save changes to the config file"]
    pub fn edit(&self) -> EditLock<'_> {
        let lock = self.0 .1.write().unwrap_or_else(PoisonError::into_inner);
        EditLock((*lock).clone(), lock, self.0.clone())
    }
}

/// A mutable lock to the configuration file.
/// WARNING: The changes must be committed using [`EditLock::commit`](EditLock::commit)!
#[derive(Debug)]
pub struct EditLock<'a>(Config, RwLockWriteGuard<'a, Config>, ConfigManagerInner);

impl<'a> EditLock<'a> {
    /// Commit all changes made to this config to the file system.
    pub fn commit(mut self) -> io::Result<()> {
        let data = serde_json::to_string_pretty(&self.0).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to serialize config: {err}"),
            )
        })?;
        std::fs::write(&self.2 .0, data).map_err(|err| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("Failed to write config to {:?}: {err}", self.2 .0),
            )
        })?;
        *self.1 = self.0;
        Ok(())
    }
}

impl<'a> Deref for EditLock<'a> {
    type Target = Config;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> DerefMut for EditLock<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
