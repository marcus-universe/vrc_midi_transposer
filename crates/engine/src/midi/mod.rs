//! MIDI: port discovery, the transposing forwarder and held-note tracking.

pub mod forwarder;
pub mod held_notes;
pub mod mapping;
pub mod ports;

/// Client name shown to the OS. Ports carrying it are our own and are hidden from the pickers.
pub const CLIENT_NAME: &str = "VRC MIDI Transposer";

/// A MIDI message. Channel messages stay on the stack; only SysEx allocates.
#[derive(Clone, Debug)]
pub enum MidiBytes {
    Short([u8; 3], u8),
    Long(Vec<u8>),
}

impl MidiBytes {
    pub fn from_slice(bytes: &[u8]) -> Self {
        if bytes.len() <= 3 {
            let mut buf = [0u8; 3];
            buf[..bytes.len()].copy_from_slice(bytes);
            MidiBytes::Short(buf, bytes.len() as u8)
        } else {
            MidiBytes::Long(bytes.to_vec())
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        match self {
            MidiBytes::Short(buf, len) => &buf[..*len as usize],
            MidiBytes::Long(v) => v,
        }
    }
}
