//! Console command parser, shared by the GUI console and headless mode.
//!
//! The accepted spellings are part of the public contract; see the
//! `transposer-protocols` skill before changing them.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Blank line. Headless mode treats it as exit, the GUI ignores it.
    Empty,
    Exit,
    Debug(bool),
    /// OSC note sending on/off.
    Osc(bool),
    /// `true`: send original (input) notes, `false`: transposed.
    SendOriginal(bool),
    MidiList,
    Help,
    Mqtt(bool),
    Transpose(i32),
    Unknown(String),
}

pub const HELP: &[&str] = &[
    "Commands:",
    "  <number>         - Set transpose in semitones",
    "  osc on/enable    - Enable OSC sending",
    "  osc off/disable  - Disable OSC sending",
    "  osc original     - Send original input MIDI via OSC",
    "  osc transposed   - Send transposed MIDI via OSC",
    "  midi list        - Show available MIDI input/output devices",
    "  mqtt on/off      - Enable/Disable MQTT for this session",
    "  debug on/off     - Enable/Disable verbose debug prints",
    "  help/h           - Show this help",
    "  exit/quit/q      - Exit program",
];

fn on_off(word: &str) -> Option<bool> {
    match word {
        "1" | "on" | "enable" => Some(true),
        "0" | "off" | "disable" => Some(false),
        _ => None,
    }
}

pub fn parse(line: &str) -> Command {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Command::Empty;
    }
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "exit" | "quit" | "q" => return Command::Exit,
        "debug on" | "debug enable" => return Command::Debug(true),
        "debug off" | "debug disable" => return Command::Debug(false),
        "osc on" | "osc enable" => return Command::Osc(true),
        "osc off" | "osc disable" => return Command::Osc(false),
        "osc original" | "osc input" | "osc_original" => return Command::SendOriginal(true),
        "osc transposed" | "osc output" | "osc_transposed" => return Command::SendOriginal(false),
        "midi list" | "midi devices" | "list midi" => return Command::MidiList,
        "help" | "h" => return Command::Help,
        "mqtt on" | "mqtt enable" => return Command::Mqtt(true),
        "mqtt off" | "mqtt disable" => return Command::Mqtt(false),
        _ => {}
    }
    // Flag forms: `osc_original 1`, `osc_original:0`, `osc_original on`.
    if let Some(rest) = lower
        .strip_prefix("osc_original ")
        .or_else(|| lower.strip_prefix("osc_original:"))
    {
        if let Some(on) = on_off(rest.trim()) {
            return Command::SendOriginal(on);
        }
    }
    match trimmed.parse::<i32>() {
        Ok(v) => Command::Transpose(v),
        Err(_) => Command::Unknown(trimmed.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_documented_spelling() {
        let cases: &[(&str, Command)] = &[
            ("", Command::Empty),
            ("   ", Command::Empty),
            ("exit", Command::Exit),
            ("QUIT", Command::Exit),
            ("q", Command::Exit),
            ("debug on", Command::Debug(true)),
            ("debug enable", Command::Debug(true)),
            ("Debug Off", Command::Debug(false)),
            ("debug disable", Command::Debug(false)),
            ("osc on", Command::Osc(true)),
            ("osc enable", Command::Osc(true)),
            ("osc off", Command::Osc(false)),
            ("osc disable", Command::Osc(false)),
            ("osc original", Command::SendOriginal(true)),
            ("osc input", Command::SendOriginal(true)),
            ("osc_original", Command::SendOriginal(true)),
            ("osc transposed", Command::SendOriginal(false)),
            ("osc output", Command::SendOriginal(false)),
            ("osc_transposed", Command::SendOriginal(false)),
            ("osc_original 1", Command::SendOriginal(true)),
            ("osc_original:1", Command::SendOriginal(true)),
            ("osc_original on", Command::SendOriginal(true)),
            ("osc_original enable", Command::SendOriginal(true)),
            ("osc_original 0", Command::SendOriginal(false)),
            ("osc_original:0", Command::SendOriginal(false)),
            ("osc_original off", Command::SendOriginal(false)),
            ("osc_original disable", Command::SendOriginal(false)),
            ("midi list", Command::MidiList),
            ("midi devices", Command::MidiList),
            ("list midi", Command::MidiList),
            ("help", Command::Help),
            ("h", Command::Help),
            ("mqtt on", Command::Mqtt(true)),
            ("mqtt enable", Command::Mqtt(true)),
            ("mqtt off", Command::Mqtt(false)),
            ("mqtt disable", Command::Mqtt(false)),
            ("5", Command::Transpose(5)),
            ("-12", Command::Transpose(-12)),
            ("  +3 ", Command::Transpose(3)),
            ("0", Command::Transpose(0)),
            ("1", Command::Transpose(1)),
        ];
        for (input, expected) in cases {
            assert_eq!(&parse(input), expected, "input {input:?}");
        }
    }

    #[test]
    fn unknown_input_is_reported_verbatim() {
        assert_eq!(parse("Foo Bar"), Command::Unknown("Foo Bar".into()));
        assert_eq!(
            parse("osc_original maybe"),
            Command::Unknown("osc_original maybe".into())
        );
        assert_eq!(parse("1.5"), Command::Unknown("1.5".into()));
    }
}
