//! Identity of the application, embedded in every binary.

pub const APP_NAME: &str = "VRC MIDI Transposer";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const OWNER: &str = "Marcus Universe";
pub const EMAIL: &str = "contact@marcus-universe.de";
pub const LICENSE: &str = "MIT";
pub const REPOSITORY: &str = "https://github.com/marcus-universe/vrc_midi_transposer";
/// Reverse-domain identifier; also the name of the data directory.
pub const IDENTIFIER: &str = "de.marcus-universe.vrc-midi-transposer";
/// ASCII art shown at the top of the console.
pub const BANNER: &str = include_str!("ASCII.txt");

/// Multi-line text for `--version` and the About panel.
pub fn version_text() -> String {
    format!(
        "{APP_NAME} {VERSION}\nOwner:   {OWNER}\nE-Mail:  {EMAIL}\nLicense: {LICENSE}\nGitHub:  {REPOSITORY}"
    )
}
