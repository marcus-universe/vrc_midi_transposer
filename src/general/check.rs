use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use std::io::Write;

// Connection status flags
pub static OSC_LISTENER_RUNNING: AtomicBool = AtomicBool::new(false);
static OSC_SENDER_COUNT: AtomicI32 = AtomicI32::new(0);
static BANNER_PRINTED: AtomicBool = AtomicBool::new(false);

pub fn mark_osc_sender_started() {
    OSC_SENDER_COUNT.fetch_add(1, Ordering::SeqCst);
}

pub fn mark_osc_sender_stopped() {
    OSC_SENDER_COUNT.fetch_sub(1, Ordering::SeqCst);
}

pub fn is_osc_sender_running() -> bool {
    OSC_SENDER_COUNT.load(Ordering::SeqCst) > 0
}

// Print the quick help line in blue (works on Windows CMD via termcolor)
pub fn print_quick_help() {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let _ = stdout.set_color(ColorSpec::new().set_fg(Some(Color::Blue)).set_intense(true));
    let _ = writeln!(&mut stdout, "Type 'help' for commands, 'exit' to quit");
    let _ = stdout.reset();
}

pub fn print_connections_active() {
    // Ensure we only print one banner overall
    if BANNER_PRINTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let _ = stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green)).set_intense(true));
    let _ = writeln!(&mut stdout, "Connections active | Program started");
    let _ = stdout.reset();
    print_quick_help();
}

pub fn print_connections_broken() {
    if BANNER_PRINTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let _ = stdout.set_color(ColorSpec::new().set_fg(Some(Color::Red)).set_intense(true));
    let _ = writeln!(&mut stdout, "Connections broken | Program tries reconnecting");
    let _ = stdout.reset();
}

/// Call once after startup to print a single final status line after other debug logs.
pub fn print_final_status_after_startup() {
    // Small delay so other services can print their initial logs first
    std::thread::sleep(std::time::Duration::from_millis(300));

    let mqtt_enabled = crate::MQTT_ENABLED.load(Ordering::SeqCst);
    let osc_listener = OSC_LISTENER_RUNNING.load(Ordering::SeqCst);
    let osc_sender = is_osc_sender_running();

    if mqtt_enabled {
        // With MQTT enabled, do not print here at all to avoid double banners.
        // mqtt_listener will print the final banner after setup (or on error).
        return;
    } else {
        // No MQTT involvement: decide based on OSC only
        if osc_listener && osc_sender {
            print_connections_active();
        } else {
            print_connections_broken();
        }
    }
}
