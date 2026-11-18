//! MIDI port discovery and matching a saved port reference to a present port.

use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

use super::CLIENT_NAME;
use crate::project::PortRef;
use crate::state::PortInfo;

fn is_own(name: &str) -> bool {
    name.contains(CLIENT_NAME)
}

/// ALSA appends ` client:port` numbers that change when a device is replugged.
fn strip_alsa_suffix(name: &str) -> &str {
    if let Some((head, tail)) = name.rsplit_once(' ') {
        let mut parts = tail.split(':');
        let numeric =
            |p: Option<&str>| p.is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()));
        if numeric(parts.next()) && numeric(parts.next()) && parts.next().is_none() {
            return head;
        }
    }
    name
}

/// Find the port a reference points to: by id, then exact name, then name
/// without the ALSA numbers, then name as a substring (how imported
/// `config.json` files match). `exclude_name` is skipped in the substring
/// stage so an output pattern does not pick the input's own port.
pub fn resolve(ports: &[PortInfo], want: &PortRef, exclude_name: Option<&str>) -> Option<usize> {
    if !want.id.is_empty() {
        if let Some(i) = ports.iter().position(|p| p.id == want.id) {
            return Some(i);
        }
    }
    if want.name.is_empty() {
        return None;
    }
    if let Some(i) = ports.iter().position(|p| p.name == want.name) {
        return Some(i);
    }
    let stripped = strip_alsa_suffix(&want.name);
    if let Some(i) = ports.iter().position(|p| strip_alsa_suffix(&p.name) == stripped) {
        return Some(i);
    }
    ports
        .iter()
        .position(|p| p.name.contains(&want.name) && Some(p.name.as_str()) != exclude_name)
        .or_else(|| ports.iter().position(|p| p.name.contains(&want.name)))
}

/// Turn the driver's `(id, name)` pairs into port infos with unique ids.
///
/// The driver id alone is not unique: on Windows every port of a multi-port
/// interface reports the id of the device. The id used here is the driver id
/// plus the name, with a counter for ports that are identical in both.
fn unique_ports(raw: Vec<(String, String)>) -> Vec<PortInfo> {
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    raw.into_iter()
        .map(|(driver_id, name)| {
            let base = format!("{driver_id}|{name}");
            let count = seen.entry(base.clone()).or_insert(0);
            *count += 1;
            let id = if *count == 1 {
                base
            } else {
                format!("{base}#{count}")
            };
            PortInfo { id, name }
        })
        .collect()
}

fn input_ports(midi_in: &MidiInput) -> Vec<(PortInfo, midir::MidiInputPort)> {
    let ports = midi_in.ports();
    let raw = ports
        .iter()
        .map(|p| (p.id(), midi_in.port_name(p).unwrap_or_default()))
        .collect();
    unique_ports(raw)
        .into_iter()
        .zip(ports)
        .filter(|(info, _)| !info.name.is_empty() && !is_own(&info.name))
        .collect()
}

fn output_ports(midi_out: &MidiOutput) -> Vec<(PortInfo, midir::MidiOutputPort)> {
    let ports = midi_out.ports();
    let raw = ports
        .iter()
        .map(|p| (p.id(), midi_out.port_name(p).unwrap_or_default()))
        .collect();
    unique_ports(raw)
        .into_iter()
        .zip(ports)
        .filter(|(info, _)| !info.name.is_empty() && !is_own(&info.name))
        .collect()
}

/// The entry for `wanted` in a fresh port list: same id, else same name.
fn find_port<P>(mut ports: Vec<(PortInfo, P)>, wanted: &PortInfo) -> Option<P> {
    let index = ports
        .iter()
        .position(|(info, _)| info.id == wanted.id)
        .or_else(|| ports.iter().position(|(info, _)| info.name == wanted.name))?;
    Some(ports.swap_remove(index).1)
}

/// Long-lived clients used only to list ports.
#[derive(Default)]
pub struct Scanner {
    input: Option<MidiInput>,
    output: Option<MidiOutput>,
}

impl Scanner {
    /// List the present ports. `Err` when the system has no usable MIDI backend.
    pub fn scan(&mut self) -> Result<(Vec<PortInfo>, Vec<PortInfo>), String> {
        if self.input.is_none() {
            self.input = Some(MidiInput::new(&format!("{CLIENT_NAME} scan")).map_err(|e| e.to_string())?);
        }
        if self.output.is_none() {
            self.output = Some(MidiOutput::new(&format!("{CLIENT_NAME} scan")).map_err(|e| e.to_string())?);
        }
        let (Some(midi_in), Some(midi_out)) = (&self.input, &self.output) else {
            return Err("MIDI backend unavailable".into());
        };
        let inputs = input_ports(midi_in).into_iter().map(|(info, _)| info).collect();
        let outputs = output_ports(midi_out).into_iter().map(|(info, _)| info).collect();
        Ok((inputs, outputs))
    }
}

