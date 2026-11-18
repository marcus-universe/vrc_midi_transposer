//! Remembers which output note each held key was sent as, so Note Off always
//! releases the note that Note On started, whatever the transpose value is now.

const NONE: u8 = 0xFF;

pub fn transposed(note: u8, semitones: i32) -> u8 {
    (note as i32 + semitones).clamp(0, 127) as u8
}

pub struct HeldNotes {
    /// `[channel][input note]` -> output note, or `NONE`.
    table: [[u8; 128]; 16],
}

impl Default for HeldNotes {
    fn default() -> Self {
        HeldNotes {
            table: [[NONE; 128]; 16],
        }
    }
}

impl HeldNotes {
    /// Record a Note On. Returns the output note, and the previous output note
    /// if the key was already held and mapped elsewhere (it must be released).
    pub fn note_on(&mut self, channel: u8, note: u8, semitones: i32) -> (u8, Option<u8>) {
        let out = transposed(note, semitones);
        let slot = &mut self.table[(channel & 0x0F) as usize][(note & 0x7F) as usize];
        let previous = (*slot != NONE && *slot != out).then_some(*slot);
        *slot = out;
        (out, previous)
    }

    /// Record a Note Off. Returns the output note to release; falls back to the
    /// current transpose if the key was not known to be held.
    pub fn note_off(&mut self, channel: u8, note: u8, semitones: i32) -> u8 {
        let slot = &mut self.table[(channel & 0x0F) as usize][(note & 0x7F) as usize];
        let out = if *slot == NONE {
            transposed(note, semitones)
        } else {
            *slot
        };
        *slot = NONE;
        out
    }

    /// Output note of a held key, for aftertouch.
    pub fn get(&self, channel: u8, note: u8) -> Option<u8> {
        let v = self.table[(channel & 0x0F) as usize][(note & 0x7F) as usize];
        (v != NONE).then_some(v)
    }

    /// Remove and return every held key as `(channel, output note)`.
    pub fn drain(&mut self) -> Vec<(u8, u8)> {
        let mut out = Vec::new();
        for (channel, row) in self.table.iter_mut().enumerate() {
            for slot in row.iter_mut() {
                if *slot != NONE {
                    out.push((channel as u8, *slot));
                    *slot = NONE;
                }
            }
        }
        out
    }

    /// Every held key as `(channel, output note)`, without clearing.
    pub fn held(&self) -> Vec<(u8, u8)> {
        let mut out = Vec::new();
        for (channel, row) in self.table.iter().enumerate() {
            for slot in row.iter().filter(|s| **s != NONE) {
                out.push((channel as u8, *slot));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_to_midi_range() {
        assert_eq!(transposed(60, 3), 63);
        assert_eq!(transposed(120, 24), 127);
        assert_eq!(transposed(5, -24), 0);
    }

    #[test]
    fn note_off_releases_the_note_that_was_started() {
        let mut held = HeldNotes::default();
        let (out, prev) = held.note_on(0, 60, 0);
        assert_eq!((out, prev), (60, None));
        // Transpose changes to +5 while the key is down.
        assert_eq!(held.note_off(0, 60, 5), 60, "must release 60, not 65");
        // Unknown key: use the current transpose.
        assert_eq!(held.note_off(0, 60, 5), 65);
    }

    #[test]
    fn channels_are_independent() {
        let mut held = HeldNotes::default();
        held.note_on(0, 60, 2);
        held.note_on(1, 60, 7);
        assert_eq!(held.get(0, 60), Some(62));
        assert_eq!(held.get(1, 60), Some(67));
        assert_eq!(held.note_off(1, 60, 0), 67);
        assert_eq!(held.get(0, 60), Some(62));
        assert_eq!(held.get(1, 60), None);
    }

    #[test]
    fn retrigger_with_new_transpose_reports_the_old_note() {
        let mut held = HeldNotes::default();
        held.note_on(3, 64, 0);
        let (out, prev) = held.note_on(3, 64, 12);
        assert_eq!(out, 76);
        assert_eq!(prev, Some(64));
        let (out, prev) = held.note_on(3, 64, 12);
        assert_eq!((out, prev), (76, None));
    }

    #[test]
    fn drain_returns_everything_once() {
        let mut held = HeldNotes::default();
        held.note_on(0, 60, 1);
        held.note_on(9, 36, 0);
        assert_eq!(held.held().len(), 2);
        let mut all = held.drain();
        all.sort();
        assert_eq!(all, [(0, 61), (9, 36)]);
        assert!(held.drain().is_empty());
    }
}
