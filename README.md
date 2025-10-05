# VRC MIDI TRANSPOSER

<p align="center">
  <img src="./docs/assets/icon_wide.png" alt="VRC_Midi_Transposer Icon Wide" width="600"/>
  <br>
  <img src="https://img.shields.io/github/license/marcus-universe/vrc_midi_transposer.svg" />
  <img src="https://img.shields.io/github/stars/marcus-universe/vrc_midi_transposer?style=flat" />
  <img src="https://img.shields.io/badge/lang-rust-orange" />
</p>

<p align="center">
VRC_Midi_Transposer is a fast Rust application designed to transpose MIDI signals in real-time with remote control capabilities and to integrate with VRChat's OSC system, Home Assistant via MQTT for flexible MIDI Transposing and avatar piano control in VRC.
</p>

## Features

- 🎹 **Real-time MIDI Transposition** - Transpose incoming MIDI notes by semitones (-24 to +24)
- 🎮 **VRChat OSC Integration** - Control transposition directly from VRChat and control a piano avatar with OSC data [OSC_PARAMETERS.md](docs/OSC_PARAMETERS.md)
- 🏠 **MQTT Home Assistant Control** - Full integration with Home Assistant for easy remote control
- 🎵 **MIDI Routing** - Intelligent MIDI input/output port detection and routing
- 📡 **Multi-Protocol Support** - Supports OSC, MQTT, and direct keyboard input for control
- ⚡ **Low Latency** - Optimized for real-time performance with minimal delay
- 🔄 **Live Control** - Change transpose settings on-the-fly without stopping playback
- 🛠️ **Configurable** - Comprehensive JSON-based configuration system

<img src="./docs/assets/showcase.gif" alt="VRC_Midi_Transposer Screenshot" width="600"/>

## Quick Start

1. [Download the latest release](https://github.com/marcus-universe/vrc_midi_transposer/releases) or compile from source
2. Place the `VRC-Midi-Transposer.exe` and `config.json` in the same folder
3. Configure your MIDI ports and control settings in `config.json` (see [CONFIG.md](docs/CONFIG.md) for details)
4. Run the executable: `VRC-Midi-Transposer.exe`

The application will automatically detect your MIDI devices inside the `config.json` and start listening for control commands.

## Dependencies & Installation Requirements

### For Running the Executable

- **Windows 10/11** (64-bit)
- **MIDI Interface** - Any MIDI interface or virtual MIDI ports
- **Network Access** - For OSC and MQTT functionality (optional)
- **VRChat** - For VRChat OSC integration (optional)
- **Home Assistant** - For MQTT integration (optional)

### For Development/Compilation

- **Rust toolchain**
- **Visual Studio Build Tools** (for Windows)
- **Git** (for cloning the repository)

## Documentation

### Configuration & Setup

- **[Configuration Guide](docs/CONFIG.md)** - Complete `config.json` reference including MIDI ports, OSC endpoints, MQTT credentials, and transpose limits
- **[VRChat Avatar Integration](docs/AVATAR_SETUP.md)** - Setup guide for VRChat avatar piano control
- **[Build Guide](docs/BUILD_GUIDE.md)** - Step-by-step compilation instructions and troubleshooting

### Control & Usage

- **[Home Assistant Integration Guide](docs/HomeAssistant.md)** – Schritt-für-Schritt-Anleitung zur Einrichtung von MQTT, Mosquitto und Home Assistant
- **[Control Commands Reference](docs/CONTROL_COMMANDS.md)** – Komplette Liste aller Konsolen-, OSC- und MQTT-Kommandos
- **[OSC Parameters Reference](docs/OSC_PARAMETERS.md)** – VRChat OSC Parameter Dokumentation

## Quick Control Reference

The application can be controlled through multiple methods:

- **Console Input**: Type numbers or commands directly into the console
- **OSC Messages**: Send messages to configured OSC paths
- **MQTT Commands**: Publish to configured MQTT topics for Home Assistant integration
- **VRChat OSC**: Direct integration with VRChat's OSC system

For the complete command reference including all available console commands, OSC paths, and MQTT topics, see [CONTROL_COMMANDS.md](docs/CONTROL_COMMANDS.md).

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Contributing & Support

Thank you for your interest in VRC_Midi_Transposer! 🎵

If this project has been helpful to you, please consider:

- ⭐ **Giving it a star** on GitHub to help others discover it
- 🐛 **Reporting bugs** or suggesting features through GitHub Issues
- 🔧 **Contributing code** - Pull requests are welcome!
- 📖 **Improving documentation** - Help make this project more accessible

Your support and contributions help make this project better for the entire community!
