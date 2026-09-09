use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

const LAST_BACKUP_PATH: &str = "/var/lib/draupnir/last-backup";
const THRESHOLD_DAYS_PATH: &str = "/var/lib/draupnir/threshold-days";
const DEFAULT_THRESHOLD_DAYS: u64 = 7;
const RECENT_SECS: u64 = 24 * 3600;

fn main() {
    let now = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => {
            eprintln!("system clock is before Unix epoch");
            return;
        }
    };

    match read_last_backup() {
        None => notify("No backup has ever been performed.", "critical"),
        Some(ts) => {
            let threshold_secs = read_threshold_days() * 86400;
            let elapsed = now.saturating_sub(ts);
            let remaining = threshold_secs.saturating_sub(elapsed);
            let days_elapsed = elapsed / 86400;
            let hours_elapsed = elapsed / 3600;
            let days_remaining = remaining / 86400;
            let days_overdue = elapsed.saturating_sub(threshold_secs) / 86400;

            if elapsed < RECENT_SECS {
                notify(
                    &format!(
                        "Backup completed {hours_elapsed} hour(s) ago. Next due in {days_remaining} day(s)."
                    ),
                    "low",
                );
            } else if elapsed > threshold_secs {
                notify(
                    &format!(
                        "Backup is {days_overdue} day(s) overdue (last: {days_elapsed} day(s) ago). Please plug in your backup disk."
                    ),
                    "critical",
                );
            } else {
                notify(
                    &format!(
                        "Last backup {days_elapsed} day(s) ago. Next due in {days_remaining} day(s)."
                    ),
                    "low",
                );
            }
        }
    }
}

fn read_threshold_days() -> u64 {
    std::fs::read_to_string(Path::new(THRESHOLD_DAYS_PATH))
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_THRESHOLD_DAYS)
}

fn read_last_backup() -> Option<u64> {
    let raw = std::fs::read_to_string(Path::new(LAST_BACKUP_PATH)).ok()?;
    raw.trim().parse::<u64>().ok()
}

fn notify(message: &str, urgency: &str) {
    match Command::new("/usr/bin/notify-send")
        .args(["--urgency", urgency, "Draupnir Backup", message])
        .status()
    {
        Ok(st) if st.success() => {}
        Ok(st) => eprintln!("notify-send exited with {st}"),
        Err(e) => eprintln!("failed to spawn notify-send: {e}"),
    }
}
