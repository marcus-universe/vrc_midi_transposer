use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::collections::HashMap;
use std::net::UdpSocket;
use rosc::{OscMessage, OscPacket, OscType, encoder};

// Access global debug flag from crate root
use crate::is_debug_enabled;

// MIDI note names for OSC conversion
const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// Convert MIDI note number to note name with octave (e.g., "C4", "F#5")
pub fn midi_note_to_name(note_number: u8) -> String {
    if note_number > 127 {
        return "INVALID".to_string();
    }
    
    let note_index = (note_number % 12) as usize;
    let octave = (note_number / 12) as i32 - 1;
    
    format!("{}{}", NOTE_NAMES[note_index], octave)
}

/// Convert note name for OSC path (replace # with 'Sharp', e.g., G#3 -> GSharp3)
pub fn note_name_for_osc(note_name: &str) -> String {
    note_name.replace('#', "SHARP")
}

/// Structure to hold a MIDI message for OSC processing
#[derive(Clone, Debug)]
pub struct MidiMessageForOsc {
    pub status: u8,
    pub data1: u8,
    pub data2: u8,
    #[allow(dead_code)]
    pub data3: u8,
}

impl MidiMessageForOsc {
    pub fn new(raw_bytes: &[u8]) -> Option<Self> {
        if raw_bytes.is_empty() {
            return None;
        }
        
        let status = raw_bytes[0];
        let data1 = raw_bytes.get(1).copied().unwrap_or(0);
        let data2 = raw_bytes.get(2).copied().unwrap_or(0);
        let data3 = raw_bytes.get(3).copied().unwrap_or(0);
        
        Some(MidiMessageForOsc {
            status,
            data1,
            data2,
            data3,
        })
    }
}

/// OSC sender that processes MIDI messages and sends OSC messages
pub struct OscSender {
    socket: UdpSocket,
    target_addr: String,
    key_states: HashMap<String, i32>,
}

impl OscSender {
    pub fn new(target_addr: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Explizit an IPv4-Loopback binden, um sicherzustellen, dass wir über 127.0.0.1 senden
        let socket = UdpSocket::bind("127.0.0.1:0")?;
        // Fallback: if no target provided, default to localhost:9000
        let target = if target_addr.trim().is_empty() {
            "127.0.0.1:9000".to_string()
        } else {
            target_addr.to_string()
        };

        // Socket mit Ziel verbinden, so dass send() genutzt werden kann
        socket.connect(&target)?;

        Ok(OscSender {
            socket,
            target_addr: target,
            key_states: HashMap::new(),
        })
    }
    
    /// Process and send MIDI message as OSC
    pub fn process_midi_message(&mut self, midi_msg: &MidiMessageForOsc) -> Result<(), Box<dyn std::error::Error>> {
        let status = midi_msg.status;
        let data1 = midi_msg.data1;
        let data2 = midi_msg.data2;
        
        // Validate MIDI note number
        if data1 > 127 {
            return Ok(()); // Skip invalid notes
        }
        
        match status & 0xF0 {
            // Note On (0x90..=0x9F) and Note Off (0x80..=0x8F)
            0x90 => {
                let note_name = midi_note_to_name(data1);
                let osc_note_name = note_name_for_osc(&note_name);

                // Velocity 0 on Note On is Note Off per MIDI spec
                let note_state_int = if data2 > 0 { 1 } else { 0 };

                // Update key state
                self.key_states.insert(note_name.clone(), note_state_int);

                // Create and send OSC message
                let osc_path = format!("/avatar/parameters/{}", osc_note_name);
                let osc_msg = OscMessage { addr: osc_path, args: vec![OscType::Int(note_state_int)] };
                self.send_osc_message(osc_msg)?;
            }
            0x80 => {
                let note_name = midi_note_to_name(data1);
                let osc_note_name = note_name_for_osc(&note_name);
                let note_state_int = 0;

                self.key_states.insert(note_name.clone(), note_state_int);

                let osc_path = format!("/avatar/parameters/{}", osc_note_name);
                let osc_msg = OscMessage { addr: osc_path, args: vec![OscType::Int(note_state_int)] };
                self.send_osc_message(osc_msg)?;
            }

            // Pitch Bend (0xE0..=0xEF)
            0xE0 => {
                let pitch_bend_raw = (data2 as i32 * 128 + data1 as i32) - 8192;
                let pitch_bend_value = (pitch_bend_raw as f32 / 8192.0).max(-1.0).min(1.0);
                let pitch_bend_rounded = (pitch_bend_value * 10.0).round() / 10.0;

                if pitch_bend_rounded > 0.0 {
                    let osc_msg = OscMessage { addr: "/avatar/parameters/PitchUp".to_string(), args: vec![OscType::Float(pitch_bend_rounded)] };
                    self.send_osc_message(osc_msg)?;
                } else if pitch_bend_rounded < 0.0 {
                    let osc_msg = OscMessage { addr: "/avatar/parameters/PitchDown".to_string(), args: vec![OscType::Float(pitch_bend_rounded.abs())] };
                    self.send_osc_message(osc_msg)?;
                }
            }

            _ => {
                // Ignore other MIDI messages for now
            }
        }
        
        Ok(())
    }
    
