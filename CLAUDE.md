# VRC MIDI Transposer

Desktop app that forwards MIDI from an input device to an output device with live transposition, mirrors pressed keys to a VRChat avatar over OSC, and exposes its controls to Home Assistant over MQTT. Owner: Marcus Universe <contact@marcus-universe.de>, MIT, https://github.com/marcus-universe/vrc_midi_transposer.

## Layout

| Path | What |
|---|---|
| `crates/engine/` | All MIDI, OSC, MQTT, project and console logic. Pure Rust, no Tauri dependency. Also the headless (terminal) front end. |
| `src-tauri/` | Tauri 2 shell: argument handling, window, commands, the event pump to the webview. |
| `src/` | Svelte 5 + TypeScript UI. `src/lib/ipc.ts` is the typed contract with `src-tauri/src/gui.rs`. |
| `packaging/` | Linux desktop metadata (AppStream metainfo, copyright). |
| `.github/workflows/` | `ci.yml`, `build.yml` (reusable build matrix), `release.yml` (manual draft release). |
| `docs/` | User documentation. |

## Commands

Use `bun run <script>` or `npm run <script>`; both work. `bun.lock` is the committed lockfile.

| Script | Does |
|---|---|
| `dev` | Run the app with hot reload |
| `build` | Release build and installer for the current OS |
| `build:windows` / `build:linux` / `build:linux:arm64` / `build:macos` | Platform bundles (must run on that OS) |
| `check` | `svelte-check` |
| `test` | `cargo test --workspace` |
| `lint` | `cargo fmt --check` and `cargo clippy -D warnings` |

Headless: `cargo run -p vrc-midi-transposer -- --headless`. Never run `cargo clean`: `target/release/` holds hand-made archives of old releases.

## Architecture rules

- The engine is driven by **one supervisor thread**. Front ends, the OSC listener and the MQTT client only send `EngineCmd` values to it and read `EngineEvent` values back. Do not add shared mutable state that bypasses the supervisor.
- The MIDI hot path is: midir callback → channel → forwarder thread. The callback must only send on the channel. The forwarder owns the output connection and the held-note table; do not log or allocate per message outside debug mode.
- Every service (MIDI in, MIDI out, OSC listener, MQTT) must be stoppable and restartable on its own. Stopping must not block for more than about a second.
- Runtime values changed from the console, OSC or MQTT (transpose, debug, OSC sending) are session state. Only edits made in the UI change the project, and the project is written to disk only on Save.
- The webview receives state, not deltas: one coalesced frame per display frame at most, via a Tauri `Channel`. Never emit one IPC message per MIDI event.
- The MQTT password never goes to the webview and is stripped from exported projects.

## Compatibility promise

Existing avatars and Home Assistant setups must keep working. Do not rename or change the meaning of OSC paths, MQTT topics, Home Assistant `unique_id`s, discovery topics or console commands. The full contract is in the `transposer-protocols` skill; read it before touching `crates/engine/src/osc`, `mqtt` or `command.rs`.

## UI rules

- Dark only. Tokens and component rules are in the `transposer-ui` skill.
- Run `/modern-web-guidance` before writing HTML, CSS or client-side JS.
- **Browser support:** the UI runs in WebView2 (Chromium, evergreen), WKWebView (macOS) and WebKitGTK (Linux; the AppImage bundles the version from Ubuntu 22.04). Treat anything that is not Baseline Widely Available as progressive enhancement and include the fallback its guide requires. No polyfill larger than a few kB.
- No `backdrop-filter`, no infinite animations, no layout work per MIDI event.

## Conventions

- English for all code, comments, log lines and UI text.
- Rust: `cargo fmt`, clippy clean, `thiserror`-free plain error enums or `String` errors at the IPC boundary. Tests live next to the code.
- Versions are pinned (`rust-toolchain.toml`, exact versions in `package.json`). When bumping, change the workspace version in the root `Cargo.toml` and `package.json` together; `tauri.conf.json` has no version field on purpose.
- Releasing: see the `release` skill.
