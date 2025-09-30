#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
cd "$repo_root"

current=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
if [ -z "$current" ]; then
  echo "Cannot read current version from Cargo.toml" >&2
  exit 1
fi

core=${current%%-*}
IFS='.' read -r major minor patch <<< "$core"
new_minor=$((minor + 1))
new_version="${major}.${new_minor}.0"

sed -i -E "s/^version = \".*\"/version = \"${new_version}\"/" Cargo.toml

git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
git add Cargo.toml
git commit -m "Bump minor version: ${current} -> ${new_version}"

tag="v${new_version}"
git tag -a "$tag" -m "Release $tag"

git push origin HEAD
git push origin "$tag"

echo "Bumped minor version to ${new_version} and pushed tag ${tag}"
