use std::error::Error;
// no direct stdin/stdout usage here; stdin is handled by `stdin_handler.rs`
use std::sync::mpsc::channel;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::thread;
use std::time::Duration;

use midir::{Ignore, MidiInput, MidiOutput};

mod input;
mod output;
mod transpose;
mod forwarder;
mod stdin_handler;
mod osc_listener;
mod osc_sender;

// ---------------------------------------------------------------------------
// Configuration: edit these if you want to change the default port selection
// ---------------------------------------------------------------------------
const INPUT_PORT_NAME_SUBSTR: &str = "MRCC";
const OUTPUT_PORT_NAME_SUBSTR: &str = "MIDIOUT7 (MRCC)";

// ---------------------------------------------------------------------------
// OSC configuration (address and paths)
// ---------------------------------------------------------------------------
pub const OSC_LISTENING_ADDR: &str = "192.168.50.78:9069";
pub const OSC_TRANSPOSE_PATH: &str = "/transpose";
pub const OSC_TRANSPOSE_UP_PATH: &str = "/transposeUp";
pub const OSC_TRANSPOSE_DOWN_PATH: &str = "/transposeDown";

// OSC sending configuration
pub const OSC_SENDING_ADDR: &str = "127.0.0.1";
pub const OSC_SENDING_PORT: u16 = 9000;

// Optional lowercase aliases for ergonomic access where desired
pub use OSC_LISTENING_ADDR as osc_listening_addr;
pub use OSC_TRANSPOSE_PATH as osc_transpose_path;
pub use OSC_SENDING_ADDR as osc_sending_addr;
pub use OSC_SENDING_PORT as osc_sending_port;

// ---------------------------------------------------------------------------
// Global runtime state (shared via atomics)
// ---------------------------------------------------------------------------
/// Current transpose amount in semitones. Updated by stdin handler thread.
static TRANSPOSE_SEMITONES: AtomicI32 = AtomicI32::new(0);

/// When true the main loop will terminate and the program will shut down.
static EXIT_FLAG: AtomicBool = AtomicBool::new(false);

/// Enable OSC sending of MIDI data (true = enabled, false = disabled)
static OSC_SENDING_ENABLED: AtomicBool = AtomicBool::new(false);

/// Send original input MIDI (true) or transposed MIDI (false) via OSC
static OSC_SEND_ORIGINAL: AtomicBool = AtomicBool::new(true);

fn main() {
    match run() {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err),
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    // Using global constants INPUT_PORT_NAME_SUBSTR and OUTPUT_PORT_NAME_SUBSTR

    let mut midi_in = MidiInput::new("midir reading input")?;
    midi_in.ignore(Ignore::None);

    let midi_out = MidiOutput::new("midir forwarding output")?;

    // Choose input port by substring match (first match). Falls back to explicit selection if none/multiple found.
    // Choose input port (substring or interactive selection)
    let input_index = input::choose_input_port(&midi_in, INPUT_PORT_NAME_SUBSTR)?;
    let in_ports = midi_in.ports();
    let in_port = &in_ports[input_index];

    println!("\nOpening input connection");
    let in_port_name = midi_in.port_name(in_port)?;

    // Channel: midi input callback -> forwarder thread
    let (tx, rx) = channel::<Vec<u8>>();
    
    // Channel: original MIDI -> OSC sender (for original input MIDI)
    let (osc_original_tx, osc_original_rx) = osc_sender::create_osc_sender_channel();
    
    // Channel: transposed MIDI -> OSC sender (for transposed MIDI)
    let (osc_transposed_tx, osc_transposed_rx) = osc_sender::create_osc_sender_channel();

    // Open the MIDI output port (choose by name substring). Prefer an output whose name
    // matches the requested substring but is not the exact same name as the selected input port.
    // Choose output port (substring or interactive selection)
    let output_index = output::choose_output_port(&midi_out, OUTPUT_PORT_NAME_SUBSTR, &in_port_name)?;
    let out_ports = midi_out.ports();
    let out_port = &out_ports[output_index];

    // Resolve output port name before connecting (connect takes ownership of midi_out)
    let out_port_name = midi_out.port_name(out_port)?;
    // Use default initial transpose 0 so forwarding starts immediately.
    // The spawned stdin handler thread still accepts numbers to change transpose later.
    let initial_transpose: i32 = 0;
    println!("Using initial transpose: {} semitones", initial_transpose);
    println!("OSC sending: {} (to {}:{})", 
        if OSC_SENDING_ENABLED.load(Ordering::SeqCst) { "enabled" } else { "disabled" },
        OSC_SENDING_ADDR, OSC_SENDING_PORT);
    println!("Type 'help' for commands, 'exit' to quit");

    // Initialize global atomics used by helper threads
    TRANSPOSE_SEMITONES.store(initial_transpose, Ordering::SeqCst);
    EXIT_FLAG.store(false, Ordering::SeqCst);

    // Connect the output; we'll move this connection into the forwarding thread
    let conn_out = midi_out.connect(out_port, "midir-forward-output")?;

    // Connect the input: print incoming messages (so you can see them) and send raw messages to the channel
    let _conn_in = midi_in.connect(
        in_port,
        "midir-read-input",
        move |_stamp, message, _| {
            // Forward raw bytes so sustain/pitchwheel/etc. are preserved
            let _ = tx.send(message.to_vec());
            
            // Send original MIDI to OSC if enabled and configured for original
            if OSC_SENDING_ENABLED.load(Ordering::SeqCst) && OSC_SEND_ORIGINAL.load(Ordering::SeqCst) {
                let _ = osc_original_tx.send(message.to_vec());
            }
        },
        (),
    )?;

    println!(
        "Connection open, forwarding from '{}' -> '{}' (type number+Enter to change transpose, empty line or 'exit' to quit)...",
        in_port_name,
        out_port_name
    );

    // Spawn forwarder thread (owns the output connection and applies transpose)
    let forward_handle = forwarder::spawn_forwarder(conn_out, rx, Some(osc_transposed_tx));

    // Spawn stdin handler (updates TRANSPOSE_SEMITONES and EXIT_FLAG)
    let stdin_handle = stdin_handler::spawn_stdin_handler();

    // Spawn OSC listener on UDP port 9069 (updates TRANSPOSE_SEMITONES on /transpose)
    let osc_handle = osc_listener::spawn_osc_listener();

    // Spawn OSC sender threads for both original and transposed MIDI
    let osc_target_addr = format!("{}:{}", OSC_SENDING_ADDR, OSC_SENDING_PORT);
    let osc_original_handle = osc_sender::spawn_osc_sender(
        osc_target_addr.clone(),
        osc_original_rx,
        &OSC_SENDING_ENABLED,
    );
    let osc_transposed_handle = osc_sender::spawn_osc_sender(
        osc_target_addr,
        osc_transposed_rx,
        &OSC_SENDING_ENABLED,
    );

    // Wait for exit signal coming from stdin handler
    while !EXIT_FLAG.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(100));
    }

    println!("Closing connections and exiting...");
    // Dropping _conn_in will stop the input callback which will eventually close the sender and end the forward thread
    drop(_conn_in);
    // Join helper threads
    let _ = stdin_handle.join();
    let _ = forward_handle.join();
    let _ = osc_handle.join();
    let _ = osc_original_handle.join();
    let _ = osc_transposed_handle.join();

    Ok(())
}
