use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use log::{error, info};
use nix::mount::{MntFlags, MsFlags, mount, umount2};

pub struct Mounted {
    mountpoint: Option<PathBuf>,
}

impl Mounted {
    /// Unmount explicitly so the caller can detect and report failure.
    /// Drop provides a silent safety net if this is never called.
    pub fn unmount(mut self) -> Result<()> {
        if let Some(mp) = self.mountpoint.take() {
            umount2(&mp, MntFlags::empty())
                .with_context(|| format!("unmounting {}", mp.display()))?;
            info!("unmounted {}", mp.display());
        }
        Ok(())
    }
}

impl Drop for Mounted {
    fn drop(&mut self) {
        if let Some(ref mp) = self.mountpoint {
            if let Err(e) = umount2(mp, MntFlags::empty()) {
                error!(
                    "umount {} failed ({}), retrying with MNT_DETACH",
                    mp.display(),
                    e
                );
                if let Err(e) = umount2(mp, MntFlags::MNT_DETACH) {
                    error!("umount --detach {} also failed: {}", mp.display(), e);
                }
            } else {
                info!("unmounted {}", mp.display());
            }
        }
    }
}

pub fn mount_disk(device: &Path, mountpoint: &Path) -> Result<Mounted> {
    info!("mounting {} -> {}", device.display(), mountpoint.display());
    std::fs::create_dir_all(mountpoint)
        .with_context(|| format!("creating mountpoint {}", mountpoint.display()))?;
    let flags = MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC;
    mount(Some(device), mountpoint, Some("ext4"), flags, None::<&str>)
        .with_context(|| format!("mounting {} on {}", device.display(), mountpoint.display()))?;
    info!("disk mounted");
    Ok(Mounted {
        mountpoint: Some(mountpoint.to_owned()),
    })
}
