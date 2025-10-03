use rumqttc::{Client, Event, Incoming, LastWill, MqttOptions, QoS};
use std::thread;
use std::time::Duration;
use std::sync::atomic::Ordering;

// MQTT Configuration Constants
const CLIENT_ID: &str = "transposer2025";
const KEEP_ALIVE_SECS: u64 = 30;
const RECONNECT_DELAY_SECS: u64 = 1;
const LOOP_DELAY_MS: u64 = 50;
// Queue for outgoing MQTT requests (subscribe/publish). Needs to be large enough
// to hold initial discovery publishes + subscriptions until the event loop drains.
const QUEUE_SIZE: usize = 64;

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
    // OSC related
    osc_sending_enabled_set: String,
    osc_sending_enabled_state: String,
    osc_send_original_set: String,
    osc_send_original_state: String,
    // Debug related
    debug_enabled_set: String,
    debug_enabled_state: String,
}

impl MqttTopics {
    fn new(base_topic: &str) -> Self {
        Self {
            transpose_set: format!("{}/transpose", base_topic),
            transpose_up: format!("{}/transposeUp", base_topic),
            transpose_down: format!("{}/transposeDown", base_topic),
            transpose_state: format!("{}/state/transpose", base_topic),
            availability: format!("{}/availability", base_topic),
            // OSC switches
            osc_sending_enabled_set: format!("{}/osc/sendingEnabled", base_topic),
            osc_sending_enabled_state: format!("{}/state/osc/sendingEnabled", base_topic),
            osc_send_original_set: format!("{}/osc/sendOriginal", base_topic),
            osc_send_original_state: format!("{}/state/osc/sendOriginal", base_topic),
            // Debug switch
            debug_enabled_set: format!("{}/debug/enabled", base_topic),
            debug_enabled_state: format!("{}/state/debug/enabled", base_topic),
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

    // Switch: OSC Sending Enabled
    let switch_osc_send_cfg = format!(
        r#"{{
  "name": "OSC Sending Enabled",
  "unique_id": "{}_osc_sending_enabled",
  "command_topic": "{}",
  "state_topic": "{}",
  "payload_on": "1",
  "payload_off": "0",
  "state_on": "1",
  "state_off": "0",
  "availability_topic": "{}",
  "device": {}
}}"#,
        CLIENT_ID,
        topics.osc_sending_enabled_set,
        topics.osc_sending_enabled_state,
        topics.availability,
        device_json
    );
    let _ = client.publish(
        "homeassistant/switch/midi_transposer/osc_sending_enabled/config",
        QoS::AtLeastOnce,
        true,
        switch_osc_send_cfg,
    );

