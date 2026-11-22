# Projects and settings

Everything you set up is stored in a **project**. Since version 2.1 a project is a file with the extension `.vrcmt` that you save wherever you like, for example one per instrument or avatar. You can copy it to another computer, send it to a friend or keep it next to your avatar files.

## Working with projects

The project menu in the top bar offers:

| Action | Shortcut | What it does |
|---|---|---|
| **New project** | | Starts an untitled project with default settings and the seven default OSC listening addresses |
| **Open…** | Ctrl+O | Opens a project file (`.vrcmt`) or a `config.json` from version 1 |
| **Save** | Ctrl+S | Writes your changes to the project file. A project that has no file yet asks where to save it. |
| **Save as…** | Ctrl+Shift+S | Saves the project to a new file and continues with that file |
| **Recent** | | The last ten project files. Click one to open it, or remove it from the list. A file that was moved or deleted is marked *Not found*. |

On macOS the shortcuts use ⌘ instead of Ctrl.

**Revert** in the top bar throws away unsaved changes and goes back to the saved file.

Changes take effect immediately, so you can try them while playing, but they are only kept after **Save**. A dot next to the project name shows unsaved changes. Before you open another project, start a new one, quit or install an update, the app asks whether to save them.

The app opens the project file you used last. New projects are saved to `Documents/VRC MIDI Transposer` unless you choose another folder; the folder is shown under **Settings**.

### Double-click to open

The installers register the `.vrcmt` file type, so a double-click on a project file opens it in the app. If the app is already running, the file opens in the running window.

