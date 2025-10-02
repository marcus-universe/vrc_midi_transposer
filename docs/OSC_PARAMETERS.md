# OSC Parameters Reference

This document provides a comprehensive reference for all OSC parameters sent by VRC_Midi_Transposer to VRChat and other OSC-enabled applications.

## Overview

VRC_Midi_Transposer converts MIDI messages to OSC parameters that can be used to control avatars in VRChat. The application supports a full 88-key piano range (A0 to C8) and additional control parameters.

## Note Parameters

### Parameter Format

- **Path**: `/avatar/parameters/{NoteName}{Octave}`
- **Type**: Integer
- **Values**:
  - `1` = Note On (key pressed)
  - `0` = Note Off (key released)

### Sharp Note Handling

Sharp notes (#) are converted to "SHARP" in OSC parameter names to ensure compatibility:

- `C#4` becomes `/avatar/parameters/CSHARP4`
- `F#2` becomes `/avatar/parameters/FSHARP2`
- `G#5` becomes `/avatar/parameters/GSHARP5`

## Complete 88-Key Piano Reference

### Octave -1 (Sub-contra octave)

| MIDI Note | Note Name | OSC Parameter           | Range      |
| --------- | --------- | ----------------------- | ---------- |
| 21        | A0        | `/avatar/parameters/A0` | Lowest key |

### Octave 0 (Contra octave)

| MIDI Note | Note Name | OSC Parameter                |
| --------- | --------- | ---------------------------- |
| 22        | A#0       | `/avatar/parameters/ASHARP0` |
| 23        | B0        | `/avatar/parameters/B0`      |

### Octave 1 (Great octave)

| MIDI Note | Note Name | OSC Parameter                |
| --------- | --------- | ---------------------------- |
| 24        | C1        | `/avatar/parameters/C1`      |
| 25        | C#1       | `/avatar/parameters/CSHARP1` |
| 26        | D1        | `/avatar/parameters/D1`      |
| 27        | D#1       | `/avatar/parameters/DSHARP1` |
| 28        | E1        | `/avatar/parameters/E1`      |
| 29        | F1        | `/avatar/parameters/F1`      |
| 30        | F#1       | `/avatar/parameters/FSHARP1` |
| 31        | G1        | `/avatar/parameters/G1`      |
| 32        | G#1       | `/avatar/parameters/GSHARP1` |
| 33        | A1        | `/avatar/parameters/A1`      |
| 34        | A#1       | `/avatar/parameters/ASHARP1` |
| 35        | B1        | `/avatar/parameters/B1`      |

### Octave 2 (Small octave)

| MIDI Note | Note Name | OSC Parameter                |
| --------- | --------- | ---------------------------- |
| 36        | C2        | `/avatar/parameters/C2`      |
| 37        | C#2       | `/avatar/parameters/CSHARP2` |
| 38        | D2        | `/avatar/parameters/D2`      |
| 39        | D#2       | `/avatar/parameters/DSHARP2` |
| 40        | E2        | `/avatar/parameters/E2`      |
| 41        | F2        | `/avatar/parameters/F2`      |
| 42        | F#2       | `/avatar/parameters/FSHARP2` |
| 43        | G2        | `/avatar/parameters/G2`      |
| 44        | G#2       | `/avatar/parameters/GSHARP2` |
| 45        | A2        | `/avatar/parameters/A2`      |
| 46        | A#2       | `/avatar/parameters/ASHARP2` |
| 47        | B2        | `/avatar/parameters/B2`      |

### Octave 3 (One-line octave)

| MIDI Note | Note Name | OSC Parameter                |
| --------- | --------- | ---------------------------- |
| 48        | C3        | `/avatar/parameters/C3`      |
| 49        | C#3       | `/avatar/parameters/CSHARP3` |
| 50        | D3        | `/avatar/parameters/D3`      |
| 51        | D#3       | `/avatar/parameters/DSHARP3` |
| 52        | E3        | `/avatar/parameters/E3`      |
| 53        | F3        | `/avatar/parameters/F3`      |
| 54        | F#3       | `/avatar/parameters/FSHARP3` |
| 55        | G3        | `/avatar/parameters/G3`      |
| 56        | G#3       | `/avatar/parameters/GSHARP3` |
| 57        | A3        | `/avatar/parameters/A3`      |
| 58        | A#3       | `/avatar/parameters/ASHARP3` |
| 59        | B3        | `/avatar/parameters/B3`      |

### Octave 4 (Two-line octave / Middle C)

| MIDI Note | Note Name | OSC Parameter                | Notes    |
| --------- | --------- | ---------------------------- | -------- |
| 60        | C4        | `/avatar/parameters/C4`      | Middle C |
| 61        | C#4       | `/avatar/parameters/CSHARP4` |
| 62        | D4        | `/avatar/parameters/D4`      |
| 63        | D#4       | `/avatar/parameters/DSHARP4` |
| 64        | E4        | `/avatar/parameters/E4`      |
| 65        | F4        | `/avatar/parameters/F4`      |
| 66        | F#4       | `/avatar/parameters/FSHARP4` |
| 67        | G4        | `/avatar/parameters/G4`      |
| 68        | G#4       | `/avatar/parameters/GSHARP4` |
| 69        | A4        | `/avatar/parameters/A4`      | 440 Hz   |
| 70        | A#4       | `/avatar/parameters/ASHARP4` |
| 71        | B4        | `/avatar/parameters/B4`      |

### Octave 5 (Three-line octave)

| MIDI Note | Note Name | OSC Parameter                |
| --------- | --------- | ---------------------------- |
| 72        | C5        | `/avatar/parameters/C5`      |
| 73        | C#5       | `/avatar/parameters/CSHARP5` |
| 74        | D5        | `/avatar/parameters/D5`      |
| 75        | D#5       | `/avatar/parameters/DSHARP5` |
| 76        | E5        | `/avatar/parameters/E5`      |
| 77        | F5        | `/avatar/parameters/F5`      |
| 78        | F#5       | `/avatar/parameters/FSHARP5` |
| 79        | G5        | `/avatar/parameters/G5`      |
| 80        | G#5       | `/avatar/parameters/GSHARP5` |
| 81        | A5        | `/avatar/parameters/A5`      |
| 82        | A#5       | `/avatar/parameters/ASHARP5` |
| 83        | B5        | `/avatar/parameters/B5`      |

### Octave 6 (Four-line octave)

| MIDI Note | Note Name | OSC Parameter                |
| --------- | --------- | ---------------------------- |
| 84        | C6        | `/avatar/parameters/C6`      |
| 85        | C#6       | `/avatar/parameters/CSHARP6` |
| 86        | D6        | `/avatar/parameters/D6`      |
| 87        | D#6       | `/avatar/parameters/DSHARP6` |
| 88        | E6        | `/avatar/parameters/E6`      |
| 89        | F6        | `/avatar/parameters/F6`      |
| 90        | F#6       | `/avatar/parameters/FSHARP6` |
| 91        | G6        | `/avatar/parameters/G6`      |
| 92        | G#6       | `/avatar/parameters/GSHARP6` |
| 93        | A6        | `/avatar/parameters/A6`      |
| 94        | A#6       | `/avatar/parameters/ASHARP6` |
| 95        | B6        | `/avatar/parameters/B6`      |

### Octave 7 (Five-line octave)

| MIDI Note | Note Name | OSC Parameter                |
| --------- | --------- | ---------------------------- |
| 96        | C7        | `/avatar/parameters/C7`      |
| 97        | C#7       | `/avatar/parameters/CSHARP7` |
| 98        | D7        | `/avatar/parameters/D7`      |
| 99        | D#7       | `/avatar/parameters/DSHARP7` |
| 100       | E7        | `/avatar/parameters/E7`      |
| 101       | F7        | `/avatar/parameters/F7`      |
| 102       | F#7       | `/avatar/parameters/FSHARP7` |
| 103       | G7        | `/avatar/parameters/G7`      |
| 104       | G#7       | `/avatar/parameters/GSHARP7` |
| 105       | A7        | `/avatar/parameters/A7`      |
| 106       | A#7       | `/avatar/parameters/ASHARP7` |
| 107       | B7        | `/avatar/parameters/B7`      |

### Octave 8 (Six-line octave)

| MIDI Note | Note Name | OSC Parameter           | Range       |
| --------- | --------- | ----------------------- | ----------- |
| 108       | C8        | `/avatar/parameters/C8` | Highest key |

## Pitch Bend Parameters

### Pitch Up

- **Path**: `/avatar/parameters/PitchUp`
- **Type**: Float
- **Range**: `0.0` to `1.0`
- **Description**: Positive pitch bend values (wheel up)

### Pitch Down

- **Path**: `/avatar/parameters/PitchDown`
- **Type**: Float
- **Range**: `0.0` to `1.0`
- **Description**: Negative pitch bend values (wheel down)

## Implementation Notes

### MIDI Message Handling

- **Note On (0x90)**: Sets parameter to `1`, velocity 0 treated as Note Off
- **Note Off (0x80)**: Sets parameter to `0`
- **Pitch Bend (0xE0)**: Converted to normalized float values

### VRChat Integration

These parameters can be used in VRChat avatar animations through:

- **Animator Parameters**: Create matching parameter names in your avatar's Animator Controller
- **Expression Parameters**: Add parameters to your avatar's Expression Parameters asset
- **Animation Layers**: Use parameters to drive animation blends and triggers

### Performance Considerations

- Only changed parameters are sent (state tracking)
- Messages are sent with 0.1 precision for pitch bend
- Invalid MIDI notes (>127) are filtered out

## Example Usage

### Basic Piano Avatar Setup

1. Create Bool parameters in your Animator for each key you want to animate
2. Use the parameter names exactly as listed above (e.g., `C4`, `FSHARP4`)
3. Create animation states that trigger when parameters become `true`
4. Configure VRC_Midi_Transposer to send to your VRChat OSC port (usually 9000)

### Advanced Usage

- Combine multiple octaves for extended range animations
- Use pitch bend parameters for continuous pitch effects
- Implement velocity-sensitive animations using the binary note states

## Troubleshooting

### Common Issues

- **Parameters not responding**: Check parameter names match exactly (case-sensitive)
- **Sharp notes not working**: Ensure you're using "SHARP" not "#" in parameter names
- **Pitch bend not smooth**: Check your animation controller blend trees

### Debugging

Enable OSC logging in the application to see all transmitted parameters and values.
