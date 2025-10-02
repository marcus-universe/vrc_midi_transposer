use rumqttc::{Client, Event, Incoming, LastWill, MqttOptions, QoS};
use std::thread;
use std::time::Duration;
use std::sync::atomic::Ordering;

// MQTT Configuration Constants
const CLIENT_ID: &str = "transposer2025";
const KEEP_ALIVE_SECS: u64 = 30;
const RECONNECT_DELAY_SECS: u64 = 1;
const LOOP_DELAY_MS: u64 = 50;
const QUEUE_SIZE: usize = 10;

// Home Assistant Discovery Constants
const DEVICE_ID: &str = "midi_transposer_transposer2025";
const DEVICE_NAME: &str = "MIDI Transposer 2025";
const DEVICE_MANUFACTURER: &str = "MidiTransposer";
const DEVICE_MODEL: &str = "MidiTransposer";

/// Struktur für MQTT Topics
struct MqttTopics {
    transpose_set: String,
    transpose_up: String,
    transpose_down: String,
    transpose_state: String,
    availability: String,
}

impl MqttTopics {
    fn new(base_topic: &str) -> Self {
        Self {
            transpose_set: format!("{}/transpose", base_topic),
            transpose_up: format!("{}/transposeUp", base_topic),
            transpose_down: format!("{}/transposeDown", base_topic),
            transpose_state: format!("{}/state/transpose", base_topic),
            availability: format!("{}/availability", base_topic),
        }
    }
}

/// Parst Payload für Transpose-Werte
/// Akzeptiert: Integers, Floats (gerundet) für absolute Werte
fn parse_transpose_payload(payload: &[u8]) -> Option<i32> {
    let s = std::str::from_utf8(payload).ok()?.trim();
    
    // Versuche Integer-Parsing
    if let Ok(v) = s.parse::<i32>() {
        return Some(v);
    }
    
    // Versuche Float-Parsing (gerundet)
    if let Ok(vf) = s.parse::<f32>() {
        return Some(vf.round() as i32);
    }
    
    None
}

/// Parst Boolean-ähnliche Payloads für Up/Down Befehle
fn parse_boolean_payload(payload: &[u8]) -> bool {
    let s = std::str::from_utf8(payload)
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    
    s == "1" || s == "true" || s == "on"
}

/// Erstellt Device JSON für Home Assistant Discovery
fn create_device_json() -> String {
    format!(
        r#"{{
  "identifiers": ["{}"],
  "name": "{}",
  "manufacturer": "{}",
  "model": "{}"
}}"#,
        DEVICE_ID, DEVICE_NAME, DEVICE_MANUFACTURER, DEVICE_MODEL
    )
}

/// Publiziert Home Assistant MQTT Discovery-Konfigurationen
fn publish_homeassistant_discovery(client: &Client, topics: &MqttTopics) {
    let device_json = create_device_json();

    // Number Entity für absoluten Transpose-Wert
    let number_config = format!(
        r#"{{
  "name": "MIDI Transpose",
  "unique_id": "{}_transpose",
  "command_topic": "{}",
  "state_topic": "{}",
  "min": {},
  "max": {},
  "step": 1,
  "unit_of_measurement": "semitones",
  "availability_topic": "{}",
  "device": {}
}}"#,
        CLIENT_ID,
        topics.transpose_set,
        topics.transpose_state,
        crate::get_config().transpose.min,
        crate::get_config().transpose.max,
        topics.availability,
        device_json
    );
    let _ = client.publish(
        "homeassistant/number/midi_transposer/transpose/config",
        QoS::AtLeastOnce,
        true,
        number_config,
    );

    // Button für Transpose Up
    let button_up_config = format!(
        r#"{{
  "name": "Transpose Up",
  "unique_id": "{}_transpose_up",
  "command_topic": "{}",
  "payload_press": "1",
  "availability_topic": "{}",
  "device": {}
}}"#,
        CLIENT_ID, topics.transpose_up, topics.availability, device_json
    );
    let _ = client.publish(
        "homeassistant/button/midi_transposer/transpose_up/config",
        QoS::AtLeastOnce,
        true,
        button_up_config,
    );

    // Button für Transpose Down
    let button_down_config = format!(
        r#"{{
  "name": "Transpose Down",
  "unique_id": "{}_transpose_down",
  "command_topic": "{}",
  "payload_press": "1",
  "availability_topic": "{}",
  "device": {}
}}"#,
        CLIENT_ID, topics.transpose_down, topics.availability, device_json
    );
    let _ = client.publish(
        "homeassistant/button/midi_transposer/transpose_down/config",
        QoS::AtLeastOnce,
        true,
        button_down_config,
    );

    println!("[MQTT] Home Assistant Discovery konfiguriert");
}

