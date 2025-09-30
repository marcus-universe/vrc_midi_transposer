/// Transpose a MIDI message in-place.
///
/// Only Note On (0x90) and Note Off (0x80) messages are modified. `transpose`
/// is the number of semitones to add (may be negative). Resulting note numbers
/// are clamped to the valid MIDI range 0..=127.
pub fn apply_transpose(msg: &mut [u8], transpose: i32) {
    if msg.len() < 2 {
        return;
    }

    let status_nibble = msg[0] & 0xF0;
    if status_nibble == 0x90 || status_nibble == 0x80 {
        let t = transpose as i16;
        let note = msg[1] as i16 + t;
        let note_clamped = if note < 0 { 0 } else if note > 127 { 127 } else { note };
        msg[1] = note_clamped as u8;
    }
}
