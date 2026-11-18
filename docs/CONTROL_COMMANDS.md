# Control Commands Reference

Everything that can control VRC MIDI Transposer besides the buttons in the app: the console, OSC messages and MQTT topics.

## Console commands

Type these in the console drawer at the bottom of the app (Ctrl+`), or in the terminal in headless mode. Commands are not case-sensitive.

| Command | Effect |
|---|---|
| `<number>` | Set the transpose value in semitones, for example `5`, `-3`, `0`. Clamped to the range set in the project. |
| `osc on` / `osc enable` | Enable sending notes to VRChat over OSC |
| `osc off` / `osc disable` | Disable it |
| `osc original` / `osc input` | Send the notes as played (original) |
| `osc transposed` / `osc output` | Send the transposed notes |
| `osc_original 1`, `osc_original:1`, `osc_original on` | Same as `osc original` (`0` / `off` for transposed) |
| `midi list` / `midi devices` / `list midi` | List the available MIDI inputs and outputs |
| `mqtt on` / `mqtt enable` | Start the MQTT client for this session |
| `mqtt off` / `mqtt disable` | Stop it |
| `debug on` / `debug enable` | Verbose logging |
| `debug off` / `debug disable` | Normal logging |
| `help` / `h` | List the commands |
| `exit` / `quit` / `q` | Quit |

In headless mode an empty line also quits. In the app's console an empty line does nothing.

Changes made from the console, OSC or MQTT apply to the running session. They are not written to the project.

## OSC messages (control)

Send to the listener, by default `127.0.0.1:9069`. The paths below are the defaults of every new project; you can change them and add more under **OSC → Listening addresses**. Only the first argument is read.

| Default path | Argument | Effect |
|---|---|---|
| `/transpose` | Int or Float (rounded) | Set the transpose value |
| `/transposeUp` | `1`, `1.0` or `true` | Transpose up by one. Other values are ignored, so a button that sends `0` on release works. |
| `/transposeDown` | `1`, `1.0` or `true` | Transpose down by one |
| `/debug` | Number or Bool | Debug logging on (non-zero / true) or off |
| `/midi/list` | none | Write the MIDI device lists to the console |
| `/osc/original` | none | Send original notes to VRChat |
| `/osc/transposed` | none | Send transposed notes to VRChat |

For what the app *sends* to VRChat, see [OSC_PARAMETERS.md](OSC_PARAMETERS.md).

## MQTT topics

`midi_transposer` is the default base topic; it can be changed in the project. Booleans accept `1`, `true` or `on` (any case) as on; anything else is off.

| Purpose | Command topic | Payload | State topic (retained) |
|---|---|---|---|
| Transpose | `midi_transposer/transpose` | Integer, e.g. `5` | `midi_transposer/state/transpose` |
| Transpose up | `midi_transposer/transposeUp` | `1` | (updates the transpose state) |
| Transpose down | `midi_transposer/transposeDown` | `1` | (updates the transpose state) |
| OSC sending | `midi_transposer/osc/sendingEnabled` | `1` / `0` | `midi_transposer/state/osc/sendingEnabled` |
| Send original notes | `midi_transposer/osc/sendOriginal` | `1` (original) / `0` (transposed) | `midi_transposer/state/osc/sendOriginal` |
| Debug | `midi_transposer/debug/enabled` | `1` / `0` | `midi_transposer/state/debug/enabled` |
| Custom control | `midi_transposer/osc/custom/<slug>/set` | `1` / `0` for a toggle, a number for a slider | `midi_transposer/state/osc/custom/<slug>` |
| Availability | none | | `midi_transposer/availability` (`online` / `offline`) |

`<slug>` is the name of the custom control in lower case with spaces replaced by `_`: "Hue Shift" becomes `hue_shift`.

State values are published as `1` / `0` or as plain numbers.

### Home Assistant

With MQTT discovery enabled in Home Assistant (the default), a device called **VRC MIDI Transposer** appears automatically with a number for the transpose value, buttons for up and down, switches for OSC sending, the note source and debug, and one switch or number per custom control. See [HomeAssistant.md](HomeAssistant.md) for the broker setup.
