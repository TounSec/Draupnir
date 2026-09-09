use std::fs::{self, File};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, anyhow};
use log::{info, warn};

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
    info!("[ 1/6 ] creating archive {name}");
    let part_path = config.destination.join(format!("{name}.part"));
    let final_path = config.destination.join(&name);

    let recipient: age::x25519::Recipient = config
        .age_pubkey
        .parse()
        .map_err(|e| anyhow!("{e}"))
        .context("parsing age recipient public key")?;

    fs::create_dir_all(&config.destination).with_context(|| {
        format!(
            "creating destination directory {}",
            config.destination.display()
        )
    })?;
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
        info!("[ 2/6 ] archiving {}", source.display());
        let meta =
            fs::symlink_metadata(source).with_context(|| format!("stat {}", source.display()))?;

        if meta.is_dir() {
            walk_into(&mut tar_b, source, source)
                .with_context(|| format!("archiving {}", source.display()))?;
        } else if meta.is_file() {
            let name = source
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("source has no filename: {}", source.display()))?;
            let mut f =
                File::open(source).with_context(|| format!("opening {}", source.display()))?;
            tar_b
                .append_file(name, &mut f)
                .with_context(|| format!("adding {} to archive", source.display()))?;
        } else {
            warn!("skipping {}: not a file or directory", source.display());
        }
    }

    // End in the order : tar -> zstd -> age
    info!("[ 3/6 ] finalising archive chain (tar -> zstd -> age)");
    let zstd_w = tar_b.into_inner().context("finalising tar")?;
    let age_w = zstd_w.finish().context("finalising zstd")?;
    let file = age_w.finish().context("finalising age encryption")?;

    info!("[ 4/6 ] syncing to disk");
    file.sync_all().context("syncing archive to disk")?;
    fs::rename(&part_path, &final_path).context("renaming archive")?;
    guard.disarm();
    fsync_dir(&config.destination).context("fsyncing destination directory")?;

    // rotate only after commited new archive
    info!("[ 5/6 ] rotating old archives (keep {})", config.keep);
    rotate(&config.destination, config.keep).context("rotating old archives")?;

    info!("[ 6/6 ] writing timestamp");
    write_last_backup(ts, config.threshold_days).context("writing last-backup timestamp")?;

    info!("done {name}");
    Ok(())
}

fn write_last_backup(ts: u64, threshold_days: u64) -> Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt;

    let dir = Path::new("/var/lib/draupnir");
    fs::create_dir_all(dir).context("creating /var/lib/draupnir")?;

    let part = dir.join("last-backup.part");
    let final_path = dir.join("last-backup");

    // last-backup
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o644)
        .open(&part)
        .context("creating last-backup.part")?;

    writeln!(file, "{ts}").context("writing timestamp")?;
    file.sync_all().context("syncing last-backup")?;
    fs::rename(&part, &final_path).context("renaming last-backup")?;

    // threshold_days
    let part = dir.join("threshold-days.part");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o644)
        .open(&part)
        .context("creating threshold-days.part")?;

    writeln!(file, "{threshold_days}").context("writing threshold")?;
    file.sync_all().context("syncing threshold-days")?;
    fs::rename(&part, dir.join("threshold-days")).context("renaming threshold-days")?;

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
