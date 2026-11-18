---
name: transposer-protocols
description: The external contract of VRC MIDI Transposer - OSC listening roles and argument types, OSC messages sent to VRChat, MQTT topics and payloads, Home Assistant discovery entities, the project file schema and the console commands. Load before changing crates/engine/src/osc, crates/engine/src/mqtt, command.rs or the project model, and when verifying behaviour.
---

# VRC MIDI Transposer protocol contract

Avatars, Home Assistant dashboards and scripts in the wild depend on everything below. Changing a path, topic, id, payload or command spelling is a breaking change and needs an explicit request from the owner.

## MIDI

- Every incoming message is forwarded to the output unchanged, except the note number of Note On (`0x9n`), Note Off (`0x8n`) and Polyphonic Aftertouch (`0xAn`).
- Transposed note = `clamp(note + transpose, 0, 127)`.
- Note Off and aftertouch use the output note that was sent at Note On, so changing the transpose value while a key is held never leaves a stuck note.
- Transpose is clamped to the project's `transpose.min..=transpose.max`.

## OSC in (listener)

UDP, default `127.0.0.1:9069`. Each listening address has a path and a role. Several paths may share one role. Only the first argument is read. Bundles are unpacked.

| Role | Default path | First argument | Effect |
|---|---|---|---|
| `transpose` | `/transpose` | Int, Long, Float or Double (rounded) | Set transpose (clamped) |
| `transpose_up` | `/transposeUp` | Int/Long `1`, Float/Double `1.0`, Bool `true` | +1. Any other value is ignored (button release) |
| `transpose_down` | `/transposeDown` | same as up | −1 |
| `debug_set` | `/debug` | Int/Long/Float/Double non-zero, or Bool | Debug on/off |
| `midi_list` | `/midi/list` | ignored | Log the MIDI port lists |
| `osc_send_original` | `/osc/original` | ignored | Send original (input) notes over OSC |
| `osc_send_transposed` | `/osc/transposed` | ignored | Send transposed (output) notes over OSC |

Every new project starts with exactly these seven entries. An entry without a role is allowed and does nothing.

## OSC out (to VRChat)

UDP, default target `127.0.0.1:9000`. Sent only while OSC sending is enabled. The source is the input notes ("original") or the output notes ("transposed").

| MIDI | OSC address | Argument |
|---|---|---|
| Note On, velocity > 0 | `/avatar/parameters/<Name><Octave>` | Int `1` |
| Note Off, or Note On with velocity 0 | same | Int `0` |
| Pitch bend above centre | `/avatar/parameters/PitchUp` | Float 0.1..1.0, one decimal |
| Pitch bend below centre | `/avatar/parameters/PitchDown` | Float 0.1..1.0, one decimal |

- Names: `C CSHARP D DSHARP E F FSHARP G GSHARP A ASHARP B`. Octave = `note / 12 - 1`, so note 60 is `C4`, 61 is `CSHARP4`, 22 is `ASHARP0`.
- Pitch bend at centre sends nothing (known limitation, kept for compatibility).
- When the source or the enabled flag changes while keys are held, the held keys are released (Int `0`) first.

### Custom controls ("sending addresses")

Each entry: `name`, `addr`, `type` (`bool` or `float`), `default`, optional `min`/`max`. Setting a value sends one message to `addr`: Bool as Int `0`/`1`, Float as Float clamped to min/max. Custom controls are sent whether or not note sending is enabled.

## MQTT

Client id `vrc_midi_transposer`. Plain TCP. `<b>` is the project's base topic (default `midi_transposer`). All state publishes are QoS 1 and retained.

| Purpose | Command topic (subscribed) | State topic (published) |
|---|---|---|
| Transpose | `<b>/transpose` | `<b>/state/transpose` |
| Up / Down | `<b>/transposeUp`, `<b>/transposeDown` | updates `<b>/state/transpose` |
| OSC sending | `<b>/osc/sendingEnabled` | `<b>/state/osc/sendingEnabled` |
| OSC source original | `<b>/osc/sendOriginal` | `<b>/state/osc/sendOriginal` |
| Debug | `<b>/debug/enabled` | `<b>/state/debug/enabled` |
| Custom control | `<b>/osc/custom/<slug>/set` | `<b>/state/osc/custom/<slug>` |
| Availability | none | `<b>/availability`: `online` / `offline` (also the last will) |

- `slug` = control name lowercased with spaces replaced by `_`.
- Transpose payload: integer, or a float that is rounded.
- Boolean payload: `1`, `true` or `on` (any case) is true; anything else is false. For up/down a false payload is ignored.
- Booleans are published as `1` / `0`, numbers as plain text.
- Also subscribed: `homeassistant/+/midi_transposer/#` (stale entity cleanup) and `homeassistant/status` (Home Assistant online/offline).

