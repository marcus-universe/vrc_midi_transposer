use std::thread;
use std::sync::atomic::Ordering;
use std::net::UdpSocket;
use std::time::Duration;
use rosc::{OscPacket, OscType, decoder};
use midir::{MidiInput, MidiOutput};

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
        
        // Resolve paths from listening_addresses (preferred) with fallback to deprecated fields
        let (transpose_path, transpose_up_path, transpose_down_path, debug_path, midi_list_path, osc_orig_path, osc_transp_path) = {
            let mut tp: Option<String> = None;
            let mut up: Option<String> = None;
            let mut down: Option<String> = None;
            let mut dbg: Option<String> = None;
            let mut mlist: Option<String> = None;
            let mut osco: Option<String> = None;
            let mut osct: Option<String> = None;
            for entry in &config.osc.listening_addresses {
                if let Some(role) = entry.role {
                    match role {
                        crate::OscListenRole::Transpose => if tp.is_none() { tp = Some(entry.addr.clone()); },
                        crate::OscListenRole::TransposeUp => if up.is_none() { up = Some(entry.addr.clone()); },
                        crate::OscListenRole::TransposeDown => if down.is_none() { down = Some(entry.addr.clone()); },
                        crate::OscListenRole::DebugSet => if dbg.is_none() { dbg = Some(entry.addr.clone()); },
                        crate::OscListenRole::MidiList => if mlist.is_none() { mlist = Some(entry.addr.clone()); },
                        crate::OscListenRole::OscSendOriginal => if osco.is_none() { osco = Some(entry.addr.clone()); },
                        crate::OscListenRole::OscSendTransposed => if osct.is_none() { osct = Some(entry.addr.clone()); },
                    }
                }
            }
            (
                tp.unwrap_or_else(|| config.osc.transpose_path.clone()),
                up.unwrap_or_else(|| config.osc.transpose_up_path.clone()),
                down.unwrap_or_else(|| config.osc.transpose_down_path.clone()),
                dbg,
                mlist,
                osco,
                osct,
            )
        };

        if crate::is_debug_enabled() {
            println!("OSC listener bound on {} (paths: {}, {}, {} | extra: debug={:?}, midi_list={:?}, osc_original={:?}, osc_transposed={:?})", 
                bind_addr, 
                transpose_path,
                transpose_up_path,
                transpose_down_path,
                debug_path, midi_list_path, osc_orig_path, osc_transp_path);
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

    // Resolve current paths the same way as in the listener start
    let (transpose_path, transpose_up_path, transpose_down_path, debug_path, midi_list_path, osc_orig_path, osc_transp_path) = {
        let mut tp: Option<String> = None;
        let mut up: Option<String> = None;
        let mut down: Option<String> = None;
        let mut dbg: Option<String> = None;
        let mut mlist: Option<String> = None;
        let mut osco: Option<String> = None;
        let mut osct: Option<String> = None;
        for entry in &config.osc.listening_addresses {
            if let Some(role) = entry.role {
                match role {
                    crate::OscListenRole::Transpose => if tp.is_none() { tp = Some(entry.addr.clone()); },
                    crate::OscListenRole::TransposeUp => if up.is_none() { up = Some(entry.addr.clone()); },
                    crate::OscListenRole::TransposeDown => if down.is_none() { down = Some(entry.addr.clone()); },
                    crate::OscListenRole::DebugSet => if dbg.is_none() { dbg = Some(entry.addr.clone()); },
                    crate::OscListenRole::MidiList => if mlist.is_none() { mlist = Some(entry.addr.clone()); },
                    crate::OscListenRole::OscSendOriginal => if osco.is_none() { osco = Some(entry.addr.clone()); },
                    crate::OscListenRole::OscSendTransposed => if osct.is_none() { osct = Some(entry.addr.clone()); },
                }
            }
        }
        (
            tp.unwrap_or_else(|| config.osc.transpose_path.clone()),
            up.unwrap_or_else(|| config.osc.transpose_up_path.clone()),
            down.unwrap_or_else(|| config.osc.transpose_down_path.clone()),
            dbg,
            mlist,
            osco,
            osct,
        )
    };

    if addr == &transpose_path {
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
    } else if addr == &transpose_up_path {
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
    } else if addr == &transpose_down_path {
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
    } else if debug_path.as_deref() == Some(addr) {
        // Set debug enabled/disabled
        if let Some(arg) = args.first() {
            let on = match arg {
                &OscType::Int(v) => v != 0,
                &OscType::Long(v) => v != 0,
                &OscType::Float(v) => v != 0.0,
                &OscType::Double(v) => v != 0.0,
                &OscType::Bool(b) => b,
                _ => false,
            };
            crate::DEBUG_ENABLED.store(on, Ordering::SeqCst);
            println!("[OSC] Debug {}", if on { "enabled" } else { "disabled" });
        } else {
            eprintln!("[OSC] /debug without argument ignored");
        }
    } else if midi_list_path.as_deref() == Some(addr) {
        // Print MIDI device list
        match MidiInput::new("osc-list-midi-inputs") {
            Ok(midi_in) => {
                let in_ports = midi_in.ports();
                if in_ports.is_empty() { println!("MIDI Inputs: (none found)"); } else {
                    println!("MIDI Inputs:");
                    for (i, port) in in_ports.iter().enumerate() {
                        match midi_in.port_name(port) { Ok(name) => println!("  [{}] {}", i, name), Err(_) => println!("  [{}] <unknown>", i) }
                    }
                }
            }
            Err(e) => eprintln!("Error querying MIDI inputs: {}", e),
        }
        match MidiOutput::new("osc-list-midi-outputs") {
            Ok(midi_out) => {
                let out_ports = midi_out.ports();
                if out_ports.is_empty() { println!("MIDI Outputs: (none found)"); } else {
                    println!("MIDI Outputs:");
                    for (i, port) in out_ports.iter().enumerate() {
                        match midi_out.port_name(port) { Ok(name) => println!("  [{}] {}", i, name), Err(_) => println!("  [{}] <unknown>", i) }
                    }
                }
            }
            Err(e) => eprintln!("Error querying MIDI outputs: {}", e),
        }
    } else if osc_orig_path.as_deref() == Some(addr) {
        // Switch OSC send mode to original input
        crate::OSC_SEND_ORIGINAL.store(true, Ordering::SeqCst);
        println!("[OSC] Sending original input MIDI");
    } else if osc_transp_path.as_deref() == Some(addr) {
        // Switch OSC send mode to transposed output
        crate::OSC_SEND_ORIGINAL.store(false, Ordering::SeqCst);
        println!("[OSC] Sending transposed MIDI");
    }
}
