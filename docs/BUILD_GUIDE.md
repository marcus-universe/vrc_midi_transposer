# Build Guide

This guide explains how to compile the VRC MIDI Transposer from source code.

> **Note**: If you just want to use the application, you can [download the pre-built executable](https://github.com/marcus-universe/vrc_midi_transposer/releases) instead of building from source.

## Prerequisites

Before building the application, make sure you have the following installed:

1. **Rust toolchain** - Install from: https://rustup.rs/
2. **Git** - For cloning the repository
3. **Visual Studio Build Tools** (Windows) - Required for native dependencies

## Getting the Source Code

Clone the repository and navigate to the project directory:

```bash
git clone https://github.com/marcus-universe/vrc_midi_transposer.git
cd vrc_midi_transposer
```

## Build Commands

### Debug Build

Faster compilation but larger executable size:

```bash
cargo build
```

### Release Build

Optimized build with smaller executable size (recommended for production):

```bash
cargo build --release
```

### Run from Source

Compile and run directly without creating an executable:

```bash
cargo run
```

## Output Locations

After compilation, you'll find the executable in:

- **Debug build**: `target/debug/transposer2025.exe`
- **Release build**: `target/release/transposer2025.exe`

## Build Troubleshooting

### Common Issues

**Missing Visual Studio Build Tools (Windows)**
If you encounter linking errors, install the Microsoft C++ Build Tools:

- Download from: https://visualstudio.microsoft.com/visual-cpp-build-tools/
- Select "C++ build tools" workload during installation

**Network Dependencies**
Some dependencies may require network access during the first build. Ensure your firewall allows cargo to download packages.

**Disk Space**
The target directory can become quite large during development. You can clean it with:

```bash
cargo clean
```

## Development Setup

For active development, you might want to:

1. **Enable debug logging** by setting `"debug": true` in your `config.json`
2. **Use cargo check** for faster syntax checking without compilation:
   ```bash
   cargo check
   ```
3. **Run tests** (if available):
   ```bash
   cargo test
   ```

## Cross-Platform Notes

While this guide focuses on Windows, the application can potentially be built for other platforms supported by Rust and the MIDI libraries. However, some features may be Windows-specific.
