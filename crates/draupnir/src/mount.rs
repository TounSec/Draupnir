use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use log::{error, info};
use nix::mount::{MntFlags, MsFlags, mount, umount2};

pub struct Mounted {
    mountpoint: PathBuf,
}

impl Drop for Mounted {
    fn drop(&mut self) {
        if let Err(e) = umount2(&self.mountpoint, MntFlags::empty()) {
            error!(
                "umount {} failed ({}), retrying with MNT_DETACH",
                self.mountpoint.display(),
                e
            );
            if let Err(e) = umount2(&self.mountpoint, MntFlags::MNT_DETACH) {
                error!(
                    "umount --detach {} also failed: {}",
                    self.mountpoint.display(),
                    e
                );
            }
        } else {
            info!("unmounted {}", self.mountpoint.display());
        }
    }
}

pub fn mount_disk(device: &Path, mountpoint: &Path) -> Result<Mounted> {
    info!("mounting {} -> {}", device.display(), mountpoint.display());
    let flags = MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC;

    mount(Some(device), mountpoint, Some("ext4"), flags, None::<&str>)
        .with_context(|| format!("mounting {} on {}", device.display(), mountpoint.display()))?;

    info!("disk mounted");
    Ok(Mounted {
        mountpoint: mountpoint.to_owned(),
    })
}
