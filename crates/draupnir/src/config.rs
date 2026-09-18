use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub uuid: String,
    pub mountpoint: PathBuf,
    pub destination: PathBuf,
    pub age_pubkey: String,
    pub keep: usize,
    pub sources: Vec<PathBuf>,
    pub threshold_days: u64,
    #[serde(default)]
    pub exclude: Vec<PathBuf>,
    #[serde(default = "default_compression_level")]
    pub compression_level: i32,
}

fn default_compression_level() -> i32 {
    3
}

impl Config {
    fn validate(&self) -> Result<()> {
        if self.sources.is_empty() {
            bail!("sources is empty — the backup would archive nothing and rotation would delete existing archives");
        }
        for s in &self.sources {
            if !s.is_absolute() {
                bail!("source {} is not an absolute path", s.display());
            }
        }
        if self.keep == 0 {
            bail!("keep = 0 would delete every archive including the one just written");
        }
        if !self.mountpoint.is_absolute() {
            bail!("mountpoint must be an absolute path");
        }
        if !self.destination.is_absolute() {
            bail!("destination must be an absolute path");
        }
        if !self.destination.starts_with(&self.mountpoint) {
            bail!(
                "destination {} is not under mountpoint {} — backups would land on the root filesystem",
                self.destination.display(),
                self.mountpoint.display()
            );
        }
        if self.threshold_days == 0 {
            bail!("threshold_days = 0 makes every backup permanently overdue");
        }
        if !(1..=22).contains(&self.compression_level) {
            bail!(
                "compression_level {} is out of range (1-22)",
                self.compression_level
            );
        }
        for ex in &self.exclude {
            if ex.components().count() != 1 {
                bail!(
                    "exclude entry '{}' must be a single path component (e.g. '.cache'), not a path",
                    ex.display()
                );
            }
        }
        self.age_pubkey
            .parse::<age::x25519::Recipient>()
            .map_err(|e| anyhow::anyhow!("{e}"))
            .context("parsing age_pubkey")?;
        Ok(())
    }
}

pub fn load(path: &Path) -> Result<Config> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading config file {}", path.display()))?;
    let cfg: Config = toml::from_str(&raw)
        .with_context(|| format!("parsing config file {}", path.display()))?;
    cfg.validate()
        .with_context(|| format!("validating config file {}", path.display()))?;
    Ok(cfg)
}
