# VRC_Midi_Transposer

<p align="center">
  <img src="./docs/assets/icon_wide.png" alt="VRC_Midi_Transposer Icon Wide" width="600"/>
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

## Quick Start

1. Download the latest release or compile from source
2. Place the `VRC-Midi-Transposer.exe` and `config.json` in the same folder
3. Configure your MIDI ports and control settings in `config.json` (see [CONFIG.md](docs/CONFIG.md) for details)
4. Run the executable: `VRC-Midi-Transposer.exe`

The application will automatically detect your MIDI devices and start listening for control commands.

## Dependencies & Installation Requirements

### For Running the Executable

- **Windows 10/11** (64-bit)
- **MIDI Interface** - Any MIDI interface or virtual MIDI ports
- **Network Access** - For OSC and MQTT functionality (optional)

### For Development/Compilation

- **Rust toolchain**
- **Visual Studio Build Tools** (for Windows)
- **Git** (for cloning the repository)

Required Rust crates (automatically handled by Cargo):

- `midir` (0.10.2) - MIDI input/output handling
- `rosc` (0.11.4) - OSC (Open Sound Control) protocol
- `rumqttc` (0.24.0) - MQTT client functionality
- `serde` + `serde_json` (1.0) - JSON configuration parsing
- `winres` (0.1.12) - Windows resource embedding

## Configuration

The application uses a single `config.json` file for all settings. For detailed configuration options including MIDI ports, OSC endpoints, MQTT credentials, and transpose limits, please refer to [CONFIG.md](docs/CONFIG.md).

For VRChat avatar integration see [VRChat Avatar Integration](docs/AVATAR_SETUP.md) and the complete OSC parameter reference, see [OSC_PARAMETERS.md](docs/OSC_PARAMETERS.md).

## How to Compile

### Prerequisites

1. Install Rust: https://rustup.rs/
2. Clone this repository:
   ```bash
   git clone https://github.com/marcus-universe/MidiTransposer.git
   cd MidiTransposer
   ```

### Build Commands

```bash
# Debug build (faster compilation, larger binary)
cargo build

# Release build (optimized, smaller binary)
cargo build --release

# Run directly from source
cargo run
```

The compiled executable will be located in:

- Debug: `target/debug/transposer2025.exe`
- Release: `target/release/transposer2025.exe`

## Control Methods

- **Console Input**: Type a number and press Enter to set absolute transpose value.

- **OSC enable/disable**: You can toggle OSC sending from the console using text commands:

  - `osc on`, `osc enable` — enable OSC sending
  - `osc off`, `osc disable` — disable OSC sending
    You may also use numeric shortcuts: `1` (enable) and `0` (disable) when entered alone.

- **OSC original/transposed**: Choose whether OSC should send the original input MIDI or the transposed MIDI:
  - `osc original` or `osc input` — send original MIDI via OSC
  - `osc transposed` or `osc output` — send transposed MIDI via OSC
    There are also shorthand forms: `osc_original 1` or `osc_original:1` to force "original", and `osc_original 0` to force "transposed".

These two OSC-related flags can also be configured at startup in `config.json` inside the `osc` section. Example:

```json
"osc": {
   "listening_addr": "127.0.0.1:9069",
   "transpose_path": "/transpose",
   "transpose_up_path": "/transposeUp",
   "transpose_down_path": "/transposeDown",
   "sending_addr": "127.0.0.1",
   "sending_port": 9000,
   "sending_enabled": false,
   "send_original": true
}
```

The values `sending_enabled` (boolean) and `send_original` (boolean) determine the program's initial OSC sending state at startup but can still be changed via the console during runtime.

- **OSC Messages**: Send float values to configured OSC paths
- **MQTT Commands**: Publish commands to configured MQTT topics
- **VRChat OSC**: Direct integration with VRChat's OSC system (see [OSC_PARAMETERS.md](docs/OSC_PARAMETERS.md) for full parameter list)

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
