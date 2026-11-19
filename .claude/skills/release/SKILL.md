---
name: release
description: Release procedure for VRC MIDI Transposer - bumping the version, running the manual GitHub Actions release workflow that builds Windows, Linux (x86_64 and aarch64) and macOS packages into a draft release, checking the draft, and where package metadata (owner, e-mail, license, repo URL) is set. Use when preparing or debugging a release or changing bundling.
---

# Releasing VRC MIDI Transposer

A release is a **draft** GitHub release created by `.github/workflows/release.yml`. The owner reviews and publishes it by hand. Do not publish, tag or merge unless asked to.

## 1. Bump the version

The version lives in three places; change all of them in one commit:

- `Cargo.toml` → `[workspace.package] version`
- `package.json` → `version`
- `packaging/linux/de.marcus-universe.vrc-midi-transposer.metainfo.xml` → add a `<release version="…" date="YYYY-MM-DD">` entry at the top of `<releases>`

The release workflow fails if the first two differ and warns if the metainfo has no entry for the version.

`src-tauri/tauri.conf.json` has no version field; Tauri reads it from Cargo. Then run `cargo check --workspace` so `Cargo.lock` picks it up, and `bun install` if dependencies changed.

Tags are `v<version>`, for example `v2.0.0`.

## 2. Make sure `main` is green

The `CI` workflow on the pull request to `main` runs the full build matrix, the package install tests and a dry run of the release assembly. Do not start a release from a commit where that is red.

## 3. Run the workflow

GitHub → Actions → **Release** → Run workflow (branch `main`), or:

```sh
gh workflow run release.yml --ref main
gh run watch
```

What it does:

1. Reads the version from `Cargo.toml` and fails if the tag `v<version>` already exists.
2. Calls `build.yml`: Windows x64, Linux x86_64, Linux aarch64, macOS universal.
3. Collects the artifacts, renames them to `vrc-midi-transposer_<version>_<os>_<arch>.<ext>`, writes `SHA256SUMS.txt`.
4. Creates the draft release with the files and the install notes.

## 4. Check the draft

- All expected files are attached: one `.exe`, one `.dmg`, and `.deb`, `.rpm`, `.AppImage` for each of `x86_64` and `aarch64`, plus `SHA256SUMS.txt`.
- Download one file and compare its hash with `SHA256SUMS.txt`.
- Install the Windows build and start it once.
- Edit the "What's changed" section, then publish.

If the run failed, no release was created. Fix, push, run again.

## Which package suits which system

| System | File |
|---|---|
| Windows 10/11 x64 | `…_windows_x64-setup.exe` |
| macOS (Intel and Apple Silicon) | `…_macos_universal.dmg` |
| Ubuntu, Debian, Mint | `…_linux_<arch>.deb` |
| Fedora, openSUSE | `…_linux_<arch>.rpm` |
| Arch, SteamOS (Steam Deck: x86_64, Steam Frame: aarch64), anything else | `…_linux_<arch>.AppImage` |

The builds are not signed with a paid certificate. Windows SmartScreen shows "More info → Run anyway"; macOS needs right-click → Open (or Privacy & Security → Open Anyway) on first start.

## Where the metadata comes from

| What | Set in | Ends up in |
|---|---|---|
| Owner and e-mail | `authors` in `[workspace.package]` | deb maintainer, `--version`, About panel. The rpm has no maintainer field (the bundler sets none); its owner and e-mail come from the metainfo file. |
| Publisher | `bundle.publisher` in `tauri.conf.json` | Windows exe "Company", installer publisher |
| Copyright line (owner, e-mail, MIT) | `bundle.copyright` | Windows exe properties, macOS `Info.plist` |
| License | `license` in `[workspace.package]`, `bundle.license`, `bundle.licenseFile` | rpm license, NSIS license page, bundled LICENSE |
| Repository URL | `repository`/`homepage` in `[workspace.package]`, `bundle.homepage` | deb/rpm homepage, installer links, About panel |
| AppStream data | `packaging/linux/*.metainfo.xml` | AppImage, deb, rpm (software centres) |

When any of these change, check one package of each format: file properties on the `.exe`, `dpkg-deb -I`, `rpm -qip`, and `appstreamcli validate --no-net` on the metainfo file.

`appstreamcli` always reports one warning, `cid-rdns-contains-hyphen`, because the component id equals the Tauri identifier and `marcus-universe` contains a hyphen. That one is expected and CI ignores it; any other warning or error is a real problem.

On Linux the product name is overridden to `vrc-midi-transposer` (`src-tauri/tauri.linux.conf.json`) so the desktop file, the command and the package files have no spaces. The display name comes from the desktop template in `packaging/linux/` and from the constant in `crates/engine/src/info.rs`; never show Tauri's `package_info().name` to the user.

## Changing the build

- Linux is built in an `ubuntu:22.04` container on purpose (old glibc, runs everywhere newer). The system package list is in `build.yml`.
- The deb depends on `libasound2t64 | libasound2 (>= 1.0.16)`, not plain `libasound2`. On Ubuntu 24.04 and later `libasound2` is only a virtual name, and apt may satisfy it with `liboss4-salsa-asound2`, which lacks the MIDI sequencer functions; the app then fails to start with an undefined `snd_seq_…` symbol. The versioned alternative only matches the real library.
- Check every version, action and runner label against its source before pinning it; do not rely on memory.
- A workflow change is done when it has run green on GitHub, not when it looks right.
