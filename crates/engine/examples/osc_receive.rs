//! Print every OSC message arriving on a UDP port. A stand-in for VRChat when
//! checking what the app sends.
//!
//! cargo run -p transposer-engine --example osc_receive -- 127.0.0.1:9000

use std::net::UdpSocket;

use rosc::OscPacket;

fn print_packet(packet: OscPacket) {
    match packet {
        OscPacket::Message(msg) => println!("{} {:?}", msg.addr, msg.args),
        OscPacket::Bundle(bundle) => bundle.content.into_iter().for_each(print_packet),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:9000".to_string());
    let socket = UdpSocket::bind(&addr)?;
    println!("Listening for OSC on {addr} (Ctrl+C to stop)");
    let mut buf = [0u8; rosc::decoder::MTU];
    loop {
        let (size, _) = socket.recv_from(&mut buf)?;
        match rosc::decoder::decode_udp(&buf[..size]) {
            Ok((_, packet)) => print_packet(packet),
            Err(e) => eprintln!("decode error: {e}"),
        }
    }
}
