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

## MIDI triggers for custom controls

A custom control can be set from your keyboard or MIDI controller. Set up the trigger in the **OSC** view under *Custom controls*, or press **Learn** and use the key, knob or button on your device. See [PROJECTS.md](PROJECTS.md#custom-controls) for the settings.

The trigger is matched against the **incoming** message, before transposing, so it always reacts to the key you actually press.

| Trigger | Switch control | Slider control |
|---|---|---|
| **Control Change** | On at 64 and above, off below 64 | Value 0 to 127 is spread over the control's range: `min + value / 127 × (max − min)`. Without a range that is 0 to 1. |
| **Note**, mode *Toggle* | Each key press switches it on or off | Each key press switches between minimum and maximum |
| **Note**, mode *Hold while pressed* | On while the key is held | Maximum while held, minimum when released |
| **Program Change** | Each program change switches it | Switches between minimum and maximum |

- **Channel:** a trigger with a channel only reacts on that channel (1 to 16). *Any channel* reacts on all of them.
- **Also send to MIDI output:** off by default. Then a matching message only sets the control: it is not sent to the MIDI output, not shown on the keyboard and not sent to VRChat as a note. Switch it on if the key should also play.
- A key that was already sounding when you set up the trigger still gets its Note Off, so no note gets stuck.
- A knob that sends the same value again does not send the control again.
- **MIDI Learn** takes the next Note On, Control Change or Program Change, with its channel, as the trigger. A learned trigger starts in *Toggle* mode with *Also send to MIDI output* off; change either afterwards if needed.

**Throttling:** every matching message sends its OSC value to VRChat right away. The Dashboard and the MQTT state topic follow at most every 50 ms, so a fast knob turn does not flood Home Assistant; the last value always arrives.

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
| App availability | none | | `midi_transposer/availability` (`online` / `offline`): whether this app is connected |

`<slug>` is the name of the custom control in lower case with spaces replaced by `_`: "Hue Shift" becomes `hue_shift`.

State values are published as `1` / `0` or as plain numbers.

### Home Assistant

With MQTT discovery enabled in Home Assistant (the default), a device called **VRC MIDI Transposer** appears automatically with a number for the transpose value, buttons for up and down, switches for OSC sending, the note source and debug, and one switch or number per custom control. See [HomeAssistant.md](HomeAssistant.md) for the broker setup.
