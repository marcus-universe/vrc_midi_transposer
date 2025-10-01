use std::thread;
use std::sync::atomic::Ordering;
use std::net::UdpSocket;
use std::time::Duration;
use rosc::{OscPacket, OscType, decoder};

/// Spawns a background thread that listens for OSC on configured address.
/// Recognizes the message paths "/transpose", "/transposeUp", "/transposeDown"
/// and updates `crate::TRANSPOSE_SEMITONES` accordingly.
/// The thread checks `crate::EXIT_FLAG` periodically to shut down gracefully.
pub fn spawn_osc_listener() -> thread::JoinHandle<()> {
    thread::spawn(move || {
        // Bind UDP socket on configured address from main.rs
        let bind_addr = crate::OSC_LISTENING_ADDR;
        let socket = match UdpSocket::bind(bind_addr) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("OSC bind failed on {}: {}", bind_addr, err);
                return;
            }
        };
        
        // Set socket timeout so we can check EXIT_FLAG periodically
        socket.set_read_timeout(Some(Duration::from_millis(200))).ok();
        
        println!("OSC listener bound on {} (paths: {}, {}, {})", 
            bind_addr, 
            crate::OSC_TRANSPOSE_PATH,
            crate::OSC_TRANSPOSE_UP_PATH,
            crate::OSC_TRANSPOSE_DOWN_PATH);

        let mut buf = [0u8; rosc::decoder::MTU];

        // Listen for incoming packets
        loop {
            // Check if we should exit
            if crate::EXIT_FLAG.load(Ordering::SeqCst) {
                break;
            }

            match socket.recv_from(&mut buf) {
                Ok((size, peer_addr)) => {
                    match decoder::decode_udp(&buf[..size]) {
                        Ok((_, packet)) => {
                            handle_packet(packet);
                        }
                        Err(err) => {
                            eprintln!("OSC decode error from {}: {}", peer_addr, err);
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                    // Timeout, continue loop to check EXIT_FLAG
                    continue;
                }
                Err(err) => {
                    eprintln!("OSC recv error: {}", err);
                }
            }
        }

        println!("OSC listener exiting");
    })
}

fn handle_packet(packet: OscPacket) {
    match packet {
        OscPacket::Message(msg) => handle_message(msg),
        OscPacket::Bundle(bundle) => {
            // Process all messages in the bundle
            for pkt in bundle.content {
                handle_packet(pkt);
            }
        }
    }
}

fn handle_message(msg: rosc::OscMessage) {
    let addr = &msg.addr;
    let args = &msg.args;

    if addr == crate::OSC_TRANSPOSE_PATH {
        // Handle /transpose - set absolute transpose value
        if let Some(arg) = args.first() {
            let val_opt: Option<i32> = match arg {
                &OscType::Int(v) => Some(v),
                &OscType::Long(v) => i32::try_from(v).ok(),
                &OscType::Float(v) => Some(v.round() as i32),
                &OscType::Double(v) => Some(v.round() as i32),
                _ => None,
            };
            if let Some(v) = val_opt {
                crate::TRANSPOSE_SEMITONES.store(v, Ordering::SeqCst);
                println!("[OSC] Transpose set to {}", v);
            } else {
                eprintln!("[OSC] /transpose requires numeric argument (got {:?})", arg);
            }
        } else {
            eprintln!("[OSC] /transpose without argument ignored");
        }
    } else if addr == crate::OSC_TRANSPOSE_UP_PATH {
        // Handle /transposeUp - increment transpose by 1 if argument equals 1
        if let Some(arg) = args.first() {
            let should_increment = match arg {
                &OscType::Int(v) => v == 1,
                &OscType::Long(v) => v == 1,
                &OscType::Float(v) => (v - 1.0).abs() < f32::EPSILON,
                &OscType::Double(v) => (v - 1.0).abs() < f64::EPSILON,
                &OscType::Bool(b) => b, // true is equivalent to 1
                _ => false,
            };
            
            if should_increment {
                let current = crate::TRANSPOSE_SEMITONES.load(Ordering::SeqCst);
                let new_value = current + 1;
                crate::TRANSPOSE_SEMITONES.store(new_value, Ordering::SeqCst);
                println!("[OSC] Transpose UP: {} -> {}", current, new_value);
            }
        } else {
            eprintln!("[OSC] /transposeUp without argument ignored");
        }
    } else if addr == crate::OSC_TRANSPOSE_DOWN_PATH {
        // Handle /transposeDown - decrement transpose by 1 if argument equals 1
        if let Some(arg) = args.first() {
            let should_decrement = match arg {
                &OscType::Int(v) => v == 1,
                &OscType::Long(v) => v == 1,
                &OscType::Float(v) => (v - 1.0).abs() < f32::EPSILON,
                &OscType::Double(v) => (v - 1.0).abs() < f64::EPSILON,
                &OscType::Bool(b) => b, // true is equivalent to 1
                _ => false,
            };
            
            if should_decrement {
                let current = crate::TRANSPOSE_SEMITONES.load(Ordering::SeqCst);
                let new_value = current - 1;
                crate::TRANSPOSE_SEMITONES.store(new_value, Ordering::SeqCst);
                println!("[OSC] Transpose DOWN: {} -> {}", current, new_value);
            }
        } else {
            eprintln!("[OSC] /transposeDown without argument ignored");
        }
    }
}