    /// Send OSC message via UDP
    fn send_osc_message(&self, msg: OscMessage) -> Result<(), Box<dyn std::error::Error>> {
        let packet = OscPacket::Message(msg.clone());
        let msg_buf = encoder::encode(&packet)?;
        match self.socket.send(&msg_buf) {
            Ok(bytes_sent) => {
                if is_debug_enabled() {
                    println!("[OSC] Sent {} bytes to {}: {}", bytes_sent, self.target_addr, msg.addr);
                }
                Ok(())
            }
            Err(e) => {
                eprintln!("[OSC] Failed to send to {}: {}", self.target_addr, e);
                Err(Box::new(e))
            }
        }
    }
}

/// Send a single OSC message (addr, value) directly to the configured OSC target.
/// Bool is represented by 0/1 int. Float uses provided value (no rounding).
pub fn send_single_osc_message(addr: &str, value: OscType, target_addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Bind ephemeral local IPv4 and connect to target
    let socket = std::net::UdpSocket::bind("127.0.0.1:0")?;
    let target = if target_addr.trim().is_empty() { "127.0.0.1:9000".to_string() } else { target_addr.to_string() };
    socket.connect(&target)?;
    let msg = OscMessage { addr: addr.to_string(), args: vec![value] };
    let packet = OscPacket::Message(msg.clone());
    let msg_buf = encoder::encode(&packet)?;
    match socket.send(&msg_buf) {
        Ok(bytes_sent) => {
            if is_debug_enabled() {
                println!("[OSC] Sent {} bytes to {}: {}", bytes_sent, target, msg.addr);
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("[OSC] Failed to send to {}: {}", target, e);
            Err(Box::new(e))
        }
    }
}

/// Spawn OSC sender thread that processes MIDI messages and sends OSC
pub fn spawn_osc_sender(
    target_addr: String,
    midi_receiver: Receiver<Vec<u8>>,
    enable_flag: &'static AtomicBool,
) -> JoinHandle<()> {
    thread::spawn(move || {
        crate::general::check::mark_osc_sender_started();
        let mut osc_sender = match OscSender::new(&target_addr) {
            Ok(sender) => sender,
            Err(e) => {
                eprintln!("Failed to create OSC sender: {}", e);
                crate::general::check::mark_osc_sender_stopped();
                return;
            }
        };
        
        if is_debug_enabled() {
            if let Ok(local_addr) = osc_sender.socket.local_addr() {
                println!("OSC sender thread started, local {} -> target {}", local_addr, osc_sender.target_addr);
            } else {
                println!("OSC sender thread started, sending to: {}", target_addr);
            }
        }
        
        loop {
            // Exit promptly on global shutdown
            if crate::EXIT_FLAG.load(Ordering::SeqCst) {
                break;
            }
            // Check if OSC sending is enabled
            if !enable_flag.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }
            
            // Try to receive MIDI message with timeout
            match midi_receiver.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(raw_bytes) => {
                    if let Some(midi_msg) = MidiMessageForOsc::new(&raw_bytes) {
                        if let Err(e) = osc_sender.process_midi_message(&midi_msg) {
                            eprintln!("Error processing MIDI message for OSC: {}", e);
                        }
                    }
                },
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Continue loop, check enable flag again
                    continue;
                },
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    if is_debug_enabled() {
                        println!("OSC sender: MIDI receiver disconnected, shutting down");
                    }
                    break;
                }
            }
        }
        if is_debug_enabled() {
            println!("OSC sender thread terminated");
        }
        crate::general::check::mark_osc_sender_stopped();
    })
}

/// Create a channel pair for sending MIDI data to OSC sender
pub fn create_osc_sender_channel() -> (Sender<Vec<u8>>, Receiver<Vec<u8>>) {
    channel()
}