### Home Assistant discovery

Published retained on every connect. Node id is always `midi_transposer`. Device: identifiers `["vrc_midi_transposer"]`, name `VRC MIDI Transposer`, manufacturer `Marcus Universe`, model `VRC MIDI Transposer`. Every entity has `availability_topic`.

| Entity | Discovery topic | `unique_id` | Notes |
|---|---|---|---|
| number "MIDI Transpose" | `homeassistant/number/midi_transposer/transpose/config` | `vrc_midi_transposer_transpose` | min/max from project, step 1, unit `semitones` |
| button "Transpose Up" | `homeassistant/button/midi_transposer/transpose_up/config` | `vrc_midi_transposer_transpose_up` | `payload_press` `1` |
| button "Transpose Down" | `homeassistant/button/midi_transposer/transpose_down/config` | `vrc_midi_transposer_transpose_down` | `payload_press` `1` |
| switch "OSC Sending Enabled" | `homeassistant/switch/midi_transposer/osc_sending_enabled/config` | `vrc_midi_transposer_osc_sending_enabled` | on `1`, off `0` |
| switch "OSC Send Original" | `homeassistant/switch/midi_transposer/osc_send_original/config` | `vrc_midi_transposer_osc_send_original` | on `1`, off `0` |
| switch "Debug Enabled" | `homeassistant/switch/midi_transposer/debug_enabled/config` | `vrc_midi_transposer_debug_enabled` | on `1`, off `0` |
| custom bool → switch | `homeassistant/switch/midi_transposer/custom_<slug>/config` | `vrc_midi_transposer_custom_<slug>` | on `1`, off `0` |
| custom float → number | `homeassistant/number/midi_transposer/custom_<slug>/config` | `vrc_midi_transposer_custom_<slug>` | step 0.01, min/max if set |

About 1.2 s after connecting, retained `custom_*` discovery topics that no longer match a control are cleared with an empty retained payload, which removes the entity in Home Assistant.

## Console commands

Shared by the GUI console and headless mode. Case-insensitive.

| Input | Effect |
|---|---|
| `<integer>` | Set transpose |
| `osc on` / `osc enable`, `osc off` / `osc disable` | OSC sending |
| `osc original` / `osc input` / `osc_original` | Send original notes |
| `osc transposed` / `osc output` / `osc_transposed` | Send transposed notes |
| `osc_original 1`, `osc_original:1`, `osc_original on` / `enable` (and `0`, `off`, `disable`) | Same as above, flag form |
| `midi list` / `midi devices` / `list midi` | Log the MIDI port lists |
| `mqtt on` / `mqtt enable`, `mqtt off` / `mqtt disable` | Start or stop the MQTT client for this session |
| `debug on` / `debug enable`, `debug off` / `debug disable` | Verbose logging |
| `help` / `h` | List commands |
| `exit` / `quit` / `q` | Quit |
| empty line | Headless: quit. GUI console: nothing |

End of input (a closed stdin) does not quit in headless mode.

## Project file

JSON, one file per project in `<data dir>/projects/<id>.json`. `schema` is `1`. Unknown fields are ignored and missing fields take defaults, so older files keep loading. Add fields with defaults; never repurpose one.

```json
{
  "schema": 1,
  "id": "…",
  "name": "My setup",
  "midi": { "input": { "id": "…", "name": "MRCC" }, "output": null },
  "osc": {
    "listen_host": "127.0.0.1", "listen_port": 9069,
    "target_host": "127.0.0.1", "target_port": 9000,
    "sending_enabled": false, "send_original": true,
    "listening_addresses": [ { "addr": "/transpose", "role": "transpose" } ],
    "sending_addresses": [ { "name": "Hue Shift", "addr": "/avatar/parameters/HUE", "type": "float", "default": 0.0, "min": 0.0, "max": 1.0 } ]
  },
  "mqtt": { "enabled": false, "broker_host": "127.0.0.1", "broker_port": 1883, "base_topic": "midi_transposer", "username": "", "password": "" },
  "transpose": { "min": -24, "max": 24 },
  "debug": false
}
```

- A MIDI port reference is resolved by `id`, then exact `name`, then `name` as a substring (this is how imported `config.json` files match).
- `osc.sending_enabled`, `osc.send_original` and `debug` are the values applied when the project is opened.
- The legacy `config.json` format (v1.x) is imported by `project::legacy`; keep that importer working.
