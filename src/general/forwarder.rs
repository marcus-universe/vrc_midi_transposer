use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::sync::atomic::Ordering;

/// Spawn a forwarding thread that owns the provided `conn_out` and listens on `rx`.
/// Each incoming raw MIDI message is transposed (using the global
/// `crate::TRANSPOSE_SEMITONES`) and forwarded to the output port.
/// Also sends transposed MIDI to OSC if enabled and configured.
pub fn spawn_forwarder(
    mut conn_out: midir::MidiOutputConnection, 
    rx: Receiver<Vec<u8>>,
    osc_transposed_tx: Option<Sender<Vec<u8>>>
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for msg in rx {
            if msg.is_empty() {
                continue;
            }
            let mut out_msg = msg;
            let t = crate::TRANSPOSE_SEMITONES.load(Ordering::Relaxed);
            crate::transpose::apply_transpose(&mut out_msg, t as i32);
            
            // Send MIDI output
            if let Err(err) = conn_out.send(&out_msg) {
                eprintln!("Error sending MIDI message to output: {}", err);
            }
            
            // Send transposed MIDI to OSC if enabled and configured for transposed
            if let Some(ref osc_tx) = osc_transposed_tx {
                if crate::OSC_SENDING_ENABLED.load(Ordering::SeqCst) && !crate::OSC_SEND_ORIGINAL.load(Ordering::SeqCst) {
                    let _ = osc_tx.send(out_msg.clone());
                }
            }
        }
        // Receiver closed -> thread exits
    })
}
