mod backup;
mod config;
mod mount;

use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::os::fd::AsFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use anyhow::{Context, Result, bail};
use log::{error, info, warn};
use nix::errno::Errno;
use nix::fcntl::{Flock, FlockArg};
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use nix::unistd::geteuid;
use udev::{EventType, MonitorBuilder};

const LOCK_PATH: &str = "/run/draupnir.lock";
const CONFIG_PATH: &str = "/etc/draupnir/config.toml";

fn main() -> Result<()> {
    env_logger::Builder::from_default_env()
        .format_timestamp_secs()
        .format_module_path(false)
        .init();

    if !geteuid().is_root() {
        bail!("draupnir must run as root");
    }

    let cfg = config::load(Path::new(CONFIG_PATH)).context("loading configuration")?;

    // Named binding required: `let _ = ...` would drop the guard immediately
    let _lock = match acquire_lock()? {
        Some(l) => l,
        None => {
            warn!("another draupnir instance holds {LOCK_PATH}, exiting");
            return Ok(());
        }
    };

    let socket = MonitorBuilder::new()
        .context("creating udev monitor")?
        .match_subsystem_devtype("block", "partition")
        .context("filtering udev events on block/partition")?
        .listen()
        .context("enabling udev monitor")?;

    info!("waiting for disk with UUID {}", cfg.uuid);

    loop {
        let mut fds = [PollFd::new(socket.as_fd(), PollFlags::POLLIN)];
        match poll(&mut fds, PollTimeout::NONE) {
            Ok(_) => {}
            Err(Errno::EINTR) => continue,
            Err(e) => return Err(anyhow::Error::new(e).context("polling udev monitor socket")),
        }

        let revents = match fds[0].revents() {
            Some(r) => r,
            None => bail!("poll returned unknown event flags on udev socket"),
        };
        if revents.intersects(PollFlags::POLLERR | PollFlags::POLLNVAL | PollFlags::POLLHUP) {
            bail!("udev monitor socket error (revents {revents:?})");
        }
        if !revents.contains(PollFlags::POLLIN) {
            continue;
        }

        // Drain the socket fully before slow work to avoid netlink buffer overflow
        let matched: Vec<std::path::PathBuf> = socket
            .iter()
            .filter(|e| e.event_type() == EventType::Add)
            .filter(|e| e.property_value("ID_FS_UUID") == Some(OsStr::new(&cfg.uuid)))
            .filter_map(|e| match e.devnode() {
                Some(node) => Some(node.to_path_buf()),
                None => {
                    warn!("matching udev event has no devnode, ignoring");
                    None
                }
            })
            .collect();

        for devnode in matched {
            info!(
                "target disk detected at {}, starting backup",
                devnode.display()
            );
            if let Err(e) = handle_device(&devnode, &cfg) {
                error!("backup failed: {e:#}");
            }
        }
    }
}

fn handle_device(devnode: &Path, cfg: &config::Config) -> Result<()> {
    let mounted = mount::mount_disk(devnode, &cfg.mountpoint).context("mounting backup disk")?;
    backup::run_backup(cfg).context("running backup")?;
    info!("backup completed successfully, unmounting");
    // Explicit unmount so failure is detected and reported; Drop is the silent safety net
    mounted.unmount().context("unmounting backup disk")?;
    Ok(())
}

fn acquire_lock() -> Result<Option<Flock<std::fs::File>>> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(LOCK_PATH)
        .with_context(|| format!("opening lock file {LOCK_PATH}"))?;

    match Flock::lock(file, FlockArg::LockExclusiveNonblock) {
        Ok(lock) => Ok(Some(lock)),
        // EWOULDBLOCK aliases EAGAIN on Linux => never add a second EAGAIN arm
        Err((_file, Errno::EWOULDBLOCK)) => Ok(None),
        Err((_file, errno)) => {
            Err(anyhow::Error::new(errno).context(format!("acquiring flock on {LOCK_PATH}")))
        }
    }
}
