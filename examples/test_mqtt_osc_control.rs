use std::time::Duration;
use rumqttc::{MqttOptions, QoS, Client};

/// Example demonstrating MQTT control of OSC send original setting
/// 
/// This example shows how to control the OSC_SEND_ORIGINAL setting via MQTT.
/// 
/// Usage:
/// 1. Start the main transposer application
/// 2. Run this example to send MQTT commands
/// 3. Observe the OSC behavior changes in the main application
/// 
/// MQTT Topics:
/// - midi_transposer/oscSendOriginal (ON/OFF to control original vs transposed)
/// - midi_transposer/state/oscSendOriginal (state feedback)

fn main() {
    let broker_host = "192.168.50.200";  // Change to your Home Assistant IP
    let broker_port = 1883;
    let username = "your_mqtt_user";     // Change to your MQTT username
    let password = "your_mqtt_password"; // Change to your MQTT password
    
    let mut mqttoptions = MqttOptions::new("test_osc_controller", broker_host, broker_port);
    mqttoptions.set_credentials(username, password);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut connection) = Client::new(mqttoptions, 10);

    // Subscribe to state topic to see feedback
    client.subscribe("midi_transposer/state/oscSendOriginal", QoS::AtLeastOnce).unwrap();
    
    println!("Connected to MQTT broker {}:{}", broker_host, broker_port);
    println!("Sending test commands...");
    
    // Test commands
    std::thread::sleep(Duration::from_millis(100));
    
    // Enable sending original MIDI
    println!("Setting OSC to send ORIGINAL MIDI...");
    client.publish("midi_transposer/oscSendOriginal", QoS::AtLeastOnce, false, "ON").unwrap();
    
    std::thread::sleep(Duration::from_secs(2));
    
    // Enable sending transposed MIDI
    println!("Setting OSC to send TRANSPOSED MIDI...");
    client.publish("midi_transposer/oscSendOriginal", QoS::AtLeastOnce, false, "OFF").unwrap();
    
    std::thread::sleep(Duration::from_secs(2));
    
    // Back to original
    println!("Setting OSC to send ORIGINAL MIDI again...");
    client.publish("midi_transposer/oscSendOriginal", QoS::AtLeastOnce, false, "ON").unwrap();

    // Listen for a few state updates
    println!("Listening for state updates...");
    let mut count = 0;
    for notification in connection.iter() {
        match notification {
            Ok(rumqttc::Event::Incoming(rumqttc::Incoming::Publish(publish))) => {
                if publish.topic == "midi_transposer/state/oscSendOriginal" {
                    let state = String::from_utf8_lossy(&publish.payload);
                    println!("OSC Send Original State: {}", state);
                    count += 1;
                    if count >= 3 {
                        break;
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("MQTT Error: {}", e);
                break;
            }
        }
    }
    
    println!("Test completed!");
}