#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT

output="$temp_dir/icon.icns"

python3 "$repo_root/scripts/generate-macos-icon.py" "$repo_root/assets/icon.svg" "$output"

test -f "$output"
/usr/bin/file "$output" | /usr/bin/grep -F 'Mac OS X icon'

iconset="$temp_dir/icon.iconset"
/usr/bin/iconutil -c iconset -o "$iconset" "$output"
test -f "$iconset/icon_512x512@2x.png"