/// Open an input port. `on_message` runs on the driver's thread: it must only
/// hand the bytes on, never block or call back into MIDI.
pub fn open_input<F>(port: &PortInfo, mut on_message: F) -> Result<MidiInputConnection<()>, String>
where
    F: FnMut(&[u8]) + Send + 'static,
{
    let mut midi_in = MidiInput::new(CLIENT_NAME).map_err(|e| e.to_string())?;
    // Forward everything, including SysEx, clock and active sensing.
    midi_in.ignore(Ignore::None);
    let target = find_port(input_ports(&midi_in), port)
        .ok_or_else(|| format!("MIDI input '{}' is no longer available", port.name))?;
    midi_in
        .connect(&target, "input", move |_stamp, bytes, _| on_message(bytes), ())
        .map_err(|e| e.to_string())
}

pub fn open_output(port: &PortInfo) -> Result<MidiOutputConnection, String> {
    let midi_out = MidiOutput::new(CLIENT_NAME).map_err(|e| e.to_string())?;
    let target = find_port(output_ports(&midi_out), port)
        .ok_or_else(|| format!("MIDI output '{}' is no longer available", port.name))?;
    midi_out.connect(&target, "output").map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port(id: &str, name: &str) -> PortInfo {
        PortInfo {
            id: id.into(),
            name: name.into(),
        }
    }

    fn want(id: &str, name: &str) -> PortRef {
        PortRef {
            id: id.into(),
            name: name.into(),
        }
    }

    #[test]
    fn id_wins_over_name() {
        let ports = [port("a", "Piano"), port("b", "Piano")];
        assert_eq!(resolve(&ports, &want("b", "Piano"), None), Some(1));
    }

    #[test]
    fn falls_back_to_exact_name_when_the_id_changed() {
        let ports = [port("x1", "Synth"), port("x2", "Piano")];
        assert_eq!(resolve(&ports, &want("gone", "Piano"), None), Some(1));
    }

    #[test]
    fn alsa_numbers_are_ignored() {
        let ports = [port("32:0", "MRCC 32:0"), port("14:0", "Midi Through 14:0")];
        assert_eq!(resolve(&ports, &want("28:0", "MRCC 28:0"), None), Some(0));
        assert_eq!(strip_alsa_suffix("Name with 3 words"), "Name with 3 words");
        assert_eq!(strip_alsa_suffix("MRCC 28:0"), "MRCC");
    }

    #[test]
    fn substring_match_for_imported_configs() {
        let ports = [port("1", "MIDIIN7 (MRCC)"), port("2", "MRCC")];
        assert_eq!(
            resolve(&ports, &want("", "MRCC"), None),
            Some(1),
            "exact name first"
        );
        let outs = [port("1", "MRCC"), port("2", "MIDIOUT7 (MRCC)")];
        assert_eq!(
            resolve(&outs, &want("", "MIDIOUT7 (MRCC)"), Some("MRCC")),
            Some(1)
        );
        // Pattern matches both; the port named like the input is skipped.
        let outs = [port("1", "MRCC"), port("2", "MIDIOUT2 (MRCC)")];
        assert_eq!(resolve(&outs, &want("", "MRC"), Some("MRCC")), Some(1));
        // ...unless it is the only match.
        let outs = [port("1", "MRCC")];
        assert_eq!(resolve(&outs, &want("", "MRC"), Some("MRCC")), Some(0));
    }

    #[test]
    fn ports_sharing_a_driver_id_get_unique_ids() {
        // Windows: every port of a multi-port interface reports the device id.
        let ports = unique_ports(vec![
            ("dev".into(), "MRCC".into()),
            ("dev".into(), "MIDIIN2 (MRCC)".into()),
            ("dev".into(), "MIDIIN3 (MRCC)".into()),
            ("x".into(), "Twin".into()),
            ("x".into(), "Twin".into()),
        ]);
        let ids: std::collections::HashSet<_> = ports.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids.len(), 5, "{ports:?}");
        // A saved reference to the third port resolves to the third port.
        let saved = PortRef {
            id: ports[2].id.clone(),
            name: ports[2].name.clone(),
        };
        assert_eq!(resolve(&ports, &saved, None), Some(2));
        // The id is stable between scans.
        let again = unique_ports(vec![("dev".into(), "MIDIIN3 (MRCC)".into())]);
        assert_eq!(again[0].id, ports[2].id);
    }

    #[test]
    fn nothing_matches() {
        let ports = [port("1", "Piano")];
        assert_eq!(resolve(&ports, &want("", "Organ"), None), None);
        assert_eq!(resolve(&ports, &want("", ""), None), None);
        assert_eq!(resolve(&[], &want("1", "Piano"), None), None);
    }
}
