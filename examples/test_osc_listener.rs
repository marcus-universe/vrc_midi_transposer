use std::net::UdpSocket;
use rosc::{OscPacket, decoder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("OSC Listener Test - listening on 192.168.50.78:9069");
    println!("Send OSC messages to test paths:");
    println!("  /transpose <number>");
    println!("  /transposeUp 1");
    println!("  /transposeDown 1");
    
    let socket = UdpSocket::bind("192.168.50.78:9069")?;
    println!("Listening for OSC messages...");
    
    let mut buf = [0u8; rosc::decoder::MTU];
    
    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, peer_addr)) => {
                println!("Received {} bytes from {}", size, peer_addr);
                
                match decoder::decode_udp(&buf[..size]) {
                    Ok((_, packet)) => {
                        match packet {
                            OscPacket::Message(msg) => {
                                println!("  Message: {} with {} args", msg.addr, msg.args.len());
                                
                                if msg.addr == "/transpose" {
                                    if let Some(arg) = msg.args.first() {
                                        println!("    TRANSPOSE: {:?}", arg);
                                    }
                                } else if msg.addr == "/transposeUp" {
                                    if let Some(arg) = msg.args.first() {
                                        println!("    TRANSPOSE UP: {:?}", arg);
                                    }
                                } else if msg.addr == "/transposeDown" {
                                    if let Some(arg) = msg.args.first() {
                                        println!("    TRANSPOSE DOWN: {:?}", arg);
                                    }
                                } else {
                                    println!("    Other message: {}", msg.addr);
                                }
                                
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