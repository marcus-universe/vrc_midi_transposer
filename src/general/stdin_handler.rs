use std::io::stdin;
use std::thread;
use std::sync::atomic::Ordering;

/// Spawn a thread that reads lines from stdin. Empty line or 'exit' sets the
/// global `EXIT_FLAG`. A valid integer updates `TRANSPOSE_SEMITONES`.
pub fn spawn_stdin_handler() -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let stdin = stdin();
        let mut line = String::new();
        // Optional handle for MQTT thread when toggled from console
        let mut mqtt_handle: Option<std::thread::JoinHandle<()>> = None;
        loop {
            line.clear();
            if stdin.read_line(&mut line).is_err() {
                break;
            }
            let cmd = line.trim();
            if cmd.is_empty() {
                crate::EXIT_FLAG.store(true, Ordering::SeqCst);
                crate::MQTT_ENABLED.store(false, Ordering::SeqCst);
                break;
            }
            if cmd.eq_ignore_ascii_case("exit") || cmd.eq_ignore_ascii_case("quit") || cmd.eq_ignore_ascii_case("q") {
                crate::EXIT_FLAG.store(true, Ordering::SeqCst);
                crate::MQTT_ENABLED.store(false, Ordering::SeqCst);
                break;
            }
            
            // Debug toggle commands
            if cmd.eq_ignore_ascii_case("debug on") || cmd.eq_ignore_ascii_case("debug enable") {
                crate::DEBUG_ENABLED.store(true, std::sync::atomic::Ordering::SeqCst);
                println!("Debug enabled");
                continue;
            }
            if cmd.eq_ignore_ascii_case("debug off") || cmd.eq_ignore_ascii_case("debug disable") {
                crate::DEBUG_ENABLED.store(false, std::sync::atomic::Ordering::SeqCst);
                println!("Debug disabled");
                continue;
            }

            // OSC commands (accept text and numeric forms)
            if cmd.eq_ignore_ascii_case("osc on") || cmd.eq_ignore_ascii_case("osc enable") || cmd == "1" {
                crate::OSC_SENDING_ENABLED.store(true, Ordering::SeqCst);
                println!("OSC sending enabled");
                continue;
            }
            if cmd.eq_ignore_ascii_case("osc off") || cmd.eq_ignore_ascii_case("osc disable") || cmd == "0" {
                crate::OSC_SENDING_ENABLED.store(false, Ordering::SeqCst);
                println!("OSC sending disabled");
                continue;
            }

            // osc_original flag: text or numeric via 'osc_original 1' / 'osc_original 0'
            if cmd.eq_ignore_ascii_case("osc original") || cmd.eq_ignore_ascii_case("osc input") || cmd.eq_ignore_ascii_case("osc_original") {
                crate::OSC_SEND_ORIGINAL.store(true, Ordering::SeqCst);
                println!("OSC sending original input MIDI");
                continue;
            }
            if cmd.eq_ignore_ascii_case("osc transposed") || cmd.eq_ignore_ascii_case("osc output") || cmd.eq_ignore_ascii_case("osc_transposed") {
                crate::OSC_SEND_ORIGINAL.store(false, Ordering::SeqCst);
                println!("OSC sending transposed MIDI");
                continue;
            }

            // Numeric and explicit forms for osc_original: allow 'osc_original 1' / 'osc_original 0' or 'osc_original:1'
            if cmd.starts_with("osc_original ") || cmd.starts_with("osc_original:") || cmd.eq_ignore_ascii_case("osc_original on") || cmd.eq_ignore_ascii_case("osc_original off") || cmd.eq_ignore_ascii_case("osc_original enable") || cmd.eq_ignore_ascii_case("osc_original disable") {
                let parts: Vec<&str> = cmd.split(|c| c == ' ' || c == ':').collect();
                if parts.len() >= 2 {
                    match parts[1].trim() {
                        "1" => {
                            crate::OSC_SEND_ORIGINAL.store(true, Ordering::SeqCst);
                            println!("OSC sending original input MIDI");
                            continue;
                        }
                        "0" => {
                            crate::OSC_SEND_ORIGINAL.store(false, Ordering::SeqCst);
                            println!("OSC sending transposed MIDI");
                            continue;
                        }
                        _ => {
                            // If the command was 'osc_original on/enable' or 'osc_original off/disable', handle it here
                            if cmd.eq_ignore_ascii_case("osc_original on") || cmd.eq_ignore_ascii_case("osc_original enable") {
                                crate::OSC_SEND_ORIGINAL.store(true, Ordering::SeqCst);
                                println!("OSC sending original input MIDI");
                                continue;
                            }
                            if cmd.eq_ignore_ascii_case("osc_original off") || cmd.eq_ignore_ascii_case("osc_original disable") {
                                crate::OSC_SEND_ORIGINAL.store(false, Ordering::SeqCst);
                                println!("OSC sending transposed MIDI");
                                continue;
                            }
                            // fallthrough to unrecognized
                        }
                    }
                }
            }
            if cmd.eq_ignore_ascii_case("help") || cmd.eq_ignore_ascii_case("h") {
                println!("Commands:");
                println!("  <number>         - Set transpose in semitones");
                println!("  osc on/enable    - Enable OSC sending");
                println!("  osc off/disable  - Disable OSC sending");
                println!("  osc original     - Send original input MIDI via OSC");
                println!("  osc transposed   - Send transposed MIDI via OSC");
                println!("  mqtt on/off      - Enable/Disable MQTT listener");
                println!("  debug on/off     - Enable/Disable verbose debug prints");
                println!("  help/h           - Show this help");
                println!("  exit/quit/q      - Exit program");
                continue;
            }
            
            // MQTT toggle commands
            if cmd.eq_ignore_ascii_case("mqtt on") || cmd.eq_ignore_ascii_case("mqtt enable") {
                crate::MQTT_ENABLED.store(true, std::sync::atomic::Ordering::SeqCst);
                // Spawn MQTT listener if not running yet
                if mqtt_handle.is_none() {
                    mqtt_handle = Some(crate::mqtt_listener::spawn_mqtt_listener());
                }
                println!("MQTT enabled");
                continue;
            }
            if cmd.eq_ignore_ascii_case("mqtt off") || cmd.eq_ignore_ascii_case("mqtt disable") {
                crate::MQTT_ENABLED.store(false, std::sync::atomic::Ordering::SeqCst);
                println!("MQTT disabled (listener will stop on next reconnect/exit)");
                continue;
            }

            if let Ok(v) = cmd.parse::<i32>() {
                let clamped_value = crate::set_transpose_semitones(v);
                println!("Transpose set to {}", clamped_value);
            } else {
                println!("Unrecognized command: '{}'. Type 'help' for available commands.", cmd);
            }
        }
    })
}
