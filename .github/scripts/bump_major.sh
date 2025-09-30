#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

# Read current version from Cargo.toml
current=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
if [ -z "$current" ]; then
  echo "Cannot read current version from Cargo.toml" >&2
  exit 1
fi

# Strip pre-release if present
core=${current%%-*}
IFS='.' read -r major minor patch <<< "$core"
new_major=$((major + 1))
new_version="${new_major}.0.0"

# Update Cargo.toml (version field stores plain semver without leading v)
sed -i -E "s/^version = \".*\"/version = \"${new_version}\"/" Cargo.toml

git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
git add Cargo.toml
git commit -m "Bump major version: ${current} -> ${new_version}"

tag="v${new_version}"
git tag -a "$tag" -m "Release $tag"

# Push commit and tag
git push origin HEAD
git push origin "$tag"

echo "Bumped major version to ${new_version} and pushed tag ${tag}"
