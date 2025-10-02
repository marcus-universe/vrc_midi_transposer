/// Small helper functions for transpose handling
pub fn clamp_transpose(value: i32, min: i8, max: i8) -> i32 {
    value.clamp(min as i32, max as i32)
}

/// Apply transpose in-place to a raw MIDI message buffer.
/// Only note-on (0x9x) and note-off (0x8x) messages with a note number at byte 1 are transposed.
pub fn apply_transpose(buf: &mut [u8], semitones: i32) {
    if buf.is_empty() { return; }
    let status = buf[0] & 0xF0;
    match status {
        0x80 | 0x90 => {
            if buf.len() > 1 {
                let note = buf[1] as i32;
                let new_note = (note + semitones).clamp(0, 127) as u8;
                buf[1] = new_note;
            }
        }
        _ => {
            // other messages unchanged
        }
    }
}
