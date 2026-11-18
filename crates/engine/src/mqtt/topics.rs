//! MQTT topic names, payload parsing and Home Assistant discovery documents.
//! All of this is public contract; see the `transposer-protocols` skill.

use serde_json::{json, Value};

use crate::project::{Project, SendAddress, ValueType};

/// Fixed client id and Home Assistant device id.
pub const CLIENT_ID: &str = "vrc_midi_transposer";
/// Home Assistant node id; independent of the base topic.
pub const NODE_ID: &str = "midi_transposer";
pub const HA_STATUS: &str = "homeassistant/status";
pub const HA_OWN_DISCOVERY: &str = "homeassistant/+/midi_transposer/#";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Topics {
    pub transpose_set: String,
    pub transpose_up: String,
    pub transpose_down: String,
    pub transpose_state: String,
    pub availability: String,
    pub osc_sending_set: String,
    pub osc_sending_state: String,
    pub send_original_set: String,
    pub send_original_state: String,
    pub debug_set: String,
    pub debug_state: String,
    /// One per custom control, same order as the project.
    pub custom_set: Vec<String>,
    pub custom_state: Vec<String>,
}

impl Topics {
    pub fn new(base: &str, controls: &[SendAddress]) -> Self {
        Topics {
            transpose_set: format!("{base}/transpose"),
            transpose_up: format!("{base}/transposeUp"),
            transpose_down: format!("{base}/transposeDown"),
            transpose_state: format!("{base}/state/transpose"),
            availability: format!("{base}/availability"),
            osc_sending_set: format!("{base}/osc/sendingEnabled"),
            osc_sending_state: format!("{base}/state/osc/sendingEnabled"),
            send_original_set: format!("{base}/osc/sendOriginal"),
            send_original_state: format!("{base}/state/osc/sendOriginal"),
            debug_set: format!("{base}/debug/enabled"),
            debug_state: format!("{base}/state/debug/enabled"),
            custom_set: controls
                .iter()
                .map(|c| format!("{base}/osc/custom/{}/set", c.slug()))
                .collect(),
            custom_state: controls
                .iter()
                .map(|c| format!("{base}/state/osc/custom/{}", c.slug()))
                .collect(),
        }
    }

    /// Topics the client subscribes to.
    pub fn subscriptions(&self) -> Vec<&str> {
        let mut out = vec![
            self.transpose_set.as_str(),
            self.transpose_up.as_str(),
            self.transpose_down.as_str(),
            self.osc_sending_set.as_str(),
            self.send_original_set.as_str(),
            self.debug_set.as_str(),
        ];
        out.extend(self.custom_set.iter().map(String::as_str));
        out.push(HA_OWN_DISCOVERY);
        out.push(HA_STATUS);
        out
    }
}

/// Integer, or a float that is rounded.
pub fn parse_transpose(payload: &[u8]) -> Option<i32> {
    let s = std::str::from_utf8(payload).ok()?.trim();
    if let Ok(v) = s.parse::<i32>() {
        return Some(v);
    }
    s.parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
        .map(|v| v.round() as i32)
}

/// `1`, `true` or `on` (any case) is true; anything else is false.
pub fn parse_bool(payload: &[u8]) -> bool {
    let s = std::str::from_utf8(payload)
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    s == "1" || s == "true" || s == "on"
}

pub fn bool_payload(on: bool) -> &'static str {
    if on {
        "1"
    } else {
        "0"
    }
}

/// State payload for a custom control value.
pub fn custom_payload(control: &SendAddress, value: f32) -> String {
    match control.ty {
        ValueType::Bool => bool_payload(value != 0.0).to_string(),
        ValueType::Float => value.to_string(),
    }
}

/// Value a `…/set` payload asks for, already normalised for the control.
pub fn parse_custom(control: &SendAddress, payload: &[u8]) -> f32 {
    match control.ty {
        ValueType::Bool => parse_bool(payload) as u8 as f32,
        ValueType::Float => {
            let s = std::str::from_utf8(payload).unwrap_or("").trim();
            control.clamp(s.parse::<f32>().unwrap_or(control.default))
        }
    }
}

fn custom_component(control: &SendAddress) -> &'static str {
    match control.ty {
        ValueType::Bool => "switch",
        ValueType::Float => "number",
    }
}

pub fn custom_discovery_topic(control: &SendAddress) -> String {
    format!(
        "homeassistant/{}/{NODE_ID}/custom_{}/config",
        custom_component(control),
        control.slug()
    )
}

/// True for a discovery topic of one of our custom entities.
pub fn is_custom_discovery_topic(topic: &str) -> bool {
    topic.starts_with("homeassistant/") && topic.contains("/midi_transposer/") && topic.contains("/custom_")
}

