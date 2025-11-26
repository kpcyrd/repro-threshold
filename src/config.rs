use crate::{
    errors::*,
    rebuilder::{Rebuilder, Selectable},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;
use tokio::{fs, io};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Rules {
    /// Number of rebuilder attestations required until we believe them
    #[serde(default)]
    pub required_threshold: usize,
    /// Blindly allow these packages, even if nobody could reproduce the binary
    #[serde(default)]
    pub blindly_trust: BTreeSet<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    /// Rules for attestation policy
    #[serde(default)]
    pub rules: Rules,
    /// Rebuilders selected as trusted by the user
    #[serde(
        default,
        rename = "trusted_rebuilder",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub trusted_rebuilders: Vec<Rebuilder>,
    /// Rebuilders added manually by the user
    #[serde(
        default,
        rename = "custom_rebuilder",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub custom_rebuilders: Vec<Rebuilder>,
    /// Cached list of rebuilders from rebuilderd-community
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cached_rebuilderd_community: Vec<Rebuilder>,
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
    pub async fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create config directory: {parent:?}"))?;
        }

        let contents = toml::to_string_pretty(self)?;
        fs::write(&path, contents)
            .await
            .with_context(|| format!("Failed to write config file: {path:?}"))?;

        Ok(())
    }

    fn rebuilders_by_precedence(&self) -> Vec<Selectable<&Rebuilder>> {
        let mut rebuilders = Vec::new();
        rebuilders.extend(self.trusted_rebuilders.iter().map(|r| Selectable {
            active: true,
            item: r,
        }));
        rebuilders.extend(self.custom_rebuilders.iter().map(|r| Selectable {
            active: false,
            item: r,
        }));
        rebuilders.extend(self.cached_rebuilderd_community.iter().map(|r| Selectable {
            active: false,
            item: r,
        }));
        rebuilders
    }

    pub fn rebuilder_by_url(&self, url: &str) -> Option<Selectable<&Rebuilder>> {
        self.rebuilders_by_precedence()
            .into_iter()
            .find(|r| r.item.url.as_str() == url)
    }

    pub fn resolve_rebuilder_view(&self) -> Vec<Selectable<Rebuilder>> {
        let mut deduplicate = HashSet::new();
        let mut rebuilders = Vec::new();

        for rebuilder in self.rebuilders_by_precedence() {
            if deduplicate.insert(rebuilder.item.url.as_str()) {
                rebuilders.push(rebuilder.into());
            }
        }

        rebuilders
    }
}
