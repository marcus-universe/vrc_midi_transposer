use std::error::Error;
use std::io::Write;
// no direct stdin/stdout usage here; stdin is handled by `stdin_handler.rs`
use std::sync::mpsc::channel;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::thread;
use std::time::Duration;

use midir::{Ignore, MidiInput, MidiOutput};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

mod io;
mod remote;
mod general;

// Re-export renamed modules to keep existing `crate::input` etc. references working
pub use io::input;
pub use io::output;
pub use general::stdin_handler;
pub use general::transpose;
pub use remote::osc_listener;
pub use remote::osc_sender;
pub use remote::mqtt_listener;
pub use general::forwarder;

// ---------------------------------------------------------------------------
// Splash: print ASCII art logo in blue on supported terminals (incl. Windows CMD)
// ---------------------------------------------------------------------------
fn print_ascii_logo() {
    // Embed the ASCII art at compile time
    const ASCII: &str = include_str!("ASCII.txt");

    // Use termcolor to reliably set color on Windows (Console API) and others
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let _ = stdout.set_color(ColorSpec::new().set_fg(Some(Color::Blue)).set_intense(true));
    let _ = writeln!(&mut stdout, "\n{}\n", ASCII);
    let _ = stdout.reset();
}


// ---------------------------------------------------------------------------
// Configuration structure loaded from config.json
// ---------------------------------------------------------------------------
#[derive(Debug, serde::Deserialize, Clone)]
pub struct Config {
    pub midi: MidiConfig,
    pub osc: OscConfig,
    pub mqtt: MqttConfig,
    pub transpose: TransposeConfig,
    /// Enable verbose logging (e.g., per-note OSC send logs)
    #[serde(default)]
    pub debug: bool,
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct MidiConfig {
    pub input_port_name_substr: String,
    pub output_port_name_substr: String,
}

#[derive(Debug, serde::Deserialize, Clone)]
#[serde(default)]
pub struct OscConfig {
    pub listening_host: String,
    pub listening_port: u16,
    pub transpose_path: String,
    pub transpose_up_path: String,
    pub transpose_down_path: String,
    pub sending_addr: String,
    pub sending_port: u16,
    // Whether OSC sending of MIDI is enabled at startup
    pub sending_enabled: bool,
    // Whether to send original (true) or transposed (false) MIDI via OSC at startup
    pub send_original: bool,
}

impl Default for OscConfig {
    fn default() -> Self {
        OscConfig {
            listening_host: "127.0.0.1".to_string(),
            listening_port: 9069,
            transpose_path: "/transpose".to_string(),
            transpose_up_path: "/transposeUp".to_string(),
            transpose_down_path: "/transposeDown".to_string(),
            sending_addr: "127.0.0.1".to_string(),
            sending_port: 9000,
            sending_enabled: false,
            send_original: true,
        }
    }
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct MqttConfig {
    pub broker_host: String,
    pub broker_port: u16,
    pub base_topic: String,
    pub username: String,
    pub password: String,
    #[serde(default = "default_mqtt_enabled")]
    pub enabled: bool,
}

fn default_mqtt_enabled() -> bool { true }

#[derive(Debug, serde::Deserialize, Clone)]
pub struct TransposeConfig {
    pub min: i8,
    pub max: i8,
}

#[derive(Debug, Clone)]
pub struct MqttCredentials {
    pub username: String,
    pub password: String,
}

fn load_config() -> Config {
    let path = std::path::Path::new("config.json");
    
    // Default configuration if file doesn't exist
    let default_config = Config {
        midi: MidiConfig {
            input_port_name_substr: "MRCC".to_string(),
            output_port_name_substr: "MIDIOUT7 (MRCC)".to_string(),
        },
        osc: OscConfig {
            listening_host: "127.0.0.1".to_string(),
            listening_port: 9069,
            transpose_path: "/transpose".to_string(),
            transpose_up_path: "/transposeUp".to_string(),
            transpose_down_path: "/transposeDown".to_string(),
            sending_addr: "127.0.0.1".to_string(),
            sending_port: 9000,
            sending_enabled: false,
            send_original: true,
        },
        mqtt: MqttConfig {
            broker_host: "192.168.50.200".to_string(),
            broker_port: 1883,
            base_topic: "midi_transposer".to_string(),
            username: "".to_string(),
            password: "".to_string(),
            enabled: true,
        },
        transpose: TransposeConfig {
            min: -24,
            max: 24,
        },
        debug: false,
    };

    if !path.exists() {
        eprintln!("[CONFIG] config.json not found; using defaults");
        return default_config;
    }
    
    match std::fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<Config>(&text) {
            Ok(config) => {
                CONFIG_LOADED_FROM_FILE.store(true, Ordering::SeqCst);
                config
            },
            Err(err) => {
                eprintln!("[CONFIG] Failed to parse config.json: {} (using defaults)", err);
                default_config
            }
        },
        Err(err) => {
            eprintln!("[CONFIG] Failed to read config.json: {} (using defaults)", err);
            default_config
        }
    }
}

// ---------------------------------------------------------------------------
// Global runtime state (shared via atomics)
// ---------------------------------------------------------------------------
/// Current transpose amount in semitones. Updated by stdin handler thread.
static TRANSPOSE_SEMITONES: AtomicI32 = AtomicI32::new(0);

/// When true the main loop will terminate and the program will shut down.
static EXIT_FLAG: AtomicBool = AtomicBool::new(false);

/// Global configuration loaded at startup
static mut GLOBAL_CONFIG: Option<Config> = None;

/// Global debug flag (runtime-togglable). Initialized from config.debug.
pub(crate) static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

/// Whether config was successfully loaded from config.json (not defaults)
pub(crate) static CONFIG_LOADED_FROM_FILE: AtomicBool = AtomicBool::new(false);

/// Get the global configuration (must be loaded first)
pub fn get_config() -> &'static Config {
    unsafe {
        GLOBAL_CONFIG.as_ref().expect("Config not loaded")
    }
}

