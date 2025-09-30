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

// ---------------------------------------------------------------------------
// Configuration: edit these if you want to change the default port selection
// ---------------------------------------------------------------------------
const INPUT_PORT_NAME_SUBSTR: &str = "MRCC";
const OUTPUT_PORT_NAME_SUBSTR: &str = "MIDIOUT7 (MRCC)";

// ---------------------------------------------------------------------------
// Global runtime state (shared via atomics)
// ---------------------------------------------------------------------------
/// Current transpose amount in semitones. Updated by stdin handler thread.
static TRANSPOSE_SEMITONES: AtomicI32 = AtomicI32::new(0);

/// When true the main loop will terminate and the program will shut down.
static EXIT_FLAG: AtomicBool = AtomicBool::new(false);

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
    println!("Using initial transpose: {} semitones (type number+Enter to change, empty line or 'exit' to quit)", initial_transpose);

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
        },
        (),
    )?;

    println!(
        "Connection open, forwarding from '{}' -> '{}' (type number+Enter to change transpose, empty line or 'exit' to quit)...",
        in_port_name,
        out_port_name
    );

    // Spawn forwarder thread (owns the output connection and applies transpose)
    let forward_handle = forwarder::spawn_forwarder(conn_out, rx);

    // Spawn stdin handler (updates TRANSPOSE_SEMITONES and EXIT_FLAG)
    let stdin_handle = stdin_handler::spawn_stdin_handler();

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

    Ok(())
}
