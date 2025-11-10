use std::path::PathBuf;

use crate::{errors::*, rebuilder::Rebuilder};
use serde::{Deserialize, Serialize};
use tokio::{fs, io};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub rebuilderd_community: Vec<Rebuilder>,
}

impl Config {
    fn new() -> Self {
        Default::default()
    }

    fn path() -> Result<PathBuf> {
        let path = dirs::data_local_dir()
            .map(|path| path.join("repro-threshold").join("config.toml"))
            .context("Failed to determine config path")?;
        Ok(path)
    }

    // XXX: these are provisory, replace with more robust implementation later
    pub async fn load() -> Result<Self> {
        let path = Self::path()?;
        let config = match fs::read_to_string(&path).await {
            Ok(content) => toml::from_str(&content)
                .with_context(|| format!("Failed to parse config file: {path:?}"))?,
            Err(err) if err.kind() == io::ErrorKind::NotFound => Config::new(),
            Err(err) => {
                return Err(
                    Error::from(err).context(format!("Failed to read config file: {path:?}"))
                );
            }
        };
        Ok(config)
    }

    // XXX: these are provisory, replace with more robust implementation later
    pub fn save(&self) -> Result<()> {
        todo!()
    }
}
