use std::error::Error;
use std::io::{stdin, stdout, Write};

/// Select a MIDI input port. First tries to find a port whose name contains
/// `input_port_name_substr`. If no match is found and there are multiple ports,
/// prompts the user to choose one interactively.
pub fn choose_input_port(midi_in: &midir::MidiInput, input_port_name_substr: &str) -> Result<usize, Box<dyn Error>> {
    let ports = midi_in.ports();
    if ports.is_empty() {
        return Err("no input port found".into());
    }

    // Try substring match first
    for (i, p) in ports.iter().enumerate() {
        if let Ok(name) = midi_in.port_name(p) {
            if name.contains(input_port_name_substr) {
                println!("Choosing input port matching '{}': {}", input_port_name_substr, name);
                return Ok(i);
            }
        }
    }

    // Fallbacks: only one port -> choose it, otherwise list and ask
    if ports.len() == 1 {
        println!("Choosing the only available input port: {}", midi_in.port_name(&ports[0])?);
        return Ok(0);
    }

    println!("\nAvailable input ports:");
    for (i, p) in ports.iter().enumerate() {
        println!("{}: {}", i, midi_in.port_name(p)?);
    }

    print!("Please select input port: ");
    stdout().flush()?;
    let mut choice = String::new();
    stdin().read_line(&mut choice)?;
    let idx = choice.trim().parse::<usize>()?;
    if idx >= ports.len() {
        return Err("invalid input port selected".into());
    }
    Ok(idx)
}
