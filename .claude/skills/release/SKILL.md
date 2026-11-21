---
name: release
description: Release procedure for VRC MIDI Transposer - bumping the version, running the manual GitHub Actions release workflow that builds Windows, Linux (x86_64 and aarch64) and macOS packages into a draft release, checking the draft, the in-app updater (signing key, secrets, latest.json), the .vrcmt file association, and where package metadata (owner, e-mail, license, repo URL) is set. Use when preparing or debugging a release or changing bundling.
---

# Releasing VRC MIDI Transposer

A release is a **draft** GitHub release created by `.github/workflows/release.yml`. The owner reviews and publishes it by hand. Do not publish, tag or merge unless asked to.

## 1. Bump the version

The version lives in four places; change all of them in one commit:

- `Cargo.toml` → `[workspace.package] version`
- `package.json` → `version` (only that field)
- `packaging/linux/de.marcus-universe.vrc-midi-transposer.metainfo.xml` → add a `<release version="…" date="YYYY-MM-DD">` entry at the top of `<releases>`
- `Cargo.lock` → run `cargo check --workspace` after the edit above so it picks up the new version of both crates

The release workflow fails if the first two differ and warns if the metainfo has no entry for the version.

`src-tauri/tauri.conf.json` has no version field; Tauri reads it from Cargo. Run `bun install` if dependencies changed.

The version is also what the in-app updater compares: a release is offered to users when its `latest.json` `version` is greater (semver) than the running one. Never reuse or lower a version.

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
2. Calls `build.yml` with `updater: true` and the signing secrets: Windows x64, Linux x86_64, Linux aarch64, macOS universal, each signed for the in-app updater (fails at once if `TAURI_SIGNING_PRIVATE_KEY` is missing).
3. Collects the artifacts, renames them to `vrc-midi-transposer_<version>_<os>_<arch>.<ext>`, checks every updater signature with `minisign -V` against `plugins.updater.pubkey` in `tauri.conf.json` (the Tauri CLI only warns when the secret is a different key, and installed apps would reject such an update), writes `latest.json` and `SHA256SUMS.txt` (which covers `latest.json` too).
4. Creates the draft release with the files and the install notes.

A dry run (`ci.yml` on pull requests to `main`) builds without `updater`, needs no secret, and still runs the `latest.json` step with dummy signatures to check it; that file is not shipped.

## 4. Check the draft

- All expected files are attached: one `.exe`, one `.dmg`, one `.app.tar.gz`, and `.deb`, `.rpm`, `.AppImage` for each of `x86_64` and `aarch64`, plus `latest.json` and `SHA256SUMS.txt` (11 files).
- Download one file and compare its hash with `SHA256SUMS.txt`.
- Install the Windows build and start it once.
- Edit the "What's changed" section, then publish.
- After publishing, an installed older version should offer the update (Settings or at startup). The endpoint is `releases/latest/download/latest.json`, so only the newest **published, non-pre-release** release counts; drafts and pre-releases are invisible to the updater.

If the run failed, no release was created. Fix, push, run again.

## In-app updater

The app updates itself with `tauri-plugin-updater` (2.14), driven from Rust in `src-tauri/src/gui.rs` (`check_update`, `install_update`, `open_release`; no JS plugin API). Configuration: `plugins.updater` in `src-tauri/tauri.conf.json` (public key, endpoint, `requireSignedVersion`, Windows `installMode: passive`).

| Installed as | `update_mode` | What happens |
|---|---|---|
| NSIS (Windows) | `auto` | Downloads the `…_windows_x64-setup.exe`, verifies it, shuts the engine down, runs the installer passively; the installer restarts the app. |
| macOS `.app` | `auto` | Downloads `…_macos_universal.app.tar.gz` (both `darwin-aarch64` and `darwin-x86_64` point to it), replaces the app, restarts. |
| AppImage | `auto` | Downloads the `.AppImage` of its arch, replaces the file, restarts. |
| deb, rpm | `notify` | Reads the `linux-<arch>-appimage` entry only to learn the newest version; links to the release page. Never installs. |
| `cargo run`, `bun run dev` | `none` | No automatic check. |

### Signing key and secrets

