use std::{
    collections::HashMap,
    io,
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Cookie secret for the dashboard authentication.
    pub secret: String,
    /// The root password for the MySQL server.
    /// This is intended for Cityscale to talk with the DB but not be exposed to end-users.
    pub mysql_root_password: String,
    /// User's who are allowed to access the admin panel.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub admins: HashMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            secret: Alphanumeric.sample_string(&mut rand::thread_rng(), 64),
            mysql_root_password: Alphanumeric.sample_string(&mut rand::thread_rng(), 32),
            admins: HashMap::from([(
                "admin".to_string(),
                // "admin" argon2 hashed
                "$argon2id$v=19$m=16,t=2,p=1$Y2l0eXNjYWxl$P3dUCcax9b1yc+LUlDLdWw".to_string(),
            )]),
        }
    }
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