/// Erstellt MQTT-Optionen mit Konfiguration und Last Will Testament
fn create_mqtt_options(host: &str, port: u16, creds: &crate::MqttCredentials, availability_topic: &str) -> MqttOptions {
    let mut options = MqttOptions::new(CLIENT_ID, host, port);
    options.set_keep_alive(Duration::from_secs(KEEP_ALIVE_SECS));
    options.set_credentials(&creds.username, &creds.password);
    
    // Last Will Testament: Markiert Gerät als offline bei Verbindungsabbruch
    options.set_last_will(LastWill::new(
        availability_topic,
        "offline",
        QoS::AtLeastOnce,
        true,
    ));
    
    options
}

/// Abonniert alle benötigten MQTT-Topics
fn subscribe_to_topics(client: &Client, topics: &MqttTopics) -> Result<(), Box<dyn std::error::Error>> {
    client.subscribe(&topics.transpose_set, QoS::AtLeastOnce)?;
    client.subscribe(&topics.transpose_up, QoS::AtLeastOnce)?;
    client.subscribe(&topics.transpose_down, QoS::AtLeastOnce)?;
    
    println!(
        "[MQTT] Subscribed to topics: {}, {}, {}", 
        topics.transpose_set, topics.transpose_up, topics.transpose_down
    );
    
    Ok(())
}

/// Startet einen Hintergrund-Thread für MQTT-Kommunikation
/// 
/// Abonnierte Topics:
/// - `<base>/transpose` - Setzt absoluten Transpose-Wert (Integer)
/// - `<base>/transposeUp` - Erhöht Transpose um 1 (1/true/on)
/// - `<base>/transposeDown` - Verringert Transpose um 1 (1/true/on)
/// 
/// Publizierte Topics:
/// - `<base>/state/transpose` - Aktueller Transpose-Wert
/// - `<base>/availability` - Online/Offline Status
pub fn spawn_mqtt_listener() -> thread::JoinHandle<()> {
    let config = crate::get_config();
    let host = &config.mqtt.broker_host;
    let port = config.mqtt.broker_port;
    let base_topic = &config.mqtt.base_topic;
    let creds = crate::MqttCredentials {
        username: config.mqtt.username.clone(),
        password: config.mqtt.password.clone(),
    };

    thread::spawn(move || {
        let topics = MqttTopics::new(base_topic);
        let mqtt_options = create_mqtt_options(host, port, &creds, &topics.availability);
        let (client, connection) = Client::new(mqtt_options, QUEUE_SIZE);

        // Abonniere Topics
        if let Err(e) = subscribe_to_topics(&client, &topics) {
            eprintln!("[MQTT] Subscription failed: {}", e);
            return;
        }

        println!(
            "[MQTT] Connected to {}:{} as '{}' successfully",
            host, port, creds.username
        );

        // Publiziere Home Assistant MQTT Discovery-Konfiguration
        publish_homeassistant_discovery(&client, &topics);

        // Gerät als online markieren und initialen Zustand publizieren
        let _ = client.publish(&topics.availability, QoS::AtLeastOnce, true, "online");
        let initial_value = crate::TRANSPOSE_SEMITONES.load(Ordering::SeqCst).to_string();
        let _ = client.publish(&topics.transpose_state, QoS::AtLeastOnce, true, initial_value);

        // Hauptschleife für MQTT-Nachrichten
        run_mqtt_message_loop(connection, &client, &topics);
    })
}

