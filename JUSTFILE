default: help

help:
  @echo "Usage:"
  @echo "   just build              Build release binaries"
  @echo "   just age_keypair        Generate an age key pair for encryption"
  @echo "   just setup              Generate contrib/ config files interactively"
  @echo "   just install user       Install, enable reminder for current user"
  @echo "   just install global     Install, enable reminder for all users"
  @echo "   just uninstall user     Uninstall, disable reminder for current user"
  @echo "   just uninstall global   Uninstall, disable reminder for all users"
  @echo "   just verify             Verify latest archive integrity"
  @echo "   just restore            Restore from a backup archive"
  @echo "   just logs               Stream draupnir service logs"
  @echo "   just status             Show last backup status"
  @echo "   just check              Run clippy -D warnings"

build:
  cargo build --release

install scope: build
  #!/usr/bin/env bash
  set -e
  if [ "{{scope}}" != "user" ] && [ "{{scope}}" != "global" ]; then
    echo "Usage: just install [user|global]"; exit 1
  fi
  sudo -v
  _run() {
    local label="$1"; shift
    local output
    if output=$("$@" 2>&1); then
      echo -e "\e[32m✓\e[0m $label"
    else
      echo -e "\e[31m✗\e[0m $label"
      [ -n "$output" ] && echo "  $output"
      return 1
    fi
  }

  _run "draupnir binary"                  sudo install -m 0755 target/release/draupnir /usr/local/bin/
  _run "draupnir-reminder binary"         sudo install -m 0755 target/release/draupnir-reminder /usr/local/bin/
  _run "create /etc/draupnir"             sudo mkdir -p /etc/draupnir
  _run "install config.toml"              sudo install -m 0644 contrib/config.toml /etc/draupnir/config.toml
  _run "draupnir.service"                 sudo install -m 0644 contrib/draupnir.service /etc/systemd/system/
  _run "99-draupnir.rules"                sudo install -m 0644 contrib/99-draupnir.rules /etc/udev/rules.d/
  _run "draupnir-reminder.service"        sudo install -m 0644 contrib/draupnir-reminder.service /etc/systemd/user/
  _run "draupnir-reminder.timer"          sudo install -m 0644 contrib/draupnir-reminder.timer /etc/systemd/user/
  _run "systemctl daemon-reload"          sudo systemctl daemon-reload
  _run "udevadm reload-rules"             sudo udevadm control --reload-rules
  _run "enable draupnir.service"          sudo systemctl enable --now draupnir.service
  if [ "{{scope}}" = "user" ]; then
    _run "enable reminder timer"          systemctl --user enable --now draupnir-reminder.timer
  else
    _run "enable reminder timer (global)" sudo systemctl --global enable draupnir-reminder.timer
  fi

uninstall scope:
  #!/usr/bin/env bash
  set -e
  if [ "{{scope}}" != "user" ] && [ "{{scope}}" != "global" ]; then
    echo "Usage: just uninstall [user|global]"; exit 1
  fi
  sudo -v
  _run() {
    local label="$1"; shift
    local output
    if output=$("$@" 2>&1); then
      echo -e "\e[32m✓\e[0m $label"
    else
      echo -e "\e[31m✗\e[0m $label"
      [ -n "$output" ] && echo "  $output"
      return 1
    fi
  }

  _run "disable draupnir.service"            sudo systemctl disable --now draupnir.service || true
  if [ "{{scope}}" = "user" ]; then
    _run "disable reminder timer"            systemctl --user disable --now draupnir-reminder.timer || true
  else
    _run "disable reminder timer (global)"   sudo systemctl --global disable draupnir-reminder.timer || true
  fi
  _run "remove draupnir binaries"            sudo rm -f /usr/local/bin/draupnir /usr/local/bin/draupnir-reminder
  _run "remove /etc/draupnir"                sudo rm -rf /etc/draupnir
  _run "remove draupnir.service"             sudo rm -f /etc/systemd/system/draupnir.service
  _run "remove 99-draupnir.rules"            sudo rm -f /etc/udev/rules.d/99-draupnir.rules
  _run "remove draupnir-reminder.service"    sudo rm -f /etc/systemd/user/draupnir-reminder.service
  _run "remove draupnir-reminder.timer"      sudo rm -f /etc/systemd/user/draupnir-reminder.timer
  _run "remove /var/lib/draupnir state"      sudo rm -f /var/lib/draupnir/last-backup /var/lib/draupnir/threshold-days
  sudo rmdir --ignore-fail-on-non-empty /var/lib/draupnir 2>/dev/null || true
  _run "systemctl daemon-reload"             sudo systemctl daemon-reload
  _run "udevadm reload-rules"                sudo udevadm control --reload-rules
  echo ""
  echo "NOTE: The age private key (~/.config/draupnir/) was NOT removed."
  echo "      It is the only key that decrypts existing archives."
  echo "      Delete it manually once you no longer need access to any backup."

