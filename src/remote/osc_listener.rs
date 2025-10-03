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
        // Get configuration
        let config = crate::get_config();
        
    crate::general::check::OSC_LISTENER_RUNNING.store(true, std::sync::atomic::Ordering::SeqCst);

        // Bind UDP socket on configured host:port from config.json
        let bind_addr = format!("{}:{}", config.osc.listening_host, config.osc.listening_port);
        let socket = match UdpSocket::bind(&bind_addr) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("OSC bind failed on {}: {}", bind_addr, err);
                return;
            }
        };
        
        // Set socket timeout so we can check EXIT_FLAG periodically
        socket.set_read_timeout(Some(Duration::from_millis(200))).ok();
        
        if crate::is_debug_enabled() {
            println!("OSC listener bound on {} (paths: {}, {}, {})", 
                bind_addr, 
                config.osc.transpose_path,
                config.osc.transpose_up_path,
                config.osc.transpose_down_path);
        }

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

    if crate::is_debug_enabled() { println!("OSC listener exiting"); }
            crate::general::check::OSC_LISTENER_RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
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
    let config = crate::get_config();

    if addr == &config.osc.transpose_path {
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
                let clamped_value = crate::set_transpose_semitones(v);
                if crate::is_debug_enabled() { println!("[OSC] Transpose set to {}", clamped_value); }
            } else {
                eprintln!("[OSC] /transpose requires numeric argument (got {:?})", arg);
            }
        } else {
            eprintln!("[OSC] /transpose without argument ignored");
        }
    } else if addr == &config.osc.transpose_up_path {
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
                let new_value = crate::set_transpose_semitones(current + 1);
                if crate::is_debug_enabled() { println!("[OSC] Transpose UP: {} -> {}", current, new_value); }
            }
        } else {
            eprintln!("[OSC] /transposeUp without argument ignored");
        }
    } else if addr == &config.osc.transpose_down_path {
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
                let new_value = crate::set_transpose_semitones(current - 1);
                if crate::is_debug_enabled() { println!("[OSC] Transpose DOWN: {} -> {}", current, new_value); }
            }
        } else {
            eprintln!("[OSC] /transposeDown without argument ignored");
        }
    }
}