    // Switch: OSC Send Original (if off -> send transposed)
    let switch_send_original_cfg = format!(
        r#"{{
  "name": "OSC Send Original",
  "unique_id": "{}_osc_send_original",
  "command_topic": "{}",
  "state_topic": "{}",
  "payload_on": "1",
  "payload_off": "0",
  "state_on": "1",
  "state_off": "0",
  "availability_topic": "{}",
  "device": {}
}}"#,
        CLIENT_ID,
        topics.osc_send_original_set,
        topics.osc_send_original_state,
        topics.availability,
        device_json
    );
    let _ = client.publish(
        "homeassistant/switch/midi_transposer/osc_send_original/config",
        QoS::AtLeastOnce,
        true,
        switch_send_original_cfg,
    );

    // Switch: Debug Enabled
    let switch_debug_cfg = format!(
        r#"{{
  "name": "Debug Enabled",
  "unique_id": "{}_debug_enabled",
  "command_topic": "{}",
  "state_topic": "{}",
  "payload_on": "1",
  "payload_off": "0",
  "state_on": "1",
  "state_off": "0",
  "availability_topic": "{}",
  "device": {}
}}"#,
        CLIENT_ID,
        topics.debug_enabled_set,
        topics.debug_enabled_state,
        topics.availability,
        device_json
    );
    let _ = client.publish(
        "homeassistant/switch/midi_transposer/debug_enabled/config",
        QoS::AtLeastOnce,
        true,
        switch_debug_cfg,
    );

    if crate::is_debug_enabled() { println!("[MQTT] Home Assistant Discovery configured"); }
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
    // OSC related switches
    client.subscribe(&topics.osc_sending_enabled_set, QoS::AtLeastOnce)?;
    client.subscribe(&topics.osc_send_original_set, QoS::AtLeastOnce)?;
    // Debug switch
    client.subscribe(&topics.debug_enabled_set, QoS::AtLeastOnce)?;
    
    if crate::is_debug_enabled() {
        println!(
            "[MQTT] Subscribed to topics: {}, {}, {}, {}, {}, {}", 
            topics.transpose_set, topics.transpose_up, topics.transpose_down,
            topics.osc_sending_enabled_set, topics.osc_send_original_set,
            topics.debug_enabled_set
        );
    }
    
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

        // Hauptschleife für MQTT-Nachrichten (publishes erfolgen nach ConnAck)
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
            if crate::is_debug_enabled() { println!("[MQTT] Transpose set to {}", clamped_value); }
            let _ = client.publish(&topics.transpose_state, QoS::AtLeastOnce, true, clamped_value.to_string());
            return Some(clamped_value);
        } else {
            eprintln!("[MQTT] Invalid /transpose payload: {:?}", payload);
        }
    } else if topic == topics.transpose_up {
        // Transpose erhöhen
        if parse_boolean_payload(payload) {
            let current = crate::TRANSPOSE_SEMITONES.load(Ordering::SeqCst);
            let new_value = crate::set_transpose_semitones(current + 1);
            if crate::is_debug_enabled() { println!("[MQTT] Transpose UP: {} -> {}", current, new_value); }
            let _ = client.publish(&topics.transpose_state, QoS::AtLeastOnce, true, new_value.to_string());
            return Some(new_value);
        }
    } else if topic == topics.transpose_down {
        // Transpose verringern
        if parse_boolean_payload(payload) {
            let current = crate::TRANSPOSE_SEMITONES.load(Ordering::SeqCst);
            let new_value = crate::set_transpose_semitones(current - 1);
            if crate::is_debug_enabled() { println!("[MQTT] Transpose DOWN: {} -> {}", current, new_value); }
            let _ = client.publish(&topics.transpose_state, QoS::AtLeastOnce, true, new_value.to_string());
            return Some(new_value);
        }
    } else if topic == topics.osc_sending_enabled_set {
        // Toggle OSC sending enabled
        let enable = parse_boolean_payload(payload);
        crate::OSC_SENDING_ENABLED.store(enable, Ordering::SeqCst);
    if crate::is_debug_enabled() { println!("[MQTT] OSC Sending Enabled -> {}", enable); }
        let _ = client.publish(&topics.osc_sending_enabled_state, QoS::AtLeastOnce, true, if enable { "1" } else { "0" });
    } else if topic == topics.osc_send_original_set {
        // Toggle whether to send original (true) or transposed (false)
        let send_orig = parse_boolean_payload(payload);
        crate::OSC_SEND_ORIGINAL.store(send_orig, Ordering::SeqCst);
    if crate::is_debug_enabled() { println!("[MQTT] OSC Send Original -> {}", send_orig); }
        let _ = client.publish(&topics.osc_send_original_state, QoS::AtLeastOnce, true, if send_orig { "1" } else { "0" });
    } else if topic == topics.debug_enabled_set {
        // Toggle Debug enabled (verbose logging)
        let enable = parse_boolean_payload(payload);
        crate::DEBUG_ENABLED.store(enable, Ordering::SeqCst);
        // Note: This message is intentionally not gated by debug to ensure visibility if enabled
        if crate::is_debug_enabled() { println!("[MQTT] Debug Enabled -> {}", enable); }
        let _ = client.publish(&topics.debug_enabled_state, QoS::AtLeastOnce, true, if enable { "1" } else { "0" });
    }
    
    None
}

