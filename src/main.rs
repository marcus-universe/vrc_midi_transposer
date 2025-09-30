use std::error::Error;
use std::io::{stdin, stdout, Write};

use midir::{Ignore, MidiInput, MidiOutput};
use std::sync::mpsc::channel;
use std::sync::{Arc, atomic::{AtomicI32, AtomicBool, Ordering}};
use std::thread;
use std::time::Duration;

fn main() {
    match run() {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err),
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut input = String::new();
    // User-selectable port name substrings. Adjust these variables to pick different ports.
    let input_port_name_substr = "MRCC"; // example: "MRCC"
    let output_port_name_substr = "MIDIOUT7 (MRCC)"; // example: "MIDIOUT7 (MRCC)"

    let mut midi_in = MidiInput::new("midir reading input")?;
    midi_in.ignore(Ignore::None);

    let midi_out = MidiOutput::new("midir forwarding output")?;

    // Choose input port by substring match (first match). Falls back to explicit selection if none/multiple found.
    let in_ports = midi_in.ports();
    let in_port = if in_ports.is_empty() {
        return Err("no input port found".into());
    } else {
        // Try to find a port whose name contains the requested substring; capture its index directly
        let mut found_idx: Option<usize> = None;
        for (i, p) in in_ports.iter().enumerate() {
            if let Ok(name) = midi_in.port_name(p) {
                if name.contains(input_port_name_substr) {
                    found_idx = Some(i);
                    break;
                }
            }
        }
        match found_idx {
            Some(idx) => {
                println!("Choosing input port matching '{}': {}", input_port_name_substr, midi_in.port_name(&in_ports[idx]).unwrap());
                &in_ports[idx]
            }
            None => {
                // Fallback to asking the user if no match
                if in_ports.len() == 1 {
                    println!("Choosing the only available input port: {}", midi_in.port_name(&in_ports[0]).unwrap());
                    &in_ports[0]
                } else {
                    println!("\nAvailable input ports:");
                    for (i, p) in in_ports.iter().enumerate() {
                        println!("{}: {}", i, midi_in.port_name(p).unwrap());
                    }
                    print!("Please select input port: ");
                    stdout().flush()?;
                    let mut input = String::new();
                    stdin().read_line(&mut input)?;
                    in_ports
                        .get(input.trim().parse::<usize>()?)
                        .ok_or("invalid input port selected")?
                }
            }
        }
    };

    println!("\nOpening input connection");
    let in_port_name = midi_in.port_name(in_port)?;

    // Create a channel to forward received midi messages to the sender thread which owns the output connection
    let (tx, rx) = channel::<Vec<u8>>();

    // Open the MIDI output port (choose by name substring). Prefer an output whose name
    // matches the requested substring but is not the exact same name as the selected input port.
    let out_ports = midi_out.ports();
    if out_ports.is_empty() {
        return Err("no output port found".into());
    }
    let mut chosen_out_idx: Option<usize> = None;
    for (i, p) in out_ports.iter().enumerate() {
        if let Ok(name) = midi_out.port_name(p) {
            // Skip a port whose name equals the input name to avoid MRCC -> MRCC routing
            if name.contains(output_port_name_substr) && name != in_port_name {
                chosen_out_idx = Some(i);
                break;
            }
        }
    }
    let out_port = match chosen_out_idx {
        Some(idx) => {
            println!("Choosing output port matching '{}': {}", output_port_name_substr, midi_out.port_name(&out_ports[idx]).unwrap());
            &out_ports[idx]
        }
        None => {
            // Fallback: if only one output, use it, otherwise ask user
            if out_ports.len() == 1 {
                println!("Choosing the only available output port: {}", midi_out.port_name(&out_ports[0]).unwrap());
                &out_ports[0]
            } else {
                println!("\nAvailable output ports:");
                for (i, p) in out_ports.iter().enumerate() {
                    println!("{}: {}", i, midi_out.port_name(p).unwrap());
                }
                print!("Please select output port: ");
                stdout().flush()?;
                let mut input = String::new();
                stdin().read_line(&mut input)?;
                out_ports
                    .get(input.trim().parse::<usize>()?)
                    .ok_or("invalid output port selected")?
            }
        }
    };

    // Resolve output port name before connecting (connect takes ownership of midi_out)
    let out_port_name = midi_out.port_name(out_port)?;
    // Ask user for initial transpose amount (in semitones)
    print!("Transpose semitones (e.g. -12..12). Enter 0 for none: ");
    stdout().flush()?;
    input.clear();
    stdin().read_line(&mut input)?;
    let initial_transpose: i32 = input.trim().parse::<i32>().unwrap_or(0);
    println!("Using initial transpose: {} semitones", initial_transpose);

    // Use atomic so we can change transpose while playing
    let transpose_atomic = Arc::new(AtomicI32::new(initial_transpose));
    let exit_flag = Arc::new(AtomicBool::new(false));

    // Connect the output; we'll move this connection into the forwarding thread
    let mut conn_out = midi_out.connect(out_port, "midir-forward-output")?;

    // Connect the input: print incoming messages (so you can see them) and send raw messages to the channel
    let _conn_in = midi_in.connect(
        in_port,
        "midir-read-input",
        move |_stamp, message, _| {
            // Print incoming MIDI messages (timestamp, bytes, length)
            // println!("{}: {:?} (len = {})", stamp, message, message.len());
            // Forward raw bytes so sustain/pitchwheel/etc. are preserved
            let _ = tx.send(message.to_vec());
        },
        (),
    )?;

    println!(
        "Connection open, forwarding from '{}' -> '{}' (type a number and Enter to change transpose, empty line or 'exit' to quit)...",
        in_port_name,
        out_port_name
    );

    // Spawn a thread that owns the output connection and forwards messages from the channel
    // Apply transpose to Note On/Off messages before sending
    let transpose_fwd = transpose_atomic.clone();
    let forward_handle = thread::spawn(move || {
        for msg in rx {
            if msg.is_empty() {
                continue;
            }
            // Prepare message to send (we own msg)
            let mut out_msg = msg;
            if out_msg.len() >= 2 {
                let status_nibble = out_msg[0] & 0xF0;
                // Note On (0x90) or Note Off (0x80)
                if status_nibble == 0x90 || status_nibble == 0x80 {
                    let t = transpose_fwd.load(Ordering::Relaxed) as i16;
                    let note = out_msg[1] as i16 + t;
                    // Clamp to valid MIDI note range
                    let note_clamped = if note < 0 { 0 } else if note > 127 { 127 } else { note };
                    out_msg[1] = note_clamped as u8;
                }
            }
            if let Err(err) = conn_out.send(&out_msg) {
                eprintln!("Error sending MIDI message to output: {}", err);
            }
        }
        // When the receiver is closed the loop exits and the connection will drop here
    });

    // Spawn a thread to accept live transpose commands (type a number and Enter to change transpose)
    let transpose_cmd = transpose_atomic.clone();
    let exit_cmd = exit_flag.clone();
    let stdin_handle = thread::spawn(move || {
        let stdin = stdin();
        let mut line = String::new();
        loop {
            line.clear();
            if stdin.read_line(&mut line).is_err() {
                break;
            }
            let cmd = line.trim();
            if cmd.is_empty() {
                // empty line -> exit
                exit_cmd.store(true, Ordering::SeqCst);
                break;
            }
            if cmd.eq_ignore_ascii_case("exit") || cmd.eq_ignore_ascii_case("quit") || cmd.eq_ignore_ascii_case("q") {
                exit_cmd.store(true, Ordering::SeqCst);
                break;
            }
            if let Ok(v) = cmd.parse::<i32>() {
                transpose_cmd.store(v, Ordering::SeqCst);
                println!("Transpose set to {}", v);
            } else {
                println!("Unrecognized command: '{}'. Enter a number to set transpose or empty/exit to quit.", cmd);
            }
        }
    });

    // Wait for exit signal from stdin thread
    while !exit_flag.load(Ordering::SeqCst) {
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