setup:
  chmod +x scripts/setup.sh
  bash scripts/setup.sh

age_keypair:
  #!/usr/bin/env bash
  set -euo pipefail

  if ! command -v age-keygen &>/dev/null; then
    echo "Error: age-keygen not found. Install age first (apt install age)."
    exit 1
  fi

  KEY_DIR="$HOME/.config/draupnir"
  KEY_FILE="$KEY_DIR/draupnir_age_key.txt"

  if [ -f "$KEY_FILE" ]; then
    echo "Key already exists at $KEY_FILE"
    pubkey=$(grep "^# public key:" "$KEY_FILE" | awk '{print $NF}')
    echo ""
    echo "Public key:"
    echo "  $pubkey"
    exit 0
  fi

  mkdir -p "$KEY_DIR"
  chmod 700 "$KEY_DIR"
  age-keygen -o "$KEY_FILE"
  chmod 600 "$KEY_FILE"

  pubkey=$(grep "^# public key:" "$KEY_FILE" | awk '{print $NF}')

  echo ""
  echo "Key saved to $KEY_FILE (private, never share this)"
  echo ""
  echo "Public key — copy this into /etc/draupnir/config.toml:"
  echo ""
  echo "  $pubkey"
  echo ""

logs:
  journalctl -u draupnir.service -n 50 -f

status:
  #!/usr/bin/env bash
  set -euo pipefail

  LAST="/var/lib/draupnir/last-backup"
  THR="/var/lib/draupnir/threshold-days"
  if [ ! -f "$LAST" ]; then
    echo "No backup has ever been performed."
    exit 0
  fi

  ts=$(cat "$LAST")
  threshold=$(cat "$THR" 2>/dev/null || echo 7)
  now=$(date +%s)
  elapsed=$(( now - ts ))
  days_elapsed=$(( elapsed / 86400 ))
  threshold_secs=$(( threshold * 86400 ))
  echo "Last backup : $(date -d "@$ts" '+%Y-%m-%d %H:%M:%S') (${days_elapsed} day(s) ago)"
  echo "Threshold   : ${threshold} day(s)"
  if [ "$elapsed" -gt "$threshold_secs" ]; then
    overdue=$(( (elapsed - threshold_secs) / 86400 ))
    echo "Status      : OVERDUE by ${overdue} day(s)"
  else
    days_remaining=$(( (threshold_secs - elapsed) / 86400 ))
    echo "Next due    : in ${days_remaining} day(s)"
    echo "Status      : OK"
  fi

verify:
  #!/usr/bin/env bash
  set -euo pipefail

  CONFIG="${DRAUPNIR_CONFIG:-/etc/draupnir/config.toml}"
  KEY_FILE="$HOME/.config/draupnir/draupnir_age_key.txt"
  if [ ! -f "$CONFIG" ]; then
    echo "Error: config not found at $CONFIG"
    echo "Set DRAUPNIR_CONFIG=contrib/config.toml to use the local copy."
    exit 1
  fi
  if [ ! -f "$KEY_FILE" ]; then
    echo "Error: age private key not found at $KEY_FILE"
    exit 1
  fi

  destination=$(grep '^destination' "$CONFIG" | sed 's/.*= *"\(.*\)"/\1/')
  if [ ! -d "$destination" ]; then
    echo "Error: $destination not found (disk not mounted?)"
    exit 1
  fi

  shopt -s nullglob
  archives=("$destination"/backup-*.tar.zst.age)
  if [ ${#archives[@]} -eq 0 ]; then
    echo "No archives found in $destination"
    exit 1
  fi

  latest=$(printf '%s\n' "${archives[@]}" | sort | tail -1)
  echo "Verifying: $(basename "$latest")"
  count=$(age -d -i "$KEY_FILE" "$latest" | zstd -d | tar -t | wc -l)
  echo "Archive intact: $count entries."

restore:
  chmod +x scripts/restore.sh
  bash scripts/restore.sh

check:
  cargo clippy --workspace --all-targets -- -D warnings

clean:
  cargo clean
