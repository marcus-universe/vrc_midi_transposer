use std::error::Error;
use std::io::{stdin, stdout, Write};

/// Select a MIDI output port. Prefers a port whose name contains
/// `output_port_name_substr` and is not identical to `in_port_name`.
pub fn choose_output_port(midi_out: &midir::MidiOutput, output_port_name_substr: &str, in_port_name: &str) -> Result<usize, Box<dyn Error>> {
    let ports = midi_out.ports();
    if ports.is_empty() {
        return Err("no output port found".into());
    }

    // Try to find a matching port (but avoid selecting the same name as the input)
    for (i, p) in ports.iter().enumerate() {
        if let Ok(name) = midi_out.port_name(p) {
            if name.contains(output_port_name_substr) && name != in_port_name {
                println!("Choosing output port matching '{}': {}", output_port_name_substr, name);
                return Ok(i);
            }
        }
    }

    // Fallbacks: single port or interactive selection
    if ports.len() == 1 {
        println!("Choosing the only available output port: {}", midi_out.port_name(&ports[0])?);
        return Ok(0);
    }

    println!("\nAvailable output ports:");
    for (i, p) in ports.iter().enumerate() {
        println!("{}: {}", i, midi_out.port_name(p)?);
    }

    print!("Please select output port: ");
    stdout().flush()?;
    let mut choice = String::new();
    stdin().read_line(&mut choice)?;
    let idx = choice.trim().parse::<usize>()?;
    if idx >= ports.len() {
        return Err("invalid output port selected".into());
    }
    Ok(idx)
}
