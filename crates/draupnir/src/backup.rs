use std::fs::{self, File};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, anyhow};
use log::warn;

use crate::config::Config;

struct PartGuard(Option<PathBuf>);

impl PartGuard {
    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for PartGuard {
    fn drop(&mut self) {
        if let Some(ref p) = self.0 {
            let _ = fs::remove_file(p);
        }
    }
}

pub fn run_backup(config: &Config) -> Result<()> {
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let name = format!("backup-{ts:020}.tar.zst.age");
    let part_path = config.destination.join(format!("{name}.part"));
    let final_path = config.destination.join(&name);

    let recipient: age::x25519::Recipient = config
        .age_pubkey
        .parse()
        .map_err(|e| anyhow!("{e}"))
        .context("parsing age recipient public key")?;

    let file = File::create(&part_path).context("creating .part file")?;
    let mut guard = PartGuard(Some(part_path.clone()));

    let encryptor =
        age::Encryptor::with_recipients(std::iter::once(&recipient as &dyn age::Recipient))
            .context("creating age encryptor")?;
    let age_w = encryptor.wrap_output(file).context("writing age header")?;
    let zstd_w =
        zstd::stream::write::Encoder::new(age_w, 3).context("initialising zstd encoder")?;

    let mut tar_b = tar::Builder::new(zstd_w);
    tar_b.follow_symlinks(false);

    for source in &config.sources {
        walk_into(&mut tar_b, source, source)
            .with_context(|| format!("archiving {}", source.display()))?;
    }

    // End in the order : tar -> zstd -> age
    let zstd_w = tar_b.into_inner().context("finalising tar")?;
    let age_w = zstd_w.finish().context("finalising zstd")?;
    let file = age_w.finish().context("finalising age encryption")?;

    file.sync_all().context("syncing archive to disk")?;
    fs::rename(&part_path, &final_path).context("renaming archive")?;
    guard.disarm();
    fsync_dir(&config.destination).context("fsyncing destination directory")?;

    // rotate only after commited new archive
    rotate(&config.destination, config.keep).context("rotating old archives")?;

    Ok(())
}

fn walk_into<W: std::io::Write>(
    builder: &mut tar::Builder<W>,
    root: &Path,
    dir: &Path,
) -> Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            warn!("skipping {}: permission denied", dir.display());
            return Ok(());
        }
        Err(e) => return Err(e).with_context(|| format!("reading {}", dir.display())),
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                warn!("skipping entry in {}: permission denied", dir.display());
                continue;
            }
            Err(e) => return Err(e).with_context(|| format!("reading entry in {}", dir.display())),
        };

        let path = entry.path();
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                warn!("skipping {}: permission denied", path.display());
                continue;
            }
            Err(e) => return Err(e).with_context(|| format!("stat {}", path.display())),
        };

        let rel = path
            .strip_prefix(root)
            .expect("walk always descends from root");

        if meta.is_dir() {
            builder
                .append_dir(rel, &path)
                .with_context(|| format!("adding dir {} to archive", rel.display()))?;
            walk_into(builder, root, &path)?;
        } else if meta.is_file() {
            let mut f = match File::open(&path) {
                Ok(f) => f,
                Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                    warn!("skipping {}: permission denied", path.display());
                    continue;
                }
                Err(e) => return Err(e).with_context(|| format!("opening {}", path.display())),
            };
            builder
                .append_file(rel, &mut f)
                .with_context(|| format!("adding {} to archive", rel.display()))?;
        }
        // symlinks ignored (follow_symlinks(false) + symlink_metadata)
    }
    Ok(())
}

fn fsync_dir(dir: &Path) -> std::io::Result<()> {
    File::open(dir)?.sync_all()
}

fn rotate(dest: &Path, keep: usize) -> Result<()> {
    let mut archives: Vec<PathBuf> = fs::read_dir(dest)
        .context("reading destination directory")?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("backup-") && n.ends_with(".tar.zst.age"))
                .unwrap_or(false)
        })
        .collect();

    archives.sort();

    let to_delete = archives.len().saturating_sub(keep);
    for path in archives.iter().take(to_delete) {
        fs::remove_file(path)
            .with_context(|| format!("deleting old archive {}", path.display()))?;
    }

    Ok(())
}