| Package | Double-click |
|---|---|
| Windows setup `.exe`, macOS `.dmg`, `.deb`, `.rpm` | Works after installing |
| `.AppImage` | An AppImage cannot register file types by itself. Integrate it with [AppImageLauncher](https://github.com/TheAssassin/AppImageLauncher) or [Gear Lever](https://flathub.org/apps/it.mijorus.gearlever), or open the file with **Open…** in the app. |

### The MQTT password

The MQTT password is **never written into the project file**. It is kept on this computer only, in the app's data folder, and the app fills it in whenever you open a project that uses the same broker and user name. It is never shown in the app after you enter it.

When you open a project file on another computer, the password is missing there. Enter it once in the **MQTT** view and save; that computer keeps it from then on.

## App settings and project settings

Some settings belong to your computer, others to the project. The **Settings** view shows both.

| App settings (this computer) | Project settings (saved in the file) |
|---|---|
| Language | Everything in the table below |
| Check for updates automatically at start | Project name |
| Recent projects, the last open project, window layout | |
| MQTT passwords | |

### Language

The app is available in English, German (Deutsch), Italian (Italiano), Japanese (日本語), Simplified Chinese (简体中文), French (Français), Spanish (Español), Korean (한국어), Polish (Polski) and Russian (Русский). On first start it follows the language of your system, or English if that language is not available. Change it under **Settings → App → Language**; the switch is immediate.

The language is an app setting, so a project file looks the same in every language. Console commands, OSC paths, MQTT topics and log lines stay in English.

## What a project contains

| Area | Settings |
|---|---|
| **MIDI** | Input device and output device |
| **Transpose** | Lowest and highest allowed value (default −24 to +24) |
| **OSC listener** | Host and port the app listens on (default `127.0.0.1:9069`) and the listening addresses |
| **OSC target** | Host and port of VRChat (default `127.0.0.1:9000`) |
| **Custom controls** | Your own avatar parameters: name, OSC address, toggle or slider, default, minimum, maximum, and an optional MIDI trigger |
| **MQTT** | Enabled, broker host and port, base topic, user name (the password stays on the computer) |
| **Arp / Sequencer** | Mode, clock source and internal tempo, arpeggiator settings, the step sequence (see [ARP_SEQUENCER.md](ARP_SEQUENCER.md)) |
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

A custom control sends a value to an avatar parameter of your choice, for example a switch for `/avatar/parameters/Glasses` or a slider for `/avatar/parameters/HUE`. Each control appears on the Dashboard and, with MQTT enabled, as a switch or number in Home Assistant.

A control can also be set from your keyboard or MIDI controller. Give it a **MIDI trigger** in the **OSC** view under *Custom controls*:

| Setting | Choices |
|---|---|
| **Trigger** | None, Control Change, Note or Program Change |
| **Channel** | 1 to 16, or any channel |
| **Number** | The CC, note or program number (0 to 127) |
| **Mode** (notes only) | *Toggle*: each key press switches the control. *Hold while pressed*: on while the key is held, off when you let go. |
| **Also send to MIDI output** | Off (default): the message only sets the control. On: it is also played and forwarded as usual. |

**Learn** fills in the trigger for you: press it, then press a key, move a knob or change the program on your device. The first Note On, Control Change or Program Change becomes the trigger, with its channel. Esc stops learning.

A control with a trigger shows a small badge on the Dashboard, for example *CC 74 · Ch 1*. How the values are mapped is described in [CONTROL_COMMANDS.md](CONTROL_COMMANDS.md#midi-triggers-for-custom-controls).

## Projects from version 2.0

Version 2.0 kept its projects inside the app's data folder. On the first start of 2.1 they are converted **once** into `.vrcmt` files in `Documents/VRC MIDI Transposer`, and they appear in the recent list. The project you used last opens as before. MQTT passwords are taken out of the files and kept on this computer.

The old folder is left untouched, so going back to 2.0 still works. Changes you make in 2.1 do not reach it.

Files exported from 2.0 open with **Open…** like any project file.

## Opening a version 1 `config.json`

Choose **Open…** in the project menu and select the file. MIDI device names, OSC settings, listening and sending addresses, the MQTT broker with its credentials and the transpose range are taken over. The result is an unsaved project: press **Save** to keep it as a `.vrcmt` file.

On the very first start, if there is no project yet and a `config.json` lies next to the program or in the folder it was started from, it is opened this way automatically.

Version 1 identified MIDI devices by a part of their name. A project made from a `config.json` keeps matching that way until you pick the device from the list once.

## The data folder

The app keeps its own settings in a per-user folder:

| System | Folder |
|---|---|
| Windows | `%APPDATA%\de.marcus-universe.vrc-midi-transposer` |
| macOS | `~/Library/Application Support/de.marcus-universe.vrc-midi-transposer` |
| Linux | `~/.local/share/de.marcus-universe.vrc-midi-transposer` |

| File | Holds |
|---|---|
| `settings.json` | Recent projects, the last open project, language and window layout |
| `secrets.json` | The MQTT passwords, by broker and user name. Readable by your user only on Linux and macOS. |
| `projects/` | The projects of version 2.0, if you had any. No longer used. |

The folder is also shown under **Settings** in the app. Your projects are not in it; they are wherever you saved them.

## Running without a window

```sh
vrc-midi-transposer --headless
```

runs the same engine in the terminal with the console commands from [CONTROL_COMMANDS.md](CONTROL_COMMANDS.md). It opens the project used last.

```sh
vrc-midi-transposer --headless "D:\Music\Live set.vrcmt"
vrc-midi-transposer --headless --project "Live set"
```

| Option | Meaning |
|---|---|
| `FILE` | Open this project file (also works without `--headless`; this is what a double-click does). A `config.json` opens as an unsaved import. |
| `--project <file or name>` | Open this project file, or the project with this name from the recent list or the projects folder. The id of a 2.0 project still works. |
| `--import <file>` | Open a `config.json` or project file without adding it to the recent list |
| `--data-dir <dir>` | Use another data folder. New and converted projects then go to its `Projects` subfolder instead of Documents. |
| `--version` | Show version, owner, license and repository |
| `--help` | Show all options |

Changes made in headless mode from the console, OSC or MQTT apply to the running session only; the project file is not changed.

Only one instance can run at a time, because two would compete for the OSC port, the MIDI devices and the MQTT connection.

On Windows the app is a graphical program, so `--headless` started from cmd or PowerShell opens its own console window.
