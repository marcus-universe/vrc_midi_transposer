use rumqttc::{Client, Event, Incoming, LastWill, MqttOptions, QoS};
use std::thread;
use std::time::{Duration, Instant};
use std::collections::HashSet;
use std::sync::atomic::Ordering;

// MQTT Configuration Constants
const CLIENT_ID: &str = "transposer2025";
// Keep-alive kept low so the blocking event loop wakes up promptly and checks EXIT_FLAG on shutdown
const KEEP_ALIVE_SECS: u64 = 2;
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
    // Dynamic OSC controls
    osc_control_set: Vec<String>,
    osc_control_state: Vec<String>,
}

impl MqttTopics {
    fn new(base_topic: &str) -> Self {
        // Build dynamic topics from config.osc.sending_addresses
        let cfg = crate::get_config();
        let mut dyn_set = Vec::new();
        let mut dyn_state = Vec::new();
        for item in &cfg.osc.sending_addresses {
            let slug = item.name.to_lowercase().replace(' ', "_");
            dyn_set.push(format!("{}/osc/custom/{}/set", base_topic, slug));
            dyn_state.push(format!("{}/state/osc/custom/{}", base_topic, slug));
        }
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
            osc_control_set: dyn_set,
            osc_control_state: dyn_state,
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

    // Dynamic controls based on config
    let cfg = crate::get_config();
    for (idx, item) in cfg.osc.sending_addresses.iter().enumerate() {
        let slug = item.name.to_lowercase().replace(' ', "_");
        match item.ty {
            crate::OscValueType::Bool => {
                let cfg_topic = format!("homeassistant/switch/midi_transposer/custom_{}/config", slug);
                let payload = format!(
                    r#"{{
  "name": "{}",
  "unique_id": "{}_custom_{}",
  "command_topic": "{}",
  "state_topic": "{}",
  "payload_on": "1",
  "payload_off": "0",
  "state_on": "1",
  "state_off": "0",
  "availability_topic": "{}",
  "device": {}
}}"#,
                    item.name,
                    CLIENT_ID,
                    slug,
                    topics.osc_control_set[idx],
                    topics.osc_control_state[idx],
                    topics.availability,
                    device_json
                );
                let _ = client.publish(cfg_topic, QoS::AtLeastOnce, true, payload);
            }
            crate::OscValueType::Float => {
                let cfg_topic = format!("homeassistant/number/midi_transposer/custom_{}/config", slug);
                let mut body = format!(
                    r#"{{
  "name": "{}",
  "unique_id": "{}_custom_{}",
  "command_topic": "{}",
  "state_topic": "{}",
  "step": 0.01,
  "availability_topic": "{}",
  "device": {}"#,
                    item.name,
                    CLIENT_ID,
                    slug,
                    topics.osc_control_set[idx],
                    topics.osc_control_state[idx],
                    topics.availability,
                    device_json
                );
                if let Some(min) = item.min { body.push_str(&format!(",\n  \"min\": {}", min)); }
                if let Some(max) = item.max { body.push_str(&format!(",\n  \"max\": {}", max)); }
                body.push_str("\n}");
                let _ = client.publish(cfg_topic, QoS::AtLeastOnce, true, body);
            }
        }
    }

    if crate::is_debug_enabled() { println!("[MQTT] Home Assistant Discovery configured ({} dynamic controls)", cfg.osc.sending_addresses.len()); }
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
    
    // Dynamic OSC controls
    for set_topic in &topics.osc_control_set {
        client.subscribe(set_topic, QoS::AtLeastOnce)?;
    }
    // Also subscribe to Home Assistant discovery topics for this device to allow cleanup of stale entities
    client.subscribe("homeassistant/+/midi_transposer/#", QoS::AtLeastOnce)?;
    if crate::is_debug_enabled() {
        println!(
            "[MQTT] Subscribed to topics: {}, {}, {}, {}, {}, {}; +{} dynamic OSC controls", 
            topics.transpose_set, topics.transpose_up, topics.transpose_down,
            topics.osc_sending_enabled_set, topics.osc_send_original_set,
            topics.debug_enabled_set,
            topics.osc_control_set.len()
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
    } else {
        // Dynamic OSC control messages
        // Find matching index
        if let Some(idx) = topics.osc_control_set.iter().position(|t| t == topic) {
            let cfg = &crate::get_config().osc.sending_addresses[idx];
            match cfg.ty {
                crate::OscValueType::Bool => {
                    let on = parse_boolean_payload(payload);
                    let int_val = if on { 1 } else { 0 };
                    let target = format!("{}:{}", crate::get_config().osc.sending_addr, crate::get_config().osc.sending_port);
                    let _ = crate::remote::osc_sender::send_single_osc_message(&cfg.addr, rosc::OscType::Int(int_val), &target);
                    let _ = client.publish(&topics.osc_control_state[idx], QoS::AtLeastOnce, true, int_val.to_string());
                }
                crate::OscValueType::Float => {
                    let s = std::str::from_utf8(payload).unwrap_or("").trim();
                    let mut v = s.parse::<f32>().unwrap_or(cfg.default);
                    if let Some(min) = cfg.min { if v < min { v = min; } }
                    if let Some(max) = cfg.max { if v > max { v = max; } }
                    let target = format!("{}:{}", crate::get_config().osc.sending_addr, crate::get_config().osc.sending_port);
                    let _ = crate::remote::osc_sender::send_single_osc_message(&cfg.addr, rosc::OscType::Float(v), &target);
                    let _ = client.publish(&topics.osc_control_state[idx], QoS::AtLeastOnce, true, v.to_string());
                }
            }
        }
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

    // Track HA discovery topics to allow cleanup of removed custom controls
    let mut expected_custom_discovery: HashSet<String> = HashSet::new();
    let mut observed_custom_discovery: HashSet<String> = HashSet::new();
    let mut cleanup_started_at: Option<Instant> = None;
    const CLEANUP_WINDOW_MS: u64 = 1200; // wait briefly after ConnAck to collect retained discovery topics

    loop {
        // Prüfe Exit-Flag
        if crate::EXIT_FLAG.load(Ordering::SeqCst) {
            if crate::is_debug_enabled() { println!("[MQTT] Shutdown requested, stopping listener"); }
            // Versuche sauberes Disconnect, ignorieren bei Fehlern
            let _ = client.disconnect();
            break;
        }

        // Verarbeite nächste MQTT-Nachricht
        if let Some(result) = iter.next() {
            match result {
                Ok(Event::Incoming(Incoming::Publish(publish))) => {
                    let topic = publish.topic.as_str();
                    let payload = publish.payload.as_ref();
                    // Collect retained HA discovery configs for our namespace
                    if topic.starts_with("homeassistant/") && topic.contains("/midi_transposer/") && topic.contains("/custom_") {
                        observed_custom_discovery.insert(topic.to_string());
                    }
                    
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

                    // Dynamic topics from config
                    {
                        let cfg = crate::get_config();
                        // Build dynamic topic lists based on sending_addresses
                        // We publish HA discovery entities for each and subscribe to their set topics.
                        // We put them under: <base>/osc/custom/<slug>/set and state under <base>/state/osc/custom/<slug>
                        // Slug: lowercase name with spaces -> '_'
                        let mut set_topics = Vec::new();
                        let mut state_topics = Vec::new();
                        let mut names = Vec::new();
                        for item in &cfg.osc.sending_addresses {
                            let slug = item.name.to_lowercase().replace(' ', "_");
                            let set_t = format!("{}/osc/custom/{}/set", topics.availability.trim_end_matches("/availability"), slug);
                            let state_t = format!("{}/state/osc/custom/{}", cfg.mqtt.base_topic, slug);
                            set_topics.push(set_t);
                            state_topics.push(state_t);
                            names.push(item.name.clone());
                        }
                        // Update topics (unsafe to mutate borrowed; but we own &mut topics? Here we have &MqttTopics)
                        // Workaround: create local copies to publish discovery/state below; actual subscribe occurs via subscribe_to_topics using topics.osc_control_set
                        // Populate the vectors inside topics using unsafe cast (not allowed). Instead, rebuild MqttTopics earlier.
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
                    // Publish initial states for dynamic OSC controls using configured defaults
                    let cfg = crate::get_config();
                    for (idx, item) in cfg.osc.sending_addresses.iter().enumerate() {
                        match item.ty {
                            crate::OscValueType::Bool => {
                                let v = if item.default != 0.0 { "1" } else { "0" };
                                let _ = client.publish(&topics.osc_control_state[idx], QoS::AtLeastOnce, true, v);
                            }
                            crate::OscValueType::Float => {
                                let mut v = item.default;
                                if let Some(min) = item.min { if v < min { v = min; } }
                                if let Some(max) = item.max { if v > max { v = max; } }
                                let _ = client.publish(&topics.osc_control_state[idx], QoS::AtLeastOnce, true, v.to_string());
                            }
                        }
                    }
                    // Compute expected HA discovery topics for current custom controls and start cleanup window
                    expected_custom_discovery.clear();
                    for item in &cfg.osc.sending_addresses {
                        let slug = item.name.to_lowercase().replace(' ', "_");
                        let comp = match item.ty { crate::OscValueType::Bool => "switch", crate::OscValueType::Float => "number" };
                        expected_custom_discovery.insert(format!("homeassistant/{}/midi_transposer/custom_{}/config", comp, slug));
                    }
                    observed_custom_discovery.clear();
                    cleanup_started_at = Some(Instant::now());
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
            if crate::is_debug_enabled() { println!("[MQTT] Connection iterator ended"); }
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

        // After a short window post-ConnAck, cleanup stale HA discovery topics for removed custom controls
        if let Some(start) = cleanup_started_at {
            if start.elapsed() >= Duration::from_millis(CLEANUP_WINDOW_MS) {
                for t in observed_custom_discovery.drain() {
                    if !expected_custom_discovery.contains(&t) {
                        if crate::is_debug_enabled() { println!("[MQTT] Cleaning up stale HA discovery topic: {}", t); }
                        // Publish empty retained payload to delete entity in Home Assistant
                        let _ = client.publish(t, QoS::AtLeastOnce, true, "");
                    }
                }
                cleanup_started_at = None; // one-time per connection
            }
        }

        // Vermeide Busy-Loop
        thread::sleep(Duration::from_millis(LOOP_DELAY_MS));
    }
    if crate::is_debug_enabled() { println!("[MQTT] Listener loop terminated"); }
}
