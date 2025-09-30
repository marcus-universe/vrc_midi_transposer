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
            if let Ok(v) = cmd.parse::<i32>() {
                crate::TRANSPOSE_SEMITONES.store(v, Ordering::SeqCst);
                println!("Transpose set to {}", v);
            } else {
                println!("Unrecognized command: '{}'. Enter a number to set transpose or empty/exit to quit.", cmd);
            }
        }
    })
}