/// Hauptschleife für MQTT-Nachrichten-Verarbeitung
fn run_mqtt_message_loop(mut connection: rumqttc::Connection, client: &Client, topics: &MqttTopics) {
    let mut iter = connection.iter();
    let mut last_state_sent = crate::TRANSPOSE_SEMITONES.load(Ordering::SeqCst);
    let mut last_osc_enabled = crate::OSC_SENDING_ENABLED.load(Ordering::SeqCst);
    let mut last_send_original = crate::OSC_SEND_ORIGINAL.load(Ordering::SeqCst);
    let mut last_debug_enabled = crate::DEBUG_ENABLED.load(Ordering::SeqCst);

    loop {
        // Prüfe Exit-Flag
        if crate::EXIT_FLAG.load(Ordering::SeqCst) {
            if crate::is_debug_enabled() { println!("[MQTT] Shutdown requested, stopping listener"); }
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
                Ok(Event::Incoming(Incoming::ConnAck(ack))) => {
                    if crate::is_debug_enabled() { println!("[MQTT] ConnAck: session_present={}, code={:?}", ack.session_present, ack.code); }
                    // Mark connected; print green banner after we finished setup below
                    crate::MQTT_CONNECTED.store(true, Ordering::SeqCst);

                    // Beim (Re-)Connect: subscriben und initiale States/Discovery publizieren
                    if let Err(e) = subscribe_to_topics(client, topics) {
                        eprintln!("[MQTT] Subscription failed: {}", e);
                    }

                    // Discovery und Anfangszustände publizieren (einmal je Start; bei Reconnect erneut okay)
                    publish_homeassistant_discovery(client, topics);
                    let _ = client.publish(&topics.availability, QoS::AtLeastOnce, true, "online");
                    let initial_value = crate::TRANSPOSE_SEMITONES.load(Ordering::SeqCst).to_string();
                    let _ = client.publish(&topics.transpose_state, QoS::AtLeastOnce, true, initial_value);
                    let osc_enabled = if crate::OSC_SENDING_ENABLED.load(Ordering::SeqCst) { "1" } else { "0" };
                    let _ = client.publish(&topics.osc_sending_enabled_state, QoS::AtLeastOnce, true, osc_enabled);
                    let send_orig = if crate::OSC_SEND_ORIGINAL.load(Ordering::SeqCst) { "1" } else { "0" };
                    let _ = client.publish(&topics.osc_send_original_state, QoS::AtLeastOnce, true, send_orig);
                    let debug_enabled = if crate::DEBUG_ENABLED.load(Ordering::SeqCst) { "1" } else { "0" };
                    let _ = client.publish(&topics.debug_enabled_state, QoS::AtLeastOnce, true, debug_enabled);
                    // initial state published after ConnAck
                    // Now that subscriptions and discovery/state publishes are done, show green banner
                    if crate::MQTT_ENABLED.load(Ordering::SeqCst) {
                        crate::general::check::print_connections_active();
                    }
                }
                Ok(_) => {
                    // Ignore other events
                }
                Err(e) => {
                    eprintln!("[MQTT] Connection error: {} (reconnecting in {}s)", e, RECONNECT_DELAY_SECS);
                    // On connection error, mark disconnected and show red banner (only if MQTT enabled)
                    crate::MQTT_CONNECTED.store(false, Ordering::SeqCst);
                    if crate::MQTT_ENABLED.load(Ordering::SeqCst) {
                        crate::general::check::print_connections_broken();
                    }
                    thread::sleep(Duration::from_secs(RECONNECT_DELAY_SECS));
                }
            }
        } else {
            eprintln!("[MQTT] Connection iterator ended");
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

        // Publish OSC switch state changes (if altered externally)
        let osc_enabled_now = crate::OSC_SENDING_ENABLED.load(Ordering::SeqCst);
        if osc_enabled_now != last_osc_enabled {
            let _ = client.publish(
                &topics.osc_sending_enabled_state,
                QoS::AtLeastOnce,
                true,
                if osc_enabled_now { "1" } else { "0" },
            );
            last_osc_enabled = osc_enabled_now;
        }

        let send_original_now = crate::OSC_SEND_ORIGINAL.load(Ordering::SeqCst);
        if send_original_now != last_send_original {
            let _ = client.publish(
                &topics.osc_send_original_state,
                QoS::AtLeastOnce,
                true,
                if send_original_now { "1" } else { "0" },
            );
            last_send_original = send_original_now;
        }

        // Publish Debug switch state changes
        let debug_enabled_now = crate::DEBUG_ENABLED.load(Ordering::SeqCst);
        if debug_enabled_now != last_debug_enabled {
            let _ = client.publish(
                &topics.debug_enabled_state,
                QoS::AtLeastOnce,
                true,
                if debug_enabled_now { "1" } else { "0" },
            );
            last_debug_enabled = debug_enabled_now;
        }

        // Vermeide Busy-Loop
        thread::sleep(Duration::from_millis(LOOP_DELAY_MS));
    }
}
