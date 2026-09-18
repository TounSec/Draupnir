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
  echo "Expected: $KEY_FILE"
  exit 1
fi

destination=$(grep '^destination' "$CONFIG" | sed 's/.*= *"\(.*\)"/\1/')

if [ ! -d "$destination" ]; then
  echo "Error: $destination not found. Is the backup disk mounted?"
  exit 1
fi

mapfile -t archives < <(ls -1 "$destination"/backup-*.tar.zst.age 2>/dev/null | sort -r)

if [ ${#archives[@]} -eq 0 ]; then
  echo "No archives found in $destination"
  exit 1
fi

echo "Available archives:"
echo ""
for i in "${!archives[@]}"; do
  name=$(basename "${archives[$i]}")
  size=$(stat -c%s "${archives[$i]}" 2>/dev/null | numfmt --to=iec 2>/dev/null || echo "?")
  echo "  [$i] $name  ($size)"
done
echo ""

read -rp "Select archive to restore [0]: " choice
choice="${choice:-0}"

if ! [[ "$choice" =~ ^[0-9]+$ ]] || [ "$choice" -ge "${#archives[@]}" ]; then
  echo "Error: invalid choice"
  exit 1
fi

selected="${archives[$choice]}"
echo ""
echo "Selected: $(basename "$selected")"
echo ""

read -rp "Extract to directory [/tmp/draupnir-restore]: " extract_to
extract_to="${extract_to:-/tmp/draupnir-restore}"

mkdir -p "$extract_to"

echo ""
echo "Restoring to $extract_to ..."
age -d -i "$KEY_FILE" "$selected" | zstd -d | tar -x -C "$extract_to"

echo ""
echo "Done. Files restored to: $extract_to"
