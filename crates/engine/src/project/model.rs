//! The project file: everything the user configures. See the
//! `transposer-protocols` skill for the schema rules.

use serde::{Deserialize, Serialize};

use super::pattern::PatternSettings;

pub const SCHEMA: u32 = 1;

/// Reference to a MIDI port. Resolved by id, then exact name, then substring.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PortRef {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OscRole {
    Transpose,
    TransposeUp,
    TransposeDown,
    DebugSet,
    MidiList,
    OscSendOriginal,
    OscSendTransposed,
}

impl OscRole {
    /// Every role with its default path. New projects start with exactly these.
    pub const DEFAULTS: [(OscRole, &'static str); 7] = [
        (OscRole::Transpose, "/transpose"),
        (OscRole::TransposeUp, "/transposeUp"),
        (OscRole::TransposeDown, "/transposeDown"),
        (OscRole::DebugSet, "/debug"),
        (OscRole::MidiList, "/midi/list"),
        (OscRole::OscSendOriginal, "/osc/original"),
        (OscRole::OscSendTransposed, "/osc/transposed"),
    ];
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListenAddress {
    pub addr: String,
    /// Entries without a role are kept but do nothing.
    #[serde(default)]
    pub role: Option<OscRole>,
}

pub fn default_listening_addresses() -> Vec<ListenAddress> {
    OscRole::DEFAULTS
        .iter()
        .map(|(role, addr)| ListenAddress {
            addr: (*addr).to_string(),
            role: Some(*role),
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValueType {
    Bool,
    Float,
}

/// Which MIDI message triggers a custom control.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerKind {
    /// Control Change; the value moves a slider, or switches at 64.
    Cc,
    /// A key; toggles, or holds the control while pressed.
    Note,
    /// Program Change; toggles.
    Program,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerMode {
    /// Each Note On flips the control.
    #[default]
    Toggle,
    /// On while the key is held (Note triggers only).
    Momentary,
}

/// A MIDI message that sets a custom control.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiTrigger {
    pub kind: TriggerKind,
    /// 1-16, or any channel when `None`.
    #[serde(default)]
    pub channel: Option<u8>,
    /// CC number, note number or program number, 0-127.
    pub number: u8,
    #[serde(default)]
    pub mode: TriggerMode,
    /// Also send the message on to the MIDI output. Off: the message only controls.
    #[serde(default)]
    pub forward: bool,
}

/// A custom control: an OSC address whose value can be set from the UI, MQTT or MIDI.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SendAddress {
    pub name: String,
    pub addr: String,
    #[serde(rename = "type")]
    pub ty: ValueType,
    /// For bool: 0 or 1.
    #[serde(default)]
    pub default: f32,
    #[serde(default)]
    pub min: Option<f32>,
    #[serde(default)]
    pub max: Option<f32>,
    /// MIDI message that sets this control.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midi: Option<MidiTrigger>,
}

impl SendAddress {
    /// Lowest and highest value, with the defaults a slider uses (0 to 1).
    pub fn range(&self) -> (f32, f32) {
        match self.ty {
            ValueType::Bool => (0.0, 1.0),
            ValueType::Float => (self.min.unwrap_or(0.0), self.max.unwrap_or(1.0)),
        }
    }

    /// Normalise a value for this control: 0/1 for bool, min/max clamp for float.
    pub fn clamp(&self, value: f32) -> f32 {
        match self.ty {
            ValueType::Bool => {
                if value != 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
            ValueType::Float => {
                let mut v = if value.is_finite() { value } else { self.default };
                if let Some(min) = self.min {
                    v = v.max(min);
                }
                if let Some(max) = self.max {
                    v = v.min(max);
                }
                v
            }
        }
    }

    pub fn initial(&self) -> f32 {
        self.clamp(self.default)
    }

    /// Topic and entity id fragment. Part of the MQTT contract.
    pub fn slug(&self) -> String {
        self.name.to_lowercase().replace(' ', "_")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MidiSettings {
    pub input: Option<PortRef>,
    pub output: Option<PortRef>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OscSettings {
    pub listen_host: String,
    pub listen_port: u16,
    pub target_host: String,
    pub target_port: u16,
    /// Applied when the project is opened.
    pub sending_enabled: bool,
    /// Applied when the project is opened.
    pub send_original: bool,
    pub listening_addresses: Vec<ListenAddress>,
    pub sending_addresses: Vec<SendAddress>,
}

impl Default for OscSettings {
    fn default() -> Self {
        OscSettings {
            listen_host: "127.0.0.1".into(),
            listen_port: 9069,
            target_host: "127.0.0.1".into(),
            target_port: 9000,
            sending_enabled: false,
            send_original: true,
            listening_addresses: default_listening_addresses(),
            sending_addresses: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MqttSettings {
    pub enabled: bool,
    pub broker_host: String,
    pub broker_port: u16,
    pub base_topic: String,
    pub username: String,
    pub password: String,
}

impl Default for MqttSettings {
    fn default() -> Self {
        MqttSettings {
            enabled: false,
            broker_host: "127.0.0.1".into(),
            broker_port: 1883,
            base_topic: "midi_transposer".into(),
            username: String::new(),
            password: String::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TransposeRange {
    pub min: i32,
    pub max: i32,
}

impl Default for TransposeRange {
    fn default() -> Self {
        TransposeRange { min: -24, max: 24 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub midi: MidiSettings,
    pub osc: OscSettings,
    pub mqtt: MqttSettings,
    pub transpose: TransposeRange,
    /// Applied when the project is opened.
    pub debug: bool,
    /// Arpeggiator / sequencer and their clock.
    pub pattern: PatternSettings,
}

impl Default for Project {
    fn default() -> Self {
        Project {
            schema: SCHEMA,
            id: String::new(),
            name: "Untitled".into(),
            midi: MidiSettings::default(),
            osc: OscSettings::default(),
            mqtt: MqttSettings::default(),
            transpose: TransposeRange::default(),
            debug: false,
            pattern: PatternSettings::default(),
        }
    }
}

impl Project {
    pub fn named(name: &str) -> Self {
        Project {
            name: name.trim().to_string(),
            ..Project::default()
        }
    }

    /// Repair values that would break the engine. Called on everything that
    /// comes from a file or the UI.
    pub fn normalized(mut self) -> Self {
        self.schema = SCHEMA;
        self.pattern.normalize();
        if self.name.trim().is_empty() {
            self.name = "Untitled".into();
        }
        let range = &mut self.transpose;
        range.min = range.min.clamp(-127, 127);
        range.max = range.max.clamp(-127, 127);
        if range.min > range.max {
            std::mem::swap(&mut range.min, &mut range.max);
        }
        for port in [&mut self.midi.input, &mut self.midi.output] {
            if port
                .as_ref()
                .is_some_and(|p| p.id.is_empty() && p.name.is_empty())
            {
                *port = None;
            }
        }
        for item in &mut self.osc.sending_addresses {
            if let (Some(min), Some(max)) = (item.min, item.max) {
                if min > max {
                    item.min = Some(max);
                    item.max = Some(min);
                }
            }
            if let Some(trigger) = &mut item.midi {
                trigger.number &= 0x7F;
                trigger.channel = trigger.channel.filter(|c| (1..=16).contains(c));
                if trigger.kind != TriggerKind::Note {
                    trigger.mode = TriggerMode::Toggle;
                }
            }
        }
        self.mqtt.base_topic = self.mqtt.base_topic.trim().trim_matches('/').to_string();
        if self.mqtt.base_topic.is_empty() {
            self.mqtt.base_topic = "midi_transposer".into();
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_project_has_the_seven_default_roles() {
        let p = Project::named("Test");
        let roles: Vec<_> = p
            .osc
            .listening_addresses
            .iter()
            .map(|a| (a.role.unwrap(), a.addr.as_str()))
            .collect();
        assert_eq!(roles, OscRole::DEFAULTS.to_vec());
        assert_eq!(
            p.osc
                .listening_addresses
                .iter()
                .map(|a| a.addr.as_str())
                .collect::<Vec<_>>(),
            [
                "/transpose",
                "/transposeUp",
                "/transposeDown",
                "/debug",
                "/midi/list",
                "/osc/original",
                "/osc/transposed"
            ]
        );
    }

    #[test]
    fn missing_fields_take_defaults_and_unknown_fields_are_ignored() {
        let p: Project =
            serde_json::from_str(r#"{"name":"X","future":1,"osc":{"listen_port":9001}}"#).unwrap();
        assert_eq!(p.name, "X");
        assert_eq!(p.osc.listen_port, 9001);
        assert_eq!(p.osc.target_port, 9000);
        assert_eq!(p.osc.listening_addresses.len(), 7);
        assert_eq!(p.transpose, TransposeRange { min: -24, max: 24 });
        assert!(!p.mqtt.enabled);
    }

    #[test]
    fn round_trip() {
        let mut p = Project::named("Round");
        p.osc.sending_addresses.push(SendAddress {
            name: "Hue Shift".into(),
            addr: "/avatar/parameters/HUE".into(),
            ty: ValueType::Float,
            default: 0.25,
            min: Some(0.0),
            max: Some(1.0),
            midi: Some(MidiTrigger {
                kind: TriggerKind::Cc,
                channel: Some(2),
                number: 20,
                mode: TriggerMode::Toggle,
                forward: false,
            }),
        });
        p.midi.input = Some(PortRef {
            id: "x".into(),
            name: "MRCC".into(),
        });
        let text = serde_json::to_string_pretty(&p).unwrap();
        assert!(text.contains(r#""type": "float""#));
        assert!(text.contains(r#""role": "transpose_up""#));
        let back: Project = serde_json::from_str(&text).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn normalize_repairs_ranges() {
        let mut p = Project::named("  ");
        p.transpose = TransposeRange { min: 12, max: -300 };
        p.mqtt.base_topic = " /foo/ ".into();
        let p = p.normalized();
        assert_eq!(p.name, "Untitled");
        assert_eq!(p.transpose, TransposeRange { min: -127, max: 12 });
        assert_eq!(p.mqtt.base_topic, "foo");
    }

    #[test]
    fn control_values_are_clamped_and_slugged() {
        let f = SendAddress {
            name: "Hue Shift".into(),
            addr: "/a".into(),
            ty: ValueType::Float,
            default: 2.0,
            min: Some(0.0),
            max: Some(1.0),
            midi: None,
        };
        assert_eq!(f.initial(), 1.0);
        assert_eq!(f.clamp(-3.0), 0.0);
        assert_eq!(f.slug(), "hue_shift");
        let b = SendAddress {
            ty: ValueType::Bool,
            default: 1.0,
            min: None,
            max: None,
            ..f
        };
        assert_eq!(b.clamp(7.0), 1.0);
        assert_eq!(b.clamp(0.0), 0.0);
    }

    #[test]
    fn triggers_are_optional_and_normalised() {
        let p: Project = serde_json::from_str(
            r#"{"osc":{"sending_addresses":[
                {"name":"A","addr":"/a","type":"bool"},
                {"name":"B","addr":"/b","type":"float","midi":{"kind":"cc","channel":0,"number":200,"mode":"momentary"}}
            ]}}"#,
        )
        .unwrap();
        let p = p.normalized();
        assert_eq!(p.osc.sending_addresses[0].midi, None);
        let t = p.osc.sending_addresses[1].midi.unwrap();
        assert_eq!(
            (t.kind, t.channel, t.number, t.mode, t.forward),
            (TriggerKind::Cc, None, 72, TriggerMode::Toggle, false)
        );
        let text = serde_json::to_string(&p.osc.sending_addresses[0]).unwrap();
        assert!(!text.contains("midi"), "no trigger, no field: {text}");
    }
}
