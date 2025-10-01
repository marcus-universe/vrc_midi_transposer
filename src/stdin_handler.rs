use std::io::stdin;
use std::thread;
use std::sync::atomic::Ordering;

/// Spawn a thread that reads lines from stdin. Empty line or 'exit' sets the
/// global `EXIT_FLAG`. A valid integer updates `TRANSPOSE_SEMITONES`.
pub fn spawn_stdin_handler() -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let stdin = stdin();
        let mut line = String::new();
        loop {
            line.clear();
            if stdin.read_line(&mut line).is_err() {
                break;
            }
            let cmd = line.trim();
            if cmd.is_empty() {
                crate::EXIT_FLAG.store(true, Ordering::SeqCst);
                break;
            }
            if cmd.eq_ignore_ascii_case("exit") || cmd.eq_ignore_ascii_case("quit") || cmd.eq_ignore_ascii_case("q") {
                crate::EXIT_FLAG.store(true, Ordering::SeqCst);
                break;
            }
            
            // OSC commands
            if cmd.eq_ignore_ascii_case("osc on") || cmd.eq_ignore_ascii_case("osc enable") {
                crate::OSC_SENDING_ENABLED.store(true, Ordering::SeqCst);
                println!("OSC sending enabled");
                continue;
            }
            if cmd.eq_ignore_ascii_case("osc off") || cmd.eq_ignore_ascii_case("osc disable") {
                crate::OSC_SENDING_ENABLED.store(false, Ordering::SeqCst);
                println!("OSC sending disabled");
                continue;
            }
            if cmd.eq_ignore_ascii_case("osc original") || cmd.eq_ignore_ascii_case("osc input") {
                crate::OSC_SEND_ORIGINAL.store(true, Ordering::SeqCst);
                println!("OSC sending original input MIDI");
                continue;
            }
            if cmd.eq_ignore_ascii_case("osc transposed") || cmd.eq_ignore_ascii_case("osc output") {
                crate::OSC_SEND_ORIGINAL.store(false, Ordering::SeqCst);
                println!("OSC sending transposed MIDI");
                continue;
            }
            if cmd.eq_ignore_ascii_case("help") || cmd.eq_ignore_ascii_case("h") {
                println!("Commands:");
                println!("  <number>         - Set transpose in semitones");
                println!("  osc on/enable    - Enable OSC sending");
                println!("  osc off/disable  - Disable OSC sending");
                println!("  osc original     - Send original input MIDI via OSC");
                println!("  osc transposed   - Send transposed MIDI via OSC");
                println!("  help/h           - Show this help");
                println!("  exit/quit/q      - Exit program");
                continue;
            }
            
            if let Ok(v) = cmd.parse::<i32>() {
                crate::TRANSPOSE_SEMITONES.store(v, Ordering::SeqCst);
                println!("Transpose set to {}", v);
            } else {
                println!("Unrecognized command: '{}'. Type 'help' for available commands.", cmd);
            }
        }
    })
}