/// Behandelt eingehende MQTT-Nachrichten und aktualisiert Transpose-Werte
fn handle_mqtt_message(
    client: &Client,
    topics: &MqttTopics,
    topic: &str,
    payload: &[u8],
) -> Option<i32> {
    if topic == topics.transpose_set {
        // Absoluter Transpose-Wert
        if let Some(value) = parse_transpose_payload(payload) {
            let clamped_value = crate::set_transpose_semitones(value);
            println!("[MQTT] Transpose auf {} gesetzt", clamped_value);
            let _ = client.publish(&topics.transpose_state, QoS::AtLeastOnce, true, clamped_value.to_string());
            return Some(clamped_value);
        } else {
            eprintln!("[MQTT] Ungültige /transpose Payload: {:?}", payload);
        }
    } else if topic == topics.transpose_up {
        // Transpose erhöhen
        if parse_boolean_payload(payload) {
            let current = crate::TRANSPOSE_SEMITONES.load(Ordering::SeqCst);
            let new_value = crate::set_transpose_semitones(current + 1);
            println!("[MQTT] Transpose HOCH: {} -> {}", current, new_value);
            let _ = client.publish(&topics.transpose_state, QoS::AtLeastOnce, true, new_value.to_string());
            return Some(new_value);
        }
    } else if topic == topics.transpose_down {
        // Transpose verringern
        if parse_boolean_payload(payload) {
            let current = crate::TRANSPOSE_SEMITONES.load(Ordering::SeqCst);
            let new_value = crate::set_transpose_semitones(current - 1);
            println!("[MQTT] Transpose RUNTER: {} -> {}", current, new_value);
            let _ = client.publish(&topics.transpose_state, QoS::AtLeastOnce, true, new_value.to_string());
            return Some(new_value);
        }
    }
    
    None
}

/// Hauptschleife für MQTT-Nachrichten-Verarbeitung
fn run_mqtt_message_loop(mut connection: rumqttc::Connection, client: &Client, topics: &MqttTopics) {
    let mut iter = connection.iter();
    let mut last_state_sent = crate::TRANSPOSE_SEMITONES.load(Ordering::SeqCst);

    loop {
        // Prüfe Exit-Flag
        if crate::EXIT_FLAG.load(Ordering::SeqCst) {
            println!("[MQTT] Beenden angefordert, stoppe Listener");
            break;
        }

        // Verarbeite nächste MQTT-Nachricht
        if let Some(result) = iter.next() {
            match result {
                Ok(Event::Incoming(Incoming::Publish(publish))) => {
                    let topic = publish.topic.as_str();
                    let payload = publish.payload.as_ref();
                    
                    if let Some(new_value) = handle_mqtt_message(client, topics, topic, payload) {
                        last_state_sent = new_value;
                    }
                }
                Ok(_) => {
                    // Andere Events ignorieren
                }
                Err(e) => {
                    eprintln!("[MQTT] Verbindungsfehler: {} (Wiederverbindung in {}s)", e, RECONNECT_DELAY_SECS);
                    thread::sleep(Duration::from_secs(RECONNECT_DELAY_SECS));
                }
            }
        } else {
            eprintln!("[MQTT] Verbindungs-Iterator beendet");
            break;
        }

        // Publiziere Zustandsänderung von anderen Quellen (stdin/OSC)
        let current_value = crate::TRANSPOSE_SEMITONES.load(Ordering::SeqCst);
        if current_value != last_state_sent {
            let _ = client.publish(
                &topics.transpose_state,
                QoS::AtLeastOnce,
                true,
                current_value.to_string(),
            );
            last_state_sent = current_value;
        }

        // Vermeide Busy-Loop
        thread::sleep(Duration::from_millis(LOOP_DELAY_MS));
    }
}