/// Check whether verbose debug logging is enabled
pub fn is_debug_enabled() -> bool {
    DEBUG_ENABLED.load(Ordering::SeqCst)
}

/// Sets the transpose value with range clamping
pub fn set_transpose_semitones(value: i32) -> i32 {
    let config = get_config();
    let clamped = value.clamp(config.transpose.min as i32, config.transpose.max as i32);
    TRANSPOSE_SEMITONES.store(clamped, Ordering::SeqCst);
    if value != clamped {
        eprintln!(
            "[TRANSPOSE] Clamped {} to range [{}, {}] -> {}",
            value, config.transpose.min, config.transpose.max, clamped
        );
    }
    clamped
}

/// Enable OSC sending of MIDI data (true = enabled, false = disabled)
static OSC_SENDING_ENABLED: AtomicBool = AtomicBool::new(false);

/// Send original input MIDI (true) or transposed MIDI (false) via OSC
pub static OSC_SEND_ORIGINAL: AtomicBool = AtomicBool::new(true);

/// MQTT enabled flag (runtime)
pub(crate) static MQTT_ENABLED: AtomicBool = AtomicBool::new(true);

/// MQTT connection state (set by mqtt_listener)
pub(crate) static MQTT_CONNECTED: AtomicBool = AtomicBool::new(false);

fn main() {
    match run() {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err),
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    // Show a nice splash logo at startup
    print_ascii_logo();

    // Load configuration first
    let config = load_config();
    
    // Store config in global static for other modules to access
    unsafe {
        GLOBAL_CONFIG = Some(config.clone());
    }
    // Initialize runtime debug flag from config
    DEBUG_ENABLED.store(config.debug, Ordering::SeqCst);
    // Inform about config source when debug is enabled
    if is_debug_enabled() && CONFIG_LOADED_FROM_FILE.load(Ordering::SeqCst) {
        println!("[CONFIG] Loaded configuration from config.json");
    }

    let mut midi_in = MidiInput::new("midir reading input")?;
    midi_in.ignore(Ignore::None);

    let midi_out = MidiOutput::new("midir forwarding output")?;

    // Choose input port by substring match (first match). Falls back to explicit selection if none/multiple found.
    // Choose input port (substring or interactive selection)
    let input_index = input::choose_input_port(&midi_in, &config.midi.input_port_name_substr)?;
    let in_ports = midi_in.ports();
    let in_port = &in_ports[input_index];

    if is_debug_enabled() { println!("\nOpening input connection"); }
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
    let output_index = output::choose_output_port(&midi_out, &config.midi.output_port_name_substr, &in_port_name)?;
    let out_ports = midi_out.ports();
    let out_port = &out_ports[output_index];

    // Resolve output port name before connecting (connect takes ownership of midi_out)
    let out_port_name = midi_out.port_name(out_port)?;
    // Use default initial transpose 0 so forwarding starts immediately.
    // The spawned stdin handler thread still accepts numbers to change transpose later.
    let initial_transpose: i32 = 0;
    // Initialize OSC-related atomics from configuration
    OSC_SENDING_ENABLED.store(config.osc.sending_enabled, Ordering::SeqCst);
    OSC_SEND_ORIGINAL.store(config.osc.send_original, Ordering::SeqCst);

    if is_debug_enabled() {
        println!("Using initial transpose: {} semitones", initial_transpose);
        println!("OSC sending: {} (to {}:{})", 
            if OSC_SENDING_ENABLED.load(Ordering::SeqCst) { "enabled" } else { "disabled" },
            config.osc.sending_addr, config.osc.sending_port);
        println!("OSC sending mode: {}", if OSC_SEND_ORIGINAL.load(Ordering::SeqCst) { "original" } else { "transposed" });
    }

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

    if is_debug_enabled() {
        println!(
            "Connection open, forwarding from '{}' -> '{}' (type number+Enter to change transpose, empty line or 'exit' to quit)...",
            in_port_name,
            out_port_name
        );
    }

    // Spawn forwarder thread (owns the output connection and applies transpose)
    let forward_handle = forwarder::spawn_forwarder(conn_out, rx, Some(osc_transposed_tx));

    // Spawn stdin handler (updates TRANSPOSE_SEMITONES and EXIT_FLAG)
    let stdin_handle = stdin_handler::spawn_stdin_handler();

    // Spawn OSC listener on UDP port 9069 (updates TRANSPOSE_SEMITONES on /transpose)
    let osc_handle = osc_listener::spawn_osc_listener();

    // Initialize MQTT enabled flag from config
    MQTT_ENABLED.store(config.mqtt.enabled, Ordering::SeqCst);

    // Spawn MQTT listener only if enabled
    let mqtt_handle = if MQTT_ENABLED.load(Ordering::SeqCst) {
        Some(mqtt_listener::spawn_mqtt_listener())
    } else {
        None
    };

    // Spawn OSC sender threads for both original and transposed MIDI
    let osc_target_addr = format!("{}:{}", config.osc.sending_addr, config.osc.sending_port);
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

    // After all services are up, print final status once (ensures other debug logs appear before)
    crate::general::check::print_final_status_after_startup();

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
    if let Some(h) = mqtt_handle { let _ = h.join(); }

    Ok(())
}
