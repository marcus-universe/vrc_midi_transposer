# Projects and settings

Since version 2.0 there is no `config.json`. Everything you set up is stored in a **project**, and you can keep several projects, for example one per instrument or avatar.

## Working with projects

The project menu in the top bar lists your projects and offers:

| Action | What it does |
|---|---|
| **New** | Creates a project with default settings and the seven default OSC listening addresses |
| **Save** (Ctrl+S) | Writes your changes to the project |
| **Revert** | Throws away unsaved changes |
| **Duplicate** | Saves the current state as a new project and switches to it |
| **Import** | Reads a version 1 `config.json` or an exported project |
| **Export** | Writes the project to a file you can share. The MQTT password is not included. |
| **Delete** | Removes a project |

Changes take effect immediately, so you can try them while playing, but they are only kept after **Save**. A dot next to the project name shows unsaved changes. The app opens the project you used last.

## What a project contains

| Area | Settings |
|---|---|
| **MIDI** | Input device and output device |
| **Transpose** | Lowest and highest allowed value (default −24 to +24) |
| **OSC listener** | Host and port the app listens on (default `127.0.0.1:9069`) and the listening addresses |
| **OSC target** | Host and port of VRChat (default `127.0.0.1:9000`) |
| **Custom controls** | Your own avatar parameters: name, OSC address, toggle or slider, default, minimum, maximum |
| **MQTT** | Enabled, broker host and port, base topic, user name, password |
| **On start** | Whether OSC sending is on, whether original or transposed notes are sent, debug logging |

### Listening addresses

Each listening address is an OSC path plus the action it triggers. Every new project has these seven; you can change the paths, remove entries, or add further paths for the same action.

| Action | Default path |
|---|---|
| Set transpose | `/transpose` |
| Transpose up | `/transposeUp` |
| Transpose down | `/transposeDown` |
| Debug on/off | `/debug` |
| List MIDI devices | `/midi/list` |
| Send original notes | `/osc/original` |
| Send transposed notes | `/osc/transposed` |

*Restore defaults* adds back any of the seven that are missing. The argument each action expects is listed in [CONTROL_COMMANDS.md](CONTROL_COMMANDS.md).

### Custom controls

A custom control sends a value to an avatar parameter of your choice, for example a toggle for `/avatar/parameters/Glasses` or a slider for `/avatar/parameters/HUE`. Each control appears on the Dashboard and, with MQTT enabled, as a switch or number in Home Assistant.

## Importing a version 1 `config.json`

Choose **Import** in the project menu and select the file. MIDI device names, OSC settings, listening and sending addresses, the MQTT broker with its credentials and the transpose range are taken over.

On the very first start, if no project exists yet and a `config.json` lies next to the program, it is imported automatically.

Version 1 identified MIDI devices by a part of their name. An imported project keeps matching that way until you pick the device from the list once.

## Where projects are stored

| System | Folder |
|---|---|
| Windows | `%APPDATA%\de.marcus-universe.vrc-midi-transposer` |
| macOS | `~/Library/Application Support/de.marcus-universe.vrc-midi-transposer` |
| Linux | `~/.local/share/de.marcus-universe.vrc-midi-transposer` |

Each project is one JSON file in the `projects` subfolder; `settings.json` remembers the last project and the window layout. The folder is also shown under **Settings** in the app.

The MQTT password is stored in the project file on your computer in plain text, as it was in `config.json`. It is not shown in the app after you enter it and is left out of exported files.

## Running without a window

```sh
vrc-midi-transposer --headless
```

runs the same engine in the terminal with the console commands from [CONTROL_COMMANDS.md](CONTROL_COMMANDS.md). It opens the project used last.

| Option | Meaning |
|---|---|
| `--project <name>` | Open this project instead |
| `--import <file>` | Import a `config.json` or exported project and open it |
| `--data-dir <dir>` | Use another data folder |
| `--version` | Show version, owner, license and repository |

Only one instance can run at a time, because two would compete for the OSC port, the MIDI devices and the MQTT connection.

On Windows the app is a graphical program, so `--headless` started from cmd or PowerShell opens its own console window.
