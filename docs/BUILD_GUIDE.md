# Build Guide

VRC MIDI Transposer is a [Tauri 2](https://tauri.app) app: a Rust engine with a Svelte user interface.

## Prerequisites

| Tool | Version |
|---|---|
| Rust | The version in `rust-toolchain.toml` (installed automatically by `rustup`) |
| bun **or** Node.js with npm | bun 1.2 or newer, or Node.js 22 or newer |
| Git | any |

Platform extras:

- **Windows:** Microsoft C++ Build Tools ("Desktop development with C++") and the WebView2 runtime (part of Windows 11).
- **macOS:** Xcode Command Line Tools (`xcode-select --install`).
- **Linux:** the Tauri system packages and the ALSA headers. On Ubuntu or Debian:

  ```sh
  sudo apt install build-essential curl wget file pkg-config patchelf \
    libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
    libasound2-dev
  ```

  For other distributions see the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) and add the ALSA development package (`alsa-lib-devel` on Fedora, `alsa-lib` on Arch).

## Get the source

```sh
git clone https://github.com/marcus-universe/vrc_midi_transposer.git
cd vrc_midi_transposer
bun install        # or: npm install
```

## Run and build

Every script works with `bun run` and with `npm run`.

| Command | Result |
|---|---|
| `bun run dev` | Start the app with hot reload |
| `bun run build` | Release build and installer for the system you are on |
| `bun run build:windows` | Windows setup `.exe` (x64) |
| `bun run build:linux` | `.deb`, `.rpm` and `.AppImage` (x86_64) |
| `bun run build:linux:arm64` | The same for ARM64 (run on an ARM64 machine) |
| `bun run build:macos` | Universal `.dmg` for Intel and Apple Silicon |
| `bun run test` | Rust tests |
| `bun run check` | Type-check the user interface |
| `bun run lint` | Formatting and Clippy |

A platform build has to run on that platform; Tauri does not cross-compile between operating systems. The installers end up in `target/release/bundle/` (or `target/<target>/release/bundle/` when a target is given).

For the macOS universal build, add both targets first:

```sh
rustup target add x86_64-apple-darwin aarch64-apple-darwin
```

### Terminal mode

```sh
cargo run -p vrc-midi-transposer -- --headless
```

## Releases

Releases for all platforms are built by GitHub Actions:

1. Set the new version in `Cargo.toml` (`[workspace.package]`) and `package.json`, add a `<release>` entry to `packaging/linux/de.marcus-universe.vrc-midi-transposer.metainfo.xml`, run `cargo check --workspace` to update `Cargo.lock`, and merge to `main`.
2. On GitHub open **Actions → Release → Run workflow**.
3. The workflow builds Windows, Linux (x86_64 and ARM64) and macOS, signs the files for the in-app updater, writes `latest.json` and `SHA256SUMS.txt` and creates a **draft** release named `v<version>`.
4. Check the draft, edit the notes and publish it. Only then do installed apps see the update.

Every pull request to `main` runs the same builds without creating a release, and installs the Linux packages in Ubuntu, Debian, Fedora and Arch containers as a test.

### In-app updates

Installed copies look for new versions by themselves and can install them. This needs a signing key and two repository secrets.

- **Signing key:** made once with `tauri signer generate`. The public key is in `src-tauri/tauri.conf.json` (`plugins.updater`). The private key stays with the owner and is never committed (`*.key` is in `.gitignore`). Keep a backup: without it, installed copies can never be updated again and users would have to reinstall by hand.
- **Secrets** (GitHub → Settings → Secrets and variables → Actions):

  | Secret | Value |
  |---|---|
  | `TAURI_SIGNING_PRIVATE_KEY` | The content of the private key file |
  | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | The key's password; empty or unset if it has none |

  The release workflow fails at once if the key is missing. Pull request builds do not need the secrets.
- **`latest.json`:** the release workflow writes it and attaches it to the release. It holds the version, a link to the release notes, and the download URL and signature for each platform. Installed apps read `releases/latest/download/latest.json`.
- **Drafts do not count.** The app only sees the newest *published* release that is not a pre-release. A draft is invisible until you publish it.
- **Never reuse or lower a version.** An update is offered only when the release's version is higher than the installed one.

| Installed from | What the app does |
|---|---|
| Windows setup `.exe`, macOS `.dmg`, `.AppImage` | Downloads, checks the signature, installs and restarts |
| `.deb`, `.rpm` | Shows a notice with a link to the download page; the package manager stays in charge |
| `bun run dev`, `cargo run` | Nothing |

If a published update turns out broken, publish a fixed higher version.

## Troubleshooting

- **Linux: `alsa` or `webkit2gtk` not found while compiling:** a system package from the list above is missing.
- **"already running":** only one instance can use a data folder. Close the other one, or pass `--data-dir` with another folder.
- **Slow first build:** the first build compiles all dependencies; later builds are fast.
- Do not delete the `target` folder without need if you keep your own build results there.
