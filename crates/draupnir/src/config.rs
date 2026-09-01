/*
* Config Example in .toml file :
* uuid          = "7f3c1a2e-9b44-4d51-8e0a-1c2d3e4f5a6b"
* mountpoint    = "/mnt/draupnir"
* sources       = ["/home", "/etc"]
* destination   = "/mnt/draupnir/backups"
* age_pubkey    = "age1..."
* keep          = 5
*/

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub uuid: String,
    pub mountpoint: PathBuf,
    pub sources: Vec<PathBuf>,
    pub destination: PathBuf,
    pub age_pubkey: String,
    pub keep: usize,
}

pub fn load(path: &Path) -> Result<Config> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading config file {}", path.display()))?;
    toml::from_str(&raw).with_context(|| format!("parsing config file {}", path.display()))
}
