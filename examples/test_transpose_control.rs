use std::thread;
use std::time::Duration;
use rosc::{OscMessage, OscPacket, OscType, encoder};
use std::net::UdpSocket;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("OSC Control Test - sending commands to MIDI Transposer");
    println!("Sending to: 192.168.50.78:9069");
    
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    let target_addr = "192.168.50.78:9069";
    
    // Test sending transpose control messages
    let test_messages = vec![
        ("/transpose", 3.0),           // Set transpose to +3
        ("/transposeUp", 1.0),         // Increment by 1 (should go to +4)
        ("/transposeUp", 1.0),         // Increment by 1 (should go to +5)
        ("/transposeDown", 1.0),       // Decrement by 1 (should go to +4)
        ("/transpose", 0.0),           // Reset to 0
        ("/transposeDown", 1.0),       // Decrement by 1 (should go to -1)
    ];
    
    for (path, value) in test_messages {
        let osc_msg = OscMessage {
            addr: path.to_string(),
            args: vec![OscType::Float(value)],
        };
        
        let packet = OscPacket::Message(osc_msg);
        let msg_buf = encoder::encode(&packet)?;
        
        socket.send_to(&msg_buf, target_addr)?;
        println!("Sent OSC: {} = {}", path, value);
        
        thread::sleep(Duration::from_millis(1000));
    }
    
    println!("Control test completed!");
    Ok(())
}