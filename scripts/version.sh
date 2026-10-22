#!/usr/bin/env bash
# Shows or sets the app version, kept in three places that must agree:
# Cargo.toml ([workspace.package] version), package.json and src-tauri/tauri.conf.json.
#
#   scripts/version.sh          prints the version and fails when the three disagree
#   scripts/version.sh 0.2.0    sets all three (then commit and tag v0.2.0)
set -euo pipefail
cd "$(dirname "$0")/.."

cargo_version() { sed -n '/^\[workspace.package\]/,/^\[/{s/^version = "\(.*\)"/\1/p}' Cargo.toml; }
node_version() { node -p "require('./package.json').version"; }
tauri_version() { node -p "require('./src-tauri/tauri.conf.json').version"; }

if [ $# -eq 0 ]; then
  c=$(cargo_version); n=$(node_version); t=$(tauri_version)
  if [ "$c" != "$n" ] || [ "$c" != "$t" ]; then
    echo "version mismatch: Cargo.toml=$c package.json=$n tauri.conf.json=$t" >&2
    exit 1
  fi
  echo "$c"
  exit 0
fi

new=$1
if ! [[ $new =~ ^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$ ]]; then
  echo "not a semantic version: $new" >&2
  exit 1
fi
sed -i.bak "/^\[workspace.package\]/,/^\[/s/^version = \".*\"/version = \"$new\"/" Cargo.toml && rm Cargo.toml.bak
node -e "
const fs = require('fs');
for (const file of ['package.json', 'src-tauri/tauri.conf.json']) {
  const json = JSON.parse(fs.readFileSync(file, 'utf8'));
  json.version = '$new';
  fs.writeFileSync(file, JSON.stringify(json, null, 2) + '\n');
}"
# Cargo.lock records the workspace crates' version too.
cargo update --workspace --offline >/dev/null 2>&1 || cargo update --workspace >/dev/null
echo "version set to $new in Cargo.toml, package.json, src-tauri/tauri.conf.json"
