//! Importer for the v1.x `config.json` format.

use serde::Deserialize;

use super::model::{
    default_listening_addresses, ListenAddress, MidiSettings, MqttSettings, OscRole, OscSettings, PortRef,
    Project, SendAddress, TransposeRange,
};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LegacyConfig {
    midi: LegacyMidi,
    osc: LegacyOsc,
    mqtt: LegacyMqtt,
    transpose: LegacyTranspose,
    debug: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LegacyMidi {
    input_port_name_substr: String,
    output_port_name_substr: String,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
struct LegacyOsc {
    listening_host: String,
    listening_port: u16,
    // Deprecated in v1.3: used only when no listening address carries the role.
    transpose_path: String,
    transpose_up_path: String,
    transpose_down_path: String,
    sending_addr: String,
    sending_port: u16,
    sending_enabled: bool,
    send_original: bool,
    listening_addresses: Vec<ListenAddress>,
    sending_addresses: Vec<SendAddress>,
}

impl Default for LegacyOsc {
    fn default() -> Self {
        LegacyOsc {
            listening_host: "127.0.0.1".into(),
            listening_port: 9069,
            transpose_path: "/transpose".into(),
            transpose_up_path: "/transposeUp".into(),
            transpose_down_path: "/transposeDown".into(),
            sending_addr: "127.0.0.1".into(),
            sending_port: 9000,
            sending_enabled: false,
            send_original: true,
            listening_addresses: default_listening_addresses(),
            sending_addresses: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
struct LegacyMqtt {
    enabled: bool,
    broker_host: String,
    broker_port: u16,
    base_topic: String,
    username: String,
    password: String,
}

impl Default for LegacyMqtt {
    fn default() -> Self {
        LegacyMqtt {
            // v1 enabled MQTT when the key was absent.
            enabled: true,
            broker_host: "127.0.0.1".into(),
            broker_port: 1883,
            base_topic: "midi_transposer".into(),
            username: String::new(),
            password: String::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
struct LegacyTranspose {
    min: i32,
    max: i32,
}

impl Default for LegacyTranspose {
    fn default() -> Self {
        LegacyTranspose { min: -24, max: 24 }
    }
}

fn port(substr: String) -> Option<PortRef> {
    let name = substr.trim().to_string();
    (!name.is_empty()).then_some(PortRef {
        id: String::new(),
        name,
    })
}

/// True if the JSON text looks like a v1 `config.json` rather than a project file.
pub fn looks_legacy(value: &serde_json::Value) -> bool {
    value.get("schema").is_none()
        && (value.pointer("/midi/input_port_name_substr").is_some()
            || value.pointer("/osc/listening_port").is_some()
            || value.pointer("/mqtt/broker_host").is_some())
}

/// Convert the text of a v1 `config.json` into a project (without id).
pub fn import(text: &str, name: &str) -> Result<Project, String> {
    let text = text.trim_start_matches('\u{feff}');
    let cfg: LegacyConfig =
        serde_json::from_str(text).map_err(|e| format!("Not a valid config.json: {e}"))?;

    let mut listening = cfg.osc.listening_addresses;
    for (role, path) in [
        (OscRole::Transpose, &cfg.osc.transpose_path),
        (OscRole::TransposeUp, &cfg.osc.transpose_up_path),
        (OscRole::TransposeDown, &cfg.osc.transpose_down_path),
    ] {
        if !listening.iter().any(|a| a.role == Some(role)) && !path.is_empty() {
            listening.push(ListenAddress {
                addr: path.clone(),
                role: Some(role),
            });
        }
    }

    let project = Project {
        name: name.to_string(),
        midi: MidiSettings {
            input: port(cfg.midi.input_port_name_substr),
            output: port(cfg.midi.output_port_name_substr),
        },
        osc: OscSettings {
            listen_host: cfg.osc.listening_host,
            listen_port: cfg.osc.listening_port,
            target_host: cfg.osc.sending_addr,
            target_port: cfg.osc.sending_port,
            sending_enabled: cfg.osc.sending_enabled,
            send_original: cfg.osc.send_original,
            listening_addresses: listening,
            sending_addresses: cfg.osc.sending_addresses,
        },
        mqtt: MqttSettings {
            enabled: cfg.mqtt.enabled,
            broker_host: cfg.mqtt.broker_host,
            broker_port: cfg.mqtt.broker_port,
            base_topic: cfg.mqtt.base_topic,
            username: cfg.mqtt.username,
            password: cfg.mqtt.password,
        },
        transpose: TransposeRange {
            min: cfg.transpose.min,
            max: cfg.transpose.max,
        },
        debug: cfg.debug,
        ..Project::default()
    };
    Ok(project.normalized())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::model::ValueType;

    const V13: &str = r#"{
        "debug": false,
        "midi": { "input_port_name_substr": "MRCC", "output_port_name_substr": "MIDIOUT7 (MRCC)" },
        "osc": {
            "listening_host": "127.0.0.1", "listening_port": 9069,
            "sending_addr": "127.0.0.1", "sending_port": 9000,
            "sending_enabled": false, "send_original": true,
            "listening_addresses": [
                { "addr": "/transpose", "role": "transpose" },
                { "addr": "/transposeUp", "role": "transpose_up" },
                { "addr": "/transposeDown", "role": "transpose_down" },
                { "addr": "/debug", "role": "debug_set" },
                { "addr": "/midi/list", "role": "midi_list" },
                { "addr": "/osc/original", "role": "osc_send_original" },
                { "addr": "/osc/transposed", "role": "osc_send_transposed" }
            ],
            "sending_addresses": [
                { "name": "Sunglasses Toggle", "addr": "/avatar/parameters/Glasses", "type": "bool", "default": 1 },
                { "name": "Hue Shift", "addr": "/avatar/parameters/HUE", "type": "float", "default": 0.0, "min": 0.0, "max": 1.0 }
            ]
        },
        "mqtt": { "enabled": true, "broker_host": "10.0.0.2", "broker_port": 1883, "base_topic": "midi_transposer", "username": "u", "password": "p" },
        "transpose": { "min": -24, "max": 24 }
    }"#;

    #[test]
    fn imports_a_full_v13_config() {
        let p = import(V13, "Imported").unwrap();
        assert_eq!(p.name, "Imported");
        assert_eq!(
            p.midi.input,
            Some(PortRef {
                id: String::new(),
                name: "MRCC".into()
            })
        );
        assert_eq!(p.midi.output.unwrap().name, "MIDIOUT7 (MRCC)");
        assert_eq!(p.osc.listening_addresses.len(), 7);
        assert_eq!(p.osc.sending_addresses.len(), 2);
        assert_eq!(p.osc.sending_addresses[0].ty, ValueType::Bool);
        assert_eq!(p.osc.sending_addresses[0].default, 1.0);
        assert_eq!(p.osc.sending_addresses[1].max, Some(1.0));
        assert!(p.mqtt.enabled);
        assert_eq!(
            (
                p.mqtt.broker_host.as_str(),
                p.mqtt.username.as_str(),
                p.mqtt.password.as_str()
            ),
            ("10.0.0.2", "u", "p")
        );
        assert_eq!((p.transpose.min, p.transpose.max), (-24, 24));
    }

    #[test]
    fn old_config_without_address_lists_gets_defaults() {
        // The schema documented for v1.0-v1.2.
        let text = "\u{feff}{ \"midi\": { \"input_port_name_substr\": \"Piano\", \"output_port_name_substr\": \"\" },
            \"osc\": { \"listening_port\": 9100, \"transpose_path\": \"/t\" },
            \"mqtt\": { \"broker_host\": \"h\", \"broker_port\": 1883, \"base_topic\": \"b\", \"username\": \"\", \"password\": \"\" },
            \"transpose\": { \"min\": -12, \"max\": 12 } }";
        let p = import(text, "Old").unwrap();
        assert_eq!(p.midi.output, None);
        assert_eq!(p.osc.listen_port, 9100);
        assert_eq!(p.osc.listening_addresses.len(), 7);
        assert!(p.mqtt.enabled, "v1 enabled MQTT by default");
        assert_eq!(p.transpose.max, 12);
    }

    #[test]
    fn deprecated_paths_fill_missing_roles() {
        let text = r#"{ "osc": { "transpose_path": "/t", "transpose_up_path": "/u", "transpose_down_path": "/d",
            "listening_addresses": [ { "addr": "/debug", "role": "debug_set" } ] } }"#;
        let p = import(text, "x").unwrap();
        let addrs: Vec<_> = p
            .osc
            .listening_addresses
            .iter()
            .map(|a| a.addr.as_str())
            .collect();
        assert_eq!(addrs, ["/debug", "/t", "/u", "/d"]);
    }

    #[test]
    fn rejects_garbage() {
        assert!(import("not json", "x").is_err());
    }

    #[test]
    fn detects_legacy_files() {
        let legacy: serde_json::Value = serde_json::from_str(V13).unwrap();
        assert!(looks_legacy(&legacy));
        let project = serde_json::to_value(Project::named("p")).unwrap();
        assert!(!looks_legacy(&project));
    }
}