fn device() -> Value {
    json!({
        "identifiers": [CLIENT_ID],
        "name": "VRC MIDI Transposer",
        "manufacturer": "Marcus Universe",
        "model": "VRC MIDI Transposer",
    })
}

fn switch(name: &str, id: &str, command: &str, state: &str, availability: &str) -> Value {
    json!({
        "name": name,
        "unique_id": format!("{CLIENT_ID}_{id}"),
        "command_topic": command,
        "state_topic": state,
        "payload_on": "1",
        "payload_off": "0",
        "state_on": "1",
        "state_off": "0",
        "availability_topic": availability,
        "device": device(),
    })
}

fn button(name: &str, id: &str, command: &str, availability: &str) -> Value {
    json!({
        "name": name,
        "unique_id": format!("{CLIENT_ID}_{id}"),
        "command_topic": command,
        "payload_press": "1",
        "availability_topic": availability,
        "device": device(),
    })
}

/// Every Home Assistant discovery document as `(topic, payload)`.
pub fn discovery(project: &Project, topics: &Topics) -> Vec<(String, String)> {
    let avail = topics.availability.as_str();
    let mut docs: Vec<(String, Value)> = vec![
        (
            format!("homeassistant/number/{NODE_ID}/transpose/config"),
            json!({
                "name": "MIDI Transpose",
                "unique_id": format!("{CLIENT_ID}_transpose"),
                "command_topic": topics.transpose_set,
                "state_topic": topics.transpose_state,
                "min": project.transpose.min,
                "max": project.transpose.max,
                "step": 1,
                "unit_of_measurement": "semitones",
                "availability_topic": avail,
                "device": device(),
            }),
        ),
        (
            format!("homeassistant/button/{NODE_ID}/transpose_up/config"),
            button("Transpose Up", "transpose_up", &topics.transpose_up, avail),
        ),
        (
            format!("homeassistant/button/{NODE_ID}/transpose_down/config"),
            button("Transpose Down", "transpose_down", &topics.transpose_down, avail),
        ),
        (
            format!("homeassistant/switch/{NODE_ID}/osc_sending_enabled/config"),
            switch(
                "OSC Sending Enabled",
                "osc_sending_enabled",
                &topics.osc_sending_set,
                &topics.osc_sending_state,
                avail,
            ),
        ),
        (
            format!("homeassistant/switch/{NODE_ID}/osc_send_original/config"),
            switch(
                "OSC Send Original",
                "osc_send_original",
                &topics.send_original_set,
                &topics.send_original_state,
                avail,
            ),
        ),
        (
            format!("homeassistant/switch/{NODE_ID}/debug_enabled/config"),
            switch(
                "Debug Enabled",
                "debug_enabled",
                &topics.debug_set,
                &topics.debug_state,
                avail,
            ),
        ),
    ];

    for (idx, control) in project.osc.sending_addresses.iter().enumerate() {
        let id = format!("custom_{}", control.slug());
        let doc = match control.ty {
            ValueType::Bool => switch(
                &control.name,
                &id,
                &topics.custom_set[idx],
                &topics.custom_state[idx],
                avail,
            ),
            ValueType::Float => {
                let mut doc = json!({
                    "name": control.name,
                    "unique_id": format!("{CLIENT_ID}_{id}"),
                    "command_topic": topics.custom_set[idx],
                    "state_topic": topics.custom_state[idx],
                    "step": 0.01,
                    "availability_topic": avail,
                    "device": device(),
                });
                if let Some(min) = control.min {
                    doc["min"] = json!(min);
                }
                if let Some(max) = control.max {
                    doc["max"] = json!(max);
                }
                doc
            }
        };
        docs.push((custom_discovery_topic(control), doc));
    }

    docs.into_iter()
        .map(|(topic, doc)| (topic, doc.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control(name: &str, ty: ValueType) -> SendAddress {
        SendAddress {
            name: name.into(),
            addr: "/a".into(),
            ty,
            default: 0.5,
            min: Some(0.0),
            max: Some(1.0),
        }
    }

    fn project() -> Project {
        let mut p = Project::named("t");
        p.osc.sending_addresses = vec![
            control("Sunglasses Toggle", ValueType::Bool),
            control("Hue Shift", ValueType::Float),
        ];
        p
    }

    #[test]
    fn topic_names_are_stable() {
        let p = project();
        let t = Topics::new("midi_transposer", &p.osc.sending_addresses);
        assert_eq!(t.transpose_set, "midi_transposer/transpose");
        assert_eq!(t.transpose_up, "midi_transposer/transposeUp");
        assert_eq!(t.transpose_down, "midi_transposer/transposeDown");
        assert_eq!(t.transpose_state, "midi_transposer/state/transpose");
        assert_eq!(t.availability, "midi_transposer/availability");
        assert_eq!(t.osc_sending_set, "midi_transposer/osc/sendingEnabled");
        assert_eq!(t.osc_sending_state, "midi_transposer/state/osc/sendingEnabled");
        assert_eq!(t.send_original_set, "midi_transposer/osc/sendOriginal");
        assert_eq!(t.send_original_state, "midi_transposer/state/osc/sendOriginal");
        assert_eq!(t.debug_set, "midi_transposer/debug/enabled");
        assert_eq!(t.debug_state, "midi_transposer/state/debug/enabled");
        assert_eq!(
            t.custom_set[0],
            "midi_transposer/osc/custom/sunglasses_toggle/set"
        );
        assert_eq!(t.custom_state[1], "midi_transposer/state/osc/custom/hue_shift");
        assert_eq!(t.subscriptions().len(), 6 + 2 + 2);
    }

    #[test]
    fn payload_parsing() {
        assert_eq!(parse_transpose(b" 5 "), Some(5));
        assert_eq!(parse_transpose(b"-3"), Some(-3));
        assert_eq!(parse_transpose(b"2.6"), Some(3));
        assert_eq!(parse_transpose(b"abc"), None);
        assert_eq!(parse_transpose(b"NaN"), None);
        for yes in ["1", "true", "ON", " On "] {
            assert!(parse_bool(yes.as_bytes()), "{yes}");
        }
        for no in ["0", "false", "off", "", "yes", "2"] {
            assert!(!parse_bool(no.as_bytes()), "{no}");
        }
        let f = control("f", ValueType::Float);
        assert_eq!(parse_custom(&f, b"0.25"), 0.25);
        assert_eq!(parse_custom(&f, b"7"), 1.0);
        assert_eq!(parse_custom(&f, b"junk"), 0.5, "falls back to the default");
        let b = control("b", ValueType::Bool);
        assert_eq!(parse_custom(&b, b"on"), 1.0);
        assert_eq!(parse_custom(&b, b"nope"), 0.0);
        assert_eq!(custom_payload(&b, 1.0), "1");
        assert_eq!(custom_payload(&f, 0.25), "0.25");
    }

    #[test]
    fn discovery_documents_keep_their_ids() {
        let p = project();
        let t = Topics::new("midi_transposer", &p.osc.sending_addresses);
        let docs = discovery(&p, &t);
        let topics: Vec<_> = docs.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            topics,
            [
                "homeassistant/number/midi_transposer/transpose/config",
                "homeassistant/button/midi_transposer/transpose_up/config",
                "homeassistant/button/midi_transposer/transpose_down/config",
                "homeassistant/switch/midi_transposer/osc_sending_enabled/config",
                "homeassistant/switch/midi_transposer/osc_send_original/config",
                "homeassistant/switch/midi_transposer/debug_enabled/config",
                "homeassistant/switch/midi_transposer/custom_sunglasses_toggle/config",
                "homeassistant/number/midi_transposer/custom_hue_shift/config",
            ]
        );
        let ids: Vec<String> = docs
            .iter()
            .map(|(_, d)| {
                serde_json::from_str::<Value>(d).unwrap()["unique_id"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(
            ids,
            [
                "vrc_midi_transposer_transpose",
                "vrc_midi_transposer_transpose_up",
                "vrc_midi_transposer_transpose_down",
                "vrc_midi_transposer_osc_sending_enabled",
                "vrc_midi_transposer_osc_send_original",
                "vrc_midi_transposer_debug_enabled",
                "vrc_midi_transposer_custom_sunglasses_toggle",
                "vrc_midi_transposer_custom_hue_shift",
            ]
        );
        let number: Value = serde_json::from_str(&docs[0].1).unwrap();
        assert_eq!(number["min"], -24);
        assert_eq!(number["max"], 24);
        assert_eq!(number["unit_of_measurement"], "semitones");
        assert_eq!(number["device"]["identifiers"][0], "vrc_midi_transposer");
        let hue: Value = serde_json::from_str(&docs[7].1).unwrap();
        assert_eq!(hue["step"], 0.01);
        assert_eq!(hue["max"], 1.0);
    }

    #[test]
    fn names_with_quotes_produce_valid_json() {
        let mut p = Project::named("t");
        p.osc.sending_addresses = vec![control(r#"My "special" \ control"#, ValueType::Bool)];
        let t = Topics::new("b", &p.osc.sending_addresses);
        for (_, doc) in discovery(&p, &t) {
            serde_json::from_str::<Value>(&doc).expect("valid JSON");
        }
    }

    #[test]
    fn recognises_own_custom_discovery_topics() {
        assert!(is_custom_discovery_topic(
            "homeassistant/switch/midi_transposer/custom_x/config"
        ));
        assert!(!is_custom_discovery_topic(
            "homeassistant/switch/midi_transposer/debug_enabled/config"
        ));
        assert!(!is_custom_discovery_topic(
            "homeassistant/switch/other/custom_x/config"
        ));
    }
}
