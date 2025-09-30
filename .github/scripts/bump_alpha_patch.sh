#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

current=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
if [ -z "$current" ]; then
  echo "Cannot read current version from Cargo.toml" >&2
  exit 1
fi

# If current already has a prerelease like X.Y.Z-alpha.N, increment N
if [[ "$current" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)-alpha\.([0-9]+)$ ]]; then
  major=${BASH_REMATCH[1]}
  minor=${BASH_REMATCH[2]}
  patch=${BASH_REMATCH[3]}
  alpha=${BASH_REMATCH[4]}
  new_alpha=$((alpha + 1))
  new_version="${major}.${minor}.${patch}-alpha.${new_alpha}"
else
  # Otherwise append -alpha.1 to the core version (preserve core numbers)
  core=${current%%-*}
  new_version="${core}-alpha.1"
fi

sed -i -E "s/^version = \".*\"/version = \"${new_version}\"/" Cargo.toml

git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
git add Cargo.toml
git commit -m "Bump alpha patch version: ${current} -> ${new_version}"

tag="v${new_version}"
git tag -a "$tag" -m "Release $tag"

git push origin HEAD
git push origin "$tag"

echo "Bumped alpha patch to ${new_version} and pushed tag ${tag}"
