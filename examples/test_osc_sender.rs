use std::thread;
use std::time::Duration;
use rosc::{OscMessage, OscPacket, OscType, encoder};
use std::net::UdpSocket;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("OSC Sender Test - sending to 127.0.0.1:9000");
    
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    let target_addr = "127.0.0.1:9000";
    
    // Test sending a few OSC messages
    let test_messages = vec![
        ("/avatar/parameters/C4", 1.0),
        ("/avatar/parameters/FSHARP5", 1.0), 
        ("/avatar/parameters/PitchUp", 0.5),
        ("/transpose", 5.0),
        ("/transposeUp", 1.0),
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
        
        thread::sleep(Duration::from_millis(500));
    }
    
    println!("Test completed!");
    Ok(())
}