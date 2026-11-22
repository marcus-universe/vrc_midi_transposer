# Arpeggiator and sequencer

The **Arp / Sequencer** view turns the keys you hold into a rhythmic pattern that runs in time with a MIDI clock. The settings are part of the project and are saved with it.

## Modes

| Mode | What happens to the keys you play |
|---|---|
| **Off** | Keys are forwarded as played (and transposed), as always. |
| **Arpeggiator** | The held keys do not sound themselves. The arpeggiator plays them one after another in the chosen style. |
| **Sequencer** | The held keys do not sound themselves. The step sequence plays, starting from the last key you pressed. |

The notes the arpeggiator or sequencer plays are **transposed** like played notes, go to the MIDI output, light up the keyboard (Output view) and are sent to the VRChat avatar over OSC.

Pitch bend, mod wheel, sustain and all other messages still pass through unchanged.

## Clock

Arpeggiator and sequencer follow a **MIDI clock** (24 ticks per quarter note, 4/4 bars).

| Setting | Meaning |
|---|---|
| **Clock from the MIDI input** (default) | Uses the clock that arrives on the MIDI input you play, for example from your keyboard, DAW or drum machine. |
| **Use clock from another MIDI device** | Pick a second device whose clock drives the pattern. Only its clock is used, its notes are ignored. It cannot be the same device as the MIDI input; then use the first option. |
| **Use internal tempo when no clock arrives** | When no MIDI clock arrives for half a second, the app's own tempo (BPM) takes over. As soon as the clock comes back, it follows the clock again. |

The status line shows which clock is in use, the tempo and the beat in the bar. *No clock* means nothing advances: start the clock on your device or turn on the internal tempo.

MIDI **Start** restarts the bar count; **Song Position** jumps to the given position.

## Arpeggiator

| Setting | Values |
|---|---|
| **Style** | Up, Down, Up/Down, Down/Up, Up/Down with the top and bottom note repeated, As played, Reverse (as played, backwards), Random, Chord (all held keys on every step) |
| **Rate** | 1/1, 1/2, 1/4, 1/8, 1/16, 1/32, plus dotted (1/4, 1/8, 1/16) and triplets (1/2, 1/4, 1/8, 1/16, 1/32), always locked to the clock |
| **Octaves** | 1 to 4: the pattern repeats one or more octaves higher |
| **Gate** | Note length in percent of a step |
| **Swing** | Delays every second step |
| **Latch** | Keeps playing after you let go; the next chord you play replaces the latched one |
| **Velocity** | As played, or a fixed value |

Steps are locked to the clock grid, so the first note of a new chord starts on the next step.

## Sequencer

A pattern of up to **64 steps**. Each step has:

| Field | Meaning |
|---|---|
| **On** | Off makes the step a rest. |
| **Offset** | Semitones from the root note (−48 to +48). |
| **Velocity** | 1 to 127. |
| **Gate** | Note length in percent of a step. |
| **Tie** | Holds the previous note through this step instead of playing a new one. |

General settings: **Rate**, **Length** (1 to 64 steps), **Root note** (used while no key is held), **Channel** (until a key is played), **Swing**, and **Follow MIDI Start/Stop**.

- **Play waits for the next bar.** After you press Play, the sequencer starts on the next bar line of the clock, so it always lands on beat one. The button shows that it is waiting.
- **Follow MIDI Start/Stop:** a MIDI Start from the clock source starts the sequencer at once (Start is beat one), a MIDI Stop stops it.
- **Transposed by the held key:** while you hold a key, the steps play relative to it; let go and they play relative to the root note again. The last key pressed counts.
- **Stop** ends the sounding note at once.

### Recording

**Record** (next to Play, and in the bar under Transpose on the dashboard) records the keys you play into the steps. You hear the keys while recording, transposed like everything else.

| When you press Record | What is recorded |
|---|---|
| **Sequencer stopped**: step recording | Every key you press fills the next step, starting at step 1, timing does not matter. **Rest** leaves a step empty. Press Record again (or Play) to finish: the length becomes the number of recorded steps. |
| **Sequencer playing** (or waiting for the bar): live recording | Recording starts with the next loop (a key up to half a step early still counts) and records **one loop** in the tempo of the clock. Keys are placed on the nearest step; a key held longer than a step ties into the next steps, a short one sets the gate. Steps you did not play become rests. The old steps are quiet during that loop, and the new ones play right after it. |

The offset of a recorded step is the key minus the **root note**. Afterwards press Play: the recording plays in the tempo of the MIDI clock or the internal tempo, transposed by the held key like any sequence. The recorded steps are a normal project change; **Save** keeps them.

## MIDI control

The **MIDI control** card in the Arp / Sequencer view gives these actions a MIDI trigger, set with **Learn** like the custom controls:

| Action | Does |
|---|---|
| Switch off / Arpeggiator / Sequencer | Switches to that mode. |
| Next mode | Off → Arpeggiator → Sequencer → Off. |
| Play / Stop | Starts the sequencer (on the next bar) or stops it. |
| Record | Starts or ends recording. |
| Rest | During step recording: leaves the next step empty. |

A Note On, a CC rising to 64 or more (a button press), or a Program Change fires the action. The message is not played unless **Also to output** is on. A mode switched by MIDI lasts until you close the app or open a project; the project keeps the mode chosen in the app.
