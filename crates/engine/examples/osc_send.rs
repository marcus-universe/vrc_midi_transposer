//! Send one OSC message.
//!
//! cargo run -p transposer-engine --example osc_send -- 127.0.0.1:9069 /transpose 3
//! cargo run -p transposer-engine --example osc_send -- 127.0.0.1:9069 /transposeUp 1
//! cargo run -p transposer-engine --example osc_send -- 127.0.0.1:9069 /debug true
//! cargo run -p transposer-engine --example osc_send -- 127.0.0.1:9069 /midi/list

use std::net::UdpSocket;

use rosc::{encoder, OscMessage, OscPacket, OscType};

fn parse_value(text: &str) -> OscType {
    if let Ok(v) = text.parse::<i32>() {
        OscType::Int(v)
    } else if let Ok(v) = text.parse::<f32>() {
        OscType::Float(v)
    } else if let Ok(v) = text.parse::<bool>() {
        OscType::Bool(v)
    } else {
        OscType::String(text.to_string())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(target), Some(addr)) = (args.first(), args.get(1)) else {
        eprintln!("usage: osc_send <host:port> <address> [value ...]");
        std::process::exit(2);
    };
    let values: Vec<OscType> = args[2..].iter().map(|s| parse_value(s)).collect();
    let packet = OscPacket::Message(OscMessage {
        addr: addr.clone(),
        args: values.clone(),
    });
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.send_to(&encoder::encode(&packet)?, target)?;
    println!("sent {addr} {values:?} to {target}");
    Ok(())
}
