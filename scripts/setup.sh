#!/usr/bin/env bash
set -euo pipefail

echo "=== Draupnir Setup ==="
echo ""

read -rp "Disk UUID (lsblk -o NAME,UUID,FSTYPE): " uuid
if [[ ! "$uuid" =~ ^[0-9a-f-]{36}$ ]]; then
  echo "Error: '$uuid' does not look like a valid UUID"
  exit 1
fi

read -rp "Mountpoint [/mnt/draupnir]: " mountpoint
mountpoint="${mountpoint:-/mnt/draupnir}"
read -rp "Destination [${mountpoint}/backups]: " destination
destination="${destination:-${mountpoint}/backups}"

echo "Sources (one absolute path per line, empty line to finish):"
sources=()
while true; do
  read -rp "  + " src
  [ -z "$src" ] && break
  if [[ "$src" != /* ]]; then
    echo "  Error: '$src' is not an absolute path, skipping"
    continue
  fi
  sources+=("\"$src\"")
done

if [ ${#sources[@]} -eq 0 ]; then
  echo "Error: at least one source is required"
  exit 1
fi

joined=$(printf '%s, ' "${sources[@]}")
sources_toml="[${joined%, }]"

echo "Tip: generate a key pair with: just age_keypair"
read -rp "Age public key (age1...): " age_pubkey
if [[ "$age_pubkey" != age1* ]]; then
  echo "Error: age public key must start with 'age1'"
  exit 1
fi

read -rp "Backup threshold in days [7]: " threshold_days
threshold_days="${threshold_days:-7}"
read -rp "Number of archives to keep [5]: " keep
keep="${keep:-5}"
read -rp "Log level [info]: " log_level
log_level="${log_level:-info}"
read -rp "draupnir binary path [/usr/local/bin/draupnir]: " execstart
execstart="${execstart:-/usr/local/bin/draupnir}"
read -rp "draupnir-reminder binary path [/usr/local/bin/draupnir-reminder]: " execstart_reminder
execstart_reminder="${execstart_reminder:-/usr/local/bin/draupnir-reminder}"
read -rp "Reminder check frequency [24h]: " timer_freq
timer_freq="${timer_freq:-24h}"
read -rp "Delay after boot [5min]: " boot_sec
boot_sec="${boot_sec:-5min}"
read -rp "Directories to exclude (space-separated basenames, empty to skip): " exclude_input
read -rp "zstd compression level 1-22 [3]: " compression_level
compression_level="${compression_level:-3}"

mkdir -p contrib

if [ -n "$exclude_input" ]; then
  exclude_entries=()
  for entry in $exclude_input; do
    exclude_entries+=("\"$entry\"")
  done
  exc_joined=$(printf '%s, ' "${exclude_entries[@]}")
  exclude_toml="[${exc_joined%, }]"
  exclude_line="exclude           = ${exclude_toml}"
else
  exclude_line="# exclude         = [\".cache\", \"node_modules\", \"target\"]"
fi

cat > contrib/config.toml << EOF
uuid              = "${uuid}"
mountpoint        = "${mountpoint}"
destination       = "${destination}"
age_pubkey        = "${age_pubkey}"
keep              = ${keep}
threshold_days    = ${threshold_days}
sources           = ${sources_toml}
compression_level = ${compression_level}
${exclude_line}
EOF

cat > contrib/99-draupnir.rules << EOF
ACTION=="add", SUBSYSTEM=="block", ENV{DEVTYPE}=="partition", \\
ENV{ID_FS_UUID}=="${uuid}", \\
ENV{UDISKS_AUTO}="0", \\
TAG+="systemd", ENV{SYSTEMD_WANTS}="draupnir.service"
EOF

cat > contrib/draupnir.service << EOF
[Unit]
Description=Draupnir encrypted backup daemon
After=local-fs.target
ConditionPathExists=/etc/draupnir/config.toml

[Service]
Type=simple
Restart=on-failure
RestartSec=5
ExecStart=${execstart}
User=root
UMask=0022
Environment=RUST_LOG=${log_level}
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

cat > contrib/draupnir-reminder.service << EOF
[Unit]
Description=Draupnir backup reminder
After=graphical-session.target

[Service]
Type=oneshot
ExecStart=${execstart_reminder}
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=default.target
EOF

cat > contrib/draupnir-reminder.timer << EOF
[Unit]
Description=Run draupnir-reminder periodically

[Timer]
OnBootSec=${boot_sec}
OnUnitActiveSec=${timer_freq}

[Install]
WantedBy=timers.target
EOF

echo ""
echo "Generated files:"
for f in contrib/config.toml contrib/99-draupnir.rules contrib/draupnir.service contrib/draupnir-reminder.service contrib/draupnir-reminder.timer; do
  echo "  v $f"
done
echo ""
echo "Next step: just install [user|global]"