- Updates are signed with a minisign key made by `tauri signer generate`. The **private key** is `%USERPROFILE%\.tauri\vrc-midi-transposer.key` on the owner's machine (no password). The public key is in `tauri.conf.json`. Keep a backup of the private key somewhere safe (password manager or offline): if it is lost, no installed copy can ever be updated again; users would have to reinstall a build with a new public key by hand. Never commit it (`.gitignore` excludes `*.key`).
- Repository secrets (GitHub → Settings → Secrets and variables → Actions), set once by the owner:
  ```powershell
  gh secret set TAURI_SIGNING_PRIVATE_KEY < "$env:USERPROFILE\.tauri\vrc-midi-transposer.key"
  ```
  (in cmd: `gh secret set TAURI_SIGNING_PRIVATE_KEY < "%USERPROFILE%\.tauri\vrc-midi-transposer.key"`).
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` should be empty because the key has none. GitHub may refuse to store an empty secret; leaving it unset is equivalent, because an unset secret reads as an empty string and the workflow always exports the variable.
- `release.yml` passes both secrets explicitly to `build.yml` (reusable workflows do not inherit secrets). Only the build step of a real release sees them.
- Updater artifacts come from `src-tauri/tauri.updater.conf.json` (`bundle.createUpdaterArtifacts: true`), passed with `--config` only in signed builds. Do not move that setting into `tauri.conf.json`: every build without the key (CI, contributors) would fail.
- Local signed build (Windows), to check the key: in Git Bash
  `TAURI_SIGNING_PRIVATE_KEY="$(cat "$USERPROFILE/.tauri/vrc-midi-transposer.key")" TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" bun run build:windows --config src-tauri/tauri.updater.conf.json`
  → `target/x86_64-pc-windows-msvc/release/bundle/nsis/*-setup.exe.sig` next to the installer. Do not print the key.

### latest.json

Written by the last job of `release.yml` and attached to the release:

```json
{
  "version": "2.1.0",
  "notes": "See https://github.com/<repo>/releases/tag/v2.1.0 for what is new.",
  "pub_date": "2026-11-21T12:00:00Z",
  "platforms": {
    "windows-x86_64":         { "signature": "<text of the .sig>", "url": ".../download/v2.1.0/vrc-midi-transposer_2.1.0_windows_x64-setup.exe" },
    "darwin-aarch64":         { "signature": "…", "url": ".../vrc-midi-transposer_2.1.0_macos_universal.app.tar.gz" },
    "darwin-x86_64":          { "signature": "…", "url": ".../vrc-midi-transposer_2.1.0_macos_universal.app.tar.gz" },
    "linux-x86_64-appimage":  { "signature": "…", "url": ".../vrc-midi-transposer_2.1.0_linux_x86_64.AppImage" },
    "linux-aarch64-appimage": { "signature": "…", "url": ".../vrc-midi-transposer_2.1.0_linux_aarch64.AppImage" }
  }
}
```

Rules: the `signature` is the text content of the `.sig` file of exactly that asset (signed after the build, so renaming the file does not break it); there must **never** be a bare `linux-x86_64` or `linux-aarch64` key, because a deb or rpm install would fall back to it and replace itself with an AppImage. `requireSignedVersion` is on, so the signature must carry the version (the Tauri CLI writes it into the trusted comment) and must match `version`. The `.sig` files themselves are not attached to the release. The "What's changed" text is only on the release page; the app links there.

If an updater release is broken, publish a fixed higher version; deleting `latest.json` from the newest release makes every app report "up to date" (a 404 counts as no update).

## File association (.vrcmt)

`bundle.fileAssociations` in `tauri.conf.json` registers `.vrcmt` ("VRC MIDI Transposer Project", MIME `application/x-vrcmt`, role Editor, rank Owner):

- **Windows (NSIS):** the installer registers the extension and the open command; a double-click starts the app with the file as argument (or hands it to the running instance through the single-instance plugin).
- **macOS:** `Info.plist` gets `CFBundleDocumentTypes` and an exported type `de.marcus-universe.vrc-midi-transposer.project` (conforms to `public.json`, `public.data`); files arrive as `RunEvent::Opened`.
- **Linux:** the bundler only fills `{{mime_type}}` in the desktop template; our template `packaging/linux/vrc-midi-transposer.desktop` writes `Exec={{exec}} %F` and `MimeType={{mime_type}};` itself (the bundler has no variable for the file argument). The MIME type is defined by `packaging/linux/de.marcus-universe.vrc-midi-transposer.xml`, installed to `/usr/share/mime/packages/` by the deb, rpm and AppImage (`bundle.linux.*.files`); shared-mime-info's package triggers update the database. The CI package test checks both desktop lines and the XML file.

Changing the extension or the MIME type breaks double-click for existing users; do not do it.

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
