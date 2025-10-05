# Control Commands Reference

This document provides a comprehensive reference for all control methods available in the VRC MIDI Transposer.

## Console Commands

The application accepts commands through the console/terminal. Simply type a command and press Enter.

### Transpose Control

- `<number>` - Set absolute transpose value in semitones (e.g., `5`, `-3`, `12`)
- Range: -24 to +24 semitones

### OSC Control Commands

- `osc on` / `osc enable` - Enable OSC sending
- `osc off` / `osc disable` - Disable OSC sending
- `1` - Enable OSC sending (numeric shortcut)
- `0` - Disable OSC sending (numeric shortcut)

### OSC Data Source Commands

Choose whether OSC sends original or transposed MIDI data:

- `osc original` / `osc input` - Send original input MIDI via OSC
- `osc transposed` / `osc output` - Send transposed MIDI via OSC

### Advanced OSC Commands

- `osc_original 1` - Force sending original MIDI via OSC
- `osc_original 0` - Force sending transposed MIDI via OSC
- `osc_original:1` - Alternative syntax for forcing original MIDI
- `osc_original on` / `osc_original enable` - Enable original MIDI sending
- `osc_original off` / `osc_original disable` - Enable transposed MIDI sending

### MQTT Control Commands

- `mqtt on` / `mqtt enable` - Enable MQTT listener
- `mqtt off` / `mqtt disable` - Disable MQTT listener

### Debug Commands

- `debug on` / `debug enable` - Enable verbose debug logging
- `debug off` / `debug disable` - Disable verbose debug logging

### System Information Commands

- `midi list` / `midi devices` / `list midi` - Show available MIDI input/output devices

### Help and Exit Commands

- `help` / `h` - Show help with available commands
- `exit` / `quit` / `q` - Exit the program
- `<empty line>` - Exit the program (press Enter without typing)

## Numeric Shortcuts

For quick control, you can use these numeric shortcuts:

- `1` - Enable OSC sending (equivalent to `osc on`)
- `0` - Disable OSC sending (equivalent to `osc off`)
- Any integer between -24 and +24 - Set transpose value directly

## OSC Messages

Send OSC messages to the configured listening port (default: 9069) on the configured host (default: 127.0.0.1).

### Transpose Control Paths

- `/transpose <float>` - Set absolute transpose value (e.g., `/transpose 5.0`)
- `/transposeUp <float>` - Increase transpose by specified amount (e.g., `/transposeUp 1.0`)
- `/transposeDown <float>` - Decrease transpose by specified amount (e.g., `/transposeDown 1.0`)

### Configuration

OSC paths can be customized in `config.json`:

```json
"osc": {
  "listening_host": "127.0.0.1",
  "listening_port": 9069,
  "transpose_path": "/transpose",
  "transpose_up_path": "/transposeUp",
  "transpose_down_path": "/transposeDown"
}
```

## MQTT Commands

Send commands to the configured MQTT broker using the specified topics.

### Basic Transpose Control

- **Topic**: `midi_transposer/transpose/set`
- **Payload**: Integer value (-24 to +24)
- **Example**: `5` (transpose up 5 semitones)

### Relative Transpose Control

- **Topic**: `midi_transposer/transpose/up`
- **Payload**: `1` (increment)
- **Topic**: `midi_transposer/transpose/down`
- **Payload**: `1` (decrement)

### OSC Control via MQTT

- **Topic**: `midi_transposer/osc/sendingEnabled`
- **Payload**: `ON` or `OFF`

- **Topic**: `midi_transposer/osc/sendOriginal`
- **Payload**: `ON` (original MIDI) or `OFF` (transposed MIDI)

### Debug Control via MQTT

- **Topic**: `midi_transposer/debug/enabled`
- **Payload**: `ON` or `OFF`

### Custom OSC Controls

For custom OSC controls defined in config.json:

- **Topic**: `midi_transposer/osc/<control_name>`
- **Payload**: Depends on control type (boolean: `ON`/`OFF`, float: numeric value)

### State Topics (Read-Only)

The application publishes current state to these topics:

- `midi_transposer/state/transpose` - Current transpose value
- `midi_transposer/state/osc/sendingEnabled` - OSC sending state
- `midi_transposer/state/osc/sendOriginal` - OSC data source state
- `midi_transposer/state/debug/enabled` - Debug mode state

## VRChat OSC Integration

When integrated with VRChat, additional OSC parameters are available for avatar control. See [OSC_PARAMETERS.md](OSC_PARAMETERS.md) for the complete list of VRChat-specific parameters.

## Configuration Override

Many control settings can be pre-configured in `config.json` but can still be changed at runtime using the methods above:

```json
"osc": {
  "sending_enabled": false,
  "send_original": true
}
```

- `sending_enabled` (boolean) - Initial OSC sending state
- `send_original` (boolean) - Initial data source (true = original, false = transposed)

These settings define the startup behavior but can be modified during runtime using console commands, OSC messages, or MQTT commands.
