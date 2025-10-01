use rumqttc::{Client, Event, Incoming, LastWill, MqttOptions, QoS};
use std::thread;
use std::time::Duration;
use std::sync::atomic::Ordering;

fn parse_transpose_payload(payload: &[u8]) -> Option<i32> {
    let s = std::str::from_utf8(payload).ok()?.trim();
    // Accept integers, floats (rounded), or booleans for up/down topics
    if let Ok(v) = s.parse::<i32>() {
        return Some(v);
    }
    if let Ok(vf) = s.parse::<f32>() {
        return Some(vf.round() as i32);
    }
    None
}

/// Spawns a background thread that subscribes to MQTT topics and updates transpose.
/// Topics:
/// - <base>/transpose (payload: integer, sets absolute transpose)
/// - <base>/transposeUp (payload: 1/true increments by 1)
/// - <base>/transposeDown (payload: 1/true decrements by 1)
pub fn spawn_mqtt_listener() -> thread::JoinHandle<()> {
    let host = crate::MQTT_BROKER_HOST;
    let port = crate::MQTT_BROKER_PORT;
    let base = crate::MQTT_BASE_TOPIC;
    // Load credentials from optional JSON (falls back to constants)
    let creds = crate::load_mqtt_credentials();

    thread::spawn(move || {
        let client_id = "transposer2025";
        let availability_topic = format!("{}/availability", base);

        let mut mqttoptions = MqttOptions::new(client_id, host, port);
        mqttoptions.set_keep_alive(Duration::from_secs(30));
        mqttoptions.set_credentials(&creds.username, &creds.password);
        // Set LWT (offline). We'll publish "online" after connecting.
        mqttoptions.set_last_will(LastWill::new(
            availability_topic.clone(),
            "offline",
            QoS::AtLeastOnce,
            true,
        ));

        let (client, mut connection) = Client::new(mqttoptions, 10);

        // Subscribe to topics
        let t_set = format!("{}/transpose", base);
        let t_up = format!("{}/transposeUp", base);
        let t_down = format!("{}/transposeDown", base);
        let t_state = format!("{}/state/transpose", base);

        if let Err(e) = client.subscribe(&t_set, QoS::AtLeastOnce) {
            eprintln!("[MQTT] subscribe {} failed: {}", t_set, e);
            return;
        }
        if let Err(e) = client.subscribe(&t_up, QoS::AtLeastOnce) {
            eprintln!("[MQTT] subscribe {} failed: {}", t_up, e);
            return;
        }
        if let Err(e) = client.subscribe(&t_down, QoS::AtLeastOnce) {
            eprintln!("[MQTT] subscribe {} failed: {}", t_down, e);
            return;
        }

        println!(
            "[MQTT] Connected to {}:{} as '{}' and subscribed to {}, {}, {}",
            host, port, creds.username, t_set, t_up, t_down
        );

        // Publish Home Assistant MQTT Discovery configs so the device shows up automatically.
        // Device metadata
        let device_id = "midi_transposer_transposer2025";
        let device_json = format!(
            "{{\n  \"identifiers\": [\"{}\"],\n  \"name\": \"MIDI Transposer 2025\",\n  \"manufacturer\": \"MidiTransposer\",\n  \"model\": \"MidiTransposer\"\n}}",
            device_id
        );

        // number entity for absolute transpose
        let number_disc_topic = "homeassistant/number/midi_transposer/transpose/config";
        let number_config = format!(
            "{{\n  \"name\": \"MIDI Transpose\",\n  \"unique_id\": \"{}_transpose\",\n  \"command_topic\": \"{}\",\n  \"state_topic\": \"{}\",\n  \"min\": -24,\n  \"max\": 24,\n  \"step\": 1,\n  \"unit_of_measurement\": \"semitones\",\n  \"availability_topic\": \"{}\",\n  \"device\": {}\n}}",
            client_id,
            t_set,
            t_state,
            availability_topic,
            device_json
        );
        let _ = client.publish(number_disc_topic, QoS::AtLeastOnce, true, number_config);

        // button for transpose up
        let btn_up_disc_topic = "homeassistant/button/midi_transposer/transpose_up/config";
        let btn_up_config = format!(
            "{{\n  \"name\": \"Transpose Up\",\n  \"unique_id\": \"{}_transpose_up\",\n  \"command_topic\": \"{}\",\n  \"payload_press\": \"1\",\n  \"availability_topic\": \"{}\",\n  \"device\": {}\n}}",
            client_id,
            t_up,
            availability_topic,
            device_json
        );
        let _ = client.publish(btn_up_disc_topic, QoS::AtLeastOnce, true, btn_up_config);

        // button for transpose down
        let btn_down_disc_topic = "homeassistant/button/midi_transposer/transpose_down/config";
        let btn_down_config = format!(
            "{{\n  \"name\": \"Transpose Down\",\n  \"unique_id\": \"{}_transpose_down\",\n  \"command_topic\": \"{}\",\n  \"payload_press\": \"1\",\n  \"availability_topic\": \"{}\",\n  \"device\": {}\n}}",
            client_id,
            t_down,
            availability_topic,
            device_json
        );
        let _ = client.publish(btn_down_disc_topic, QoS::AtLeastOnce, true, btn_down_config);

        // Mark device online and publish initial state retained
        let _ = client.publish(&availability_topic, QoS::AtLeastOnce, true, "online");
        let initial = crate::TRANSPOSE_SEMITONES.load(Ordering::SeqCst).to_string();
        let _ = client.publish(&t_state, QoS::AtLeastOnce, true, initial);

        let mut iter = connection.iter();
        let mut last_state_sent = crate::TRANSPOSE_SEMITONES.load(Ordering::SeqCst);
        loop {
            if crate::EXIT_FLAG.load(Ordering::SeqCst) {
                println!("[MQTT] Exit requested, shutting down listener");
                break;
            }

            if let Some(res) = iter.next() {
                match res {
                    Ok(Event::Incoming(Incoming::Publish(p))) => {
                    let topic = p.topic.as_str();
                    let payload = p.payload.as_ref();

                    if topic == t_set {
                        if let Some(v) = parse_transpose_payload(payload) {
                            crate::TRANSPOSE_SEMITONES.store(v, Ordering::SeqCst);
                            println!("[MQTT] Transpose set to {}", v);
                            // Publish new state retained
                            let _ = client.publish(&t_state, QoS::AtLeastOnce, true, v.to_string());
                            last_state_sent = v;
                        } else {
                            eprintln!("[MQTT] Invalid /transpose payload: {:?}", payload);
                        }
                    } else if topic == t_up {
                        let s = std::str::from_utf8(payload).unwrap_or("").trim().to_ascii_lowercase();
                        let should = s == "1" || s == "true" || s == "on";
                        if should {
                            let cur = crate::TRANSPOSE_SEMITONES.load(Ordering::SeqCst);
                            crate::TRANSPOSE_SEMITONES.store(cur + 1, Ordering::SeqCst);
                            println!("[MQTT] Transpose UP: {} -> {}", cur, cur + 1);
                            let newv = cur + 1;
                            let _ = client.publish(&t_state, QoS::AtLeastOnce, true, newv.to_string());
                            last_state_sent = newv;
                        }
                    } else if topic == t_down {
                        let s = std::str::from_utf8(payload).unwrap_or("").trim().to_ascii_lowercase();
                        let should = s == "1" || s == "true" || s == "on";
                        if should {
                            let cur = crate::TRANSPOSE_SEMITONES.load(Ordering::SeqCst);
                            crate::TRANSPOSE_SEMITONES.store(cur - 1, Ordering::SeqCst);
                            println!("[MQTT] Transpose DOWN: {} -> {}", cur, cur - 1);
                            let newv = cur - 1;
                            let _ = client.publish(&t_state, QoS::AtLeastOnce, true, newv.to_string());
                            last_state_sent = newv;
                        }
                    }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("[MQTT] Connection error: {} (retrying in 1s)", e);
                        thread::sleep(Duration::from_secs(1));
                    }
                }
            } else {
                eprintln!("[MQTT] Connection iterator ended");
                break;
            }

            // If state was changed by another source (stdin/OSC), publish the new value
            let current = crate::TRANSPOSE_SEMITONES.load(Ordering::SeqCst);
            if current != last_state_sent {
                let _ = client.publish(&t_state, QoS::AtLeastOnce, true, current.to_string());
                last_state_sent = current;
            }
            // avoid busy loop
            thread::sleep(Duration::from_millis(50));
        }
    })
}
