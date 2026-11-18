//! MIDI to VRChat avatar parameter mapping. Part of the public contract.

use std::sync::OnceLock;

const NOTE_NAMES: [&str; 12] = [
    "C", "CSHARP", "D", "DSHARP", "E", "F", "FSHARP", "G", "GSHARP", "A", "ASHARP", "B",
];

pub const PITCH_UP: &str = "/avatar/parameters/PitchUp";
pub const PITCH_DOWN: &str = "/avatar/parameters/PitchDown";

/// `/avatar/parameters/<Name><Octave>`, e.g. note 60 -> `/avatar/parameters/C4`.
pub fn note_address(note: u8) -> &'static str {
    static TABLE: OnceLock<Vec<String>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        (0..128usize)
            .map(|n| format!("/avatar/parameters/{}{}", NOTE_NAMES[n % 12], n as i32 / 12 - 1))
            .collect()
    });
    &table[(note & 0x7F) as usize]
}

/// Pitch bend as an avatar parameter: `PitchUp` or `PitchDown` with 0.1..=1.0
/// in steps of 0.1. `None` at the centre position.
pub fn pitch_bend(lsb: u8, msb: u8) -> Option<(&'static str, f32)> {
    let raw = (msb as i32 & 0x7F) * 128 + (lsb as i32 & 0x7F) - 8192;
    let value = (raw as f32 / 8192.0).clamp(-1.0, 1.0);
    let rounded = (value * 10.0).round() / 10.0;
    if rounded > 0.0 {
        Some((PITCH_UP, rounded))
    } else if rounded < 0.0 {
        Some((PITCH_DOWN, rounded.abs()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_names_match_the_avatar_parameters() {
        assert_eq!(note_address(60), "/avatar/parameters/C4");
        assert_eq!(note_address(61), "/avatar/parameters/CSHARP4");
        assert_eq!(note_address(22), "/avatar/parameters/ASHARP0");
        assert_eq!(note_address(21), "/avatar/parameters/A0");
        assert_eq!(note_address(108), "/avatar/parameters/C8");
        assert_eq!(note_address(0), "/avatar/parameters/C-1");
        assert_eq!(note_address(127), "/avatar/parameters/G9");
    }

    #[test]
    fn pitch_bend_scaling() {
        assert_eq!(pitch_bend(0x00, 0x40), None, "centre");
        assert_eq!(pitch_bend(0x7F, 0x7F), Some((PITCH_UP, 1.0)));
        assert_eq!(pitch_bend(0x00, 0x00), Some((PITCH_DOWN, 1.0)));
        assert_eq!(pitch_bend(0x00, 0x60), Some((PITCH_UP, 0.5)));
        assert_eq!(pitch_bend(0x00, 0x20), Some((PITCH_DOWN, 0.5)));
        assert_eq!(pitch_bend(0x10, 0x40), None, "tiny movement rounds to centre");
    }
}
