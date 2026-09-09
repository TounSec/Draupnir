default: help 

help:
  @echo "Usage:"
  @echo "   just build              Build release binaries"
  @echo "   just install user       Install, enable reminder for current user"
  @echo "   just install global     Install, enable reminder for all users"
  @echo "   just uninstall user     Uninstall, disable reminder for current user"
  @echo "   just uninstall global   Uninstall, disable reminder for all users"
  @echo "   just check              Run clippy -D warnings"

build: clean
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

  _run "draupnir binary -> sudo cp target/release/draupnir /usr/local/bin/"                           sudo cp target/release/draupnir /usr/local/bin/
  _run "draupnir-reminder binary -> sudo cp target/release/draupnir-reminder /usr/local/bin/"         sudo cp target/release/draupnir-reminder /usr/local/bin/
  _run "draupnir.service -> sudo cp contrib/draupnir.service /etc/systemd/system/"                    sudo cp contrib/draupnir.service /etc/systemd/system/
  _run "99-draupnir.rules -> sudo cp contrib/99-draupnir.rules /etc/udev/rules.d/"                    sudo cp contrib/99-draupnir.rules /etc/udev/rules.d/
  _run "draupnir-reminder.service -> sudo cp contrib/draupnir-reminder.service /etc/systemd/user/"    sudo cp contrib/draupnir-reminder.service /etc/systemd/user/
  _run "draupnir-reminder.timer -> sudo cp contrib/draupnir-reminder.timer /etc/systemd/user/"        sudo cp contrib/draupnir-reminder.timer /etc/systemd/user/
  _run "systemctl daemon-reload -> sudo systemctl daemon-reload"                                      sudo systemctl daemon-reload
  _run "udevadm reload -> sudo udevadm control --reload-rules"                                        sudo udevadm control --reload-rules
  if [ "{{scope}}" = "user" ]; then
    _run "enable reminder timer -> sudo systemctl --user enable --now draupnir-reminder.timer"              sudo systemctl --usal enable --now draupnir-reminder.timer
  else
    _run "enable reminder timer -> sudo systemctl --global enable draupnir-reminder.timer"              sudo systemctl --global enable draupnir-reminder.timer
  fi

  # sudo cp target/release/draupnir /usr/local/bin/
  # sudo cp target/release/draupnir-reminder /usr/local/bin
  # sudo cp contrib/draupnir.service /etc/systemd/system/
  # sudo cp contrib/99-draupnir.rules /etc/udev/rules.d/
  # sudo cp contrib/draupnir-reminder.service /etc/systemd/user/
  # sudo cp contrib/draupnir-reminder.timer /etc/systemd/user/
  # sudo systemctl daemon-reload
  # sudo udevadm control --reload-rules
  # sudo systemctl --global enable draupnir-reminder.timer

uninstall scope:
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

  _run "draupnir binaries -> sudo rm -f /usr/local/bin/draupnir /usr/local/bin/draupnir-reminder"             sudo rm -f /usr/local/bin/draupnir /usr/local/bin/draupnir-reminder
  _run "draupnir.service -> sudo rm -f /etc/systemd/system/draupnir.service"                                  sudo rm -f /etc/systemd/system/draupnir.service
  _run "99-draupnir.rules -> sudo rm -f /etc/udev/rules.d/99-draupnir.rules"                                  sudo rm -f /etc/udev/rules.d/99-draupnir.rules
  _run "draupnir-reminder.service -> sudo rm -f /etc/systemd/user/draupnir-reminder.service"                  sudo rm -f /etc/systemd/user/draupnir-reminder.service
  _run "draupnir-reminder.timer -> sudo rm -f /etc/systemd/user/draupnir-reminder.timer"                      sudo rm -f /etc/systemd/user/draupnir-reminder.timer
  if [ "{{scope}}" = "user" ]; then
    _run "disable reminder timer -> sudo systemctl --user disable --now draupnir-reminder.timer"                    sudo systemctl --user disable --now draupnir-reminder.timer
  else
    _run "disable reminder timer -> sudo systemctl --global disable draupnir-reminder.timer"                    sudo systemctl --global disable draupnir-reminder.timer
  fi
  _run "/var/lib/draupnir -> sudo rm -f /var/lib/draupnir/last-backup"                                        sudo rm -f /var/lib/draupnir/last-backup
  _run "/var/lib/draupnir (rmdir) -> sudo rmdir --ignore-fail-on-non-empty /var/lib/draupnir"                 sudo rmdir --ignore-fail-on-non-empty /var/lib/draupnir
  _run "systemctl daemon-reload -> sudo systemctl daemon-reload"                                              sudo systemctl daemon-reload
  _run "udevadm reload -> sudo udevadm control --reload-rules"                                                sudo udevadm control --reload-rules

  # sudo rm -f /usr/local/bin/draupnir /usr/local/bin/draupnir-reminder
  # sudo rm -f /etc/systemd/system/draupnir.service
  # sudo rm -f /etc/udev/rules.d/99-draupnir.rules
  # sudo rm -f /etc/systemd/user/draupnir-reminder.service
  # sudo rm -f /etc/systemd/user/draupnir-reminder.timer
  # sudo rm -f /var/lib/draupnir/last-backup
  # sudo rmdir --ignore-fail-on-non-empty /var/lib/draupnir
  # sudo systemctl daemon-reload
  # sudo udevadm control --reload-rules

check:
  cargo clippy --workspace --all-targets -- -D warnings

clean:
  cargo clean
