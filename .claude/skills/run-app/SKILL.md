---
name: run-app
description: How to launch and drive VRC MIDI Transposer for development and verification - GUI dev mode, the UI alone in a browser with the mock engine, headless terminal mode, and the OSC test tools. Use when asked to run, start, try or screenshot the app, or to confirm a change works.
---

# Running VRC MIDI Transposer

Prerequisites: Rust (the version in `rust-toolchain.toml`), Node 22+ or bun, and on Linux the Tauri system packages plus `libasound2-dev`. Install JS dependencies once with `bun install` (or `npm install`).

Only one instance can run per data directory. If a start fails with "already running", close the other instance or pass a different `--data-dir`.

## GUI, with hot reload

```sh
bun run dev
```

Starts Vite and the Tauri window. Rust changes rebuild and restart the app; Svelte changes hot-reload.

## UI alone, in a browser

```sh
bun run dev:web
```

Serves the UI at http://localhost:1420 with the mock engine from `src/lib/mock.ts`. Use this for layout and styling work; the mock animates a few piano keys and answers every command. Anything that depends on real MIDI, OSC or MQTT needs the real app.

## Headless (terminal)

```sh
cargo run -p vrc-midi-transposer -- --headless --data-dir ./.tmp/data
```

Flags: `--project <name or id>`, `--import <config.json>` (import a v1 config as a project and use it), `--data-dir <dir>`, `--version`, `--help`.

Type `help` for the commands. `exit` quits. For scripted checks, pipe the commands:

```sh
printf 'help\n5\nmidi list\nexit\n' | cargo run -q -p vrc-midi-transposer -- --headless --data-dir ./.tmp/data
```

Use a throwaway `--data-dir` for tests so the user's real projects are not touched. Without the flag the app uses the real data directory, and opening a project that has MQTT enabled connects to the user's broker.

## OSC test tools

Two small examples in `crates/engine/examples`:

```sh
# Print every OSC message arriving on a port (stand-in for VRChat)
cargo run -p transposer-engine --example osc_receive -- 127.0.0.1:9000

# Send one message: target, path, then an optional value (int, float, true/false)
cargo run -p transposer-engine --example osc_send -- 127.0.0.1:9069 /transpose 3
cargo run -p transposer-engine --example osc_send -- 127.0.0.1:9069 /transposeUp 1
cargo run -p transposer-engine --example osc_send -- 127.0.0.1:9069 /debug true
```

## What to check after a change

- Engine: `cargo test --workspace`, then the headless smoke test above.
- UI: `bun run check`, then look at it in `bun run dev`.
- Anything touching OSC or MQTT: compare with the `transposer-protocols` skill.

Do not run `cargo clean`.
