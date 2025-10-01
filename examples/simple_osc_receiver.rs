use std::net::UdpSocket;
use rosc::{OscPacket, decoder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Simple OSC Receiver - listening on 127.0.0.1:9000");
    println!("This will receive OSC messages sent by the MIDI transposer");
    
    let socket = UdpSocket::bind("127.0.0.1:9000")?;
    println!("Listening for OSC messages...");
    
    let mut buf = [0u8; rosc::decoder::MTU];
    
    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, addr)) => {
                println!("Received {} bytes from {}", size, addr);
                
                match decoder::decode_udp(&buf[..size]) {
                    Ok((_, packet)) => {
                        match packet {
                            OscPacket::Message(msg) => {
                                println!("  Message: {} with {} args", msg.addr, msg.args.len());
                                for (i, arg) in msg.args.iter().enumerate() {
                                    println!("    Arg {}: {:?}", i, arg);
                                }
                            }
                            OscPacket::Bundle(bundle) => {
                                println!("  Bundle with {} elements", bundle.content.len());
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("  Failed to decode OSC: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to receive: {}", e);
            }
        }
    }
}