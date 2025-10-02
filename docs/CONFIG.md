# MidiTransposer Configuration

## config.json

All configuration is now managed through a single `config.json` file in the project root. This replaces the previous hardcoded constants and the separate `mqtt_credentials.json` file.

### Structure

```json
{
  "midi": {
    "input_port_name_substr": "MRCC",
    "output_port_name_substr": "MIDIOUT7 (MRCC)"
  },
  "osc": {
    "listening_host": "127.0.0.1",
    "listening_port": 9069,
    "transpose_path": "/transpose",
    "transpose_up_path": "/transposeUp",
    "transpose_down_path": "/transposeDown",
    "sending_addr": "127.0.0.1",
    "sending_port": 9000,
    "sending_enabled": false,
    "send_original": true
  },
  "mqtt": {
    "broker_host": "192.168.50.200",
    "broker_port": 1883,
    "base_topic": "midi_transposer",
    "username": "your_mqtt_username",
    "password": "your_mqtt_password"
  },
  "transpose": {
    "min": -24,
    "max": 24
  }
}
```

### Configuration Sections

#### MIDI Configuration

- `input_port_name_substr`: Substring to match for MIDI input port selection
- `output_port_name_substr`: Substring to match for MIDI output port selection

#### OSC Configuration

- `listening_host`: Host/IP for OSC listener
- `listening_port`: Port for OSC listener
- `transpose_path`: OSC path for absolute transpose commands
- `transpose_up_path`: OSC path for transpose increment commands
- `transpose_down_path`: OSC path for transpose decrement commands
- `sending_addr`: Target IP address for OSC sending
- `sending_port`: Target port for OSC sending
- `sending_enabled`: If true, the program will send MIDI messages via OSC at startup. Can be toggled at runtime via the console.
- `send_original`: When `sending_enabled` is true, this selects whether to send the original input MIDI (`true`) or the transposed MIDI (`false`) via OSC at startup. Can be changed at runtime.

#### MQTT Configuration

- `broker_host`: MQTT broker hostname or IP address (HomeAssistant IP)
- `broker_port`: MQTT broker port (usually 1883)
- `base_topic`: Base topic for all MQTT messages
- `username`: MQTT authentication username
- `password`: MQTT authentication password

#### Transpose Configuration

- `min`: Minimum transpose value in semitones
- `max`: Maximum transpose value in semitones

### Migration from mqtt_credentials.json

The previous `mqtt_credentials.json` file is no longer needed. The MQTT credentials are now included directly in the main `config.json` file under the `mqtt` section.

### Default Behavior

If `config.json` is not found, the program will use built-in default values and display a warning message. The defaults match the previous hardcoded configuration.
