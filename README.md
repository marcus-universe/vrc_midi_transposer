# VRC MIDI TRANSPOSER

<p align="center">
  <img src="./docs/assets/icon_wide.png" alt="VRC_Midi_Transposer Icon Wide" width="600"/>
  <br>
  <img src="https://img.shields.io/github/license/marcus-universe/vrc_midi_transposer.svg" />
  <img src="https://img.shields.io/github/stars/marcus-universe/vrc_midi_transposer?style=flat" />
  <img src="https://img.shields.io/badge/lang-rust-orange" />
</p>

<p align="center">
VRC MIDI Transposer transposes MIDI in real time between two devices, mirrors the pressed keys to a VRChat avatar over OSC, and can be remote-controlled from VRChat, Home Assistant (MQTT) or its own console.
</p>

## Features

- 🎹 **Real-time MIDI transposition** with no stuck notes when the value changes while keys are held
- 🖥️ **Desktop app** for Windows, Linux (x86_64 and ARM64) and macOS with a dark, lightweight UI
- 🎛️ **Device selection** from a list, with automatic reconnect when a device is plugged in again
- 🎮 **VRChat OSC** in both directions: control transposition from VRChat and drive an avatar piano ([OSC_PARAMETERS.md](docs/OSC_PARAMETERS.md))
- 🏠 **Home Assistant over MQTT** with automatic discovery, including your own custom avatar controls
- 💾 **Projects** instead of a config file: save, load, duplicate, import and export complete setups
- ⌨️ **Built-in console** with all commands, plus a `--headless` mode that runs without a window

<img src="./docs/assets/showcase.gif" alt="VRC_Midi_Transposer Screenshot" width="600"/>

## Install

Download the file for your system from the [latest release](https://github.com/marcus-universe/vrc_midi_transposer/releases).

| System | File |
|---|---|
| Windows 10/11 (64-bit) | `…_windows_x64-setup.exe` |
| macOS (Intel and Apple Silicon) | `…_macos_universal.dmg` |
| Ubuntu, Debian, Mint | `…_linux_<arch>.deb` |
| Fedora, openSUSE | `…_linux_<arch>.rpm` |
| Arch, SteamOS (Steam Deck: `x86_64`, Steam Frame: `aarch64`), other distributions | `…_linux_<arch>.AppImage` |

The builds are not signed with a paid certificate:

- **Windows:** SmartScreen shows a warning on first start. Choose *More info*, then *Run anyway*.
- **macOS:** right-click the app and choose *Open*, or allow it under *System Settings → Privacy & Security*.
- **AppImage:** make the file executable (`chmod +x`), then start it.

The ARM64 Linux builds are experimental; they are built and install-tested automatically but not tried on every device.

## Quick start

1. Start the app. It opens a project called *Default*.
2. On the **Dashboard**, pick your MIDI input and output device.
3. Play. Change the transpose value with the buttons, the slider, the console, OSC or MQTT.
4. Optional: enable **OSC sending** to drive a VRChat avatar piano, and set up **MQTT** for Home Assistant.
5. Press **Save** (Ctrl+S) to keep the setup in the project.

**Coming from version 1.x?** Use *Import* in the project menu and select your old `config.json`. All settings, including listening and sending addresses and the MQTT broker, become a project. OSC paths, MQTT topics and Home Assistant entities are unchanged, so your avatar and dashboards keep working.

## Documentation

- **[Projects and settings](docs/PROJECTS.md)**: what a project contains, where it is stored, importing a `config.json`, headless mode
- **[Control commands](docs/CONTROL_COMMANDS.md)**: every console command, OSC path and MQTT topic
- **[OSC parameters](docs/OSC_PARAMETERS.md)**: what is sent to VRChat
- **[VRChat avatar setup](docs/AVATAR_SETUP.md)**: building an avatar piano
- **[Home Assistant](docs/HomeAssistant.md)**: MQTT broker and Home Assistant setup
- **[Build guide](docs/BUILD_GUIDE.md)**: building from source and making a release

## License

MIT, see [LICENSE](LICENSE). Made by Marcus Universe (<contact@marcus-universe.de>).

## Contributing & support

If this project is useful to you:

- ⭐ Give it a star so others can find it
- 🐛 Report bugs or suggest features in the GitHub issues
- 🔧 Pull requests are welcome
