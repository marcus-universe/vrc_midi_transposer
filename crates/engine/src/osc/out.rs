//! OSC sender: one UDP socket shared by the forwarder and the custom controls.

use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};

use rosc::{encoder, OscMessage, OscPacket, OscType};

use super::notes;
use crate::state::{lock, Shared};

pub struct OscOut {
    shared: Arc<Shared>,
    link: Mutex<Option<(UdpSocket, SocketAddr)>>,
}

impl OscOut {
    pub fn new(shared: Arc<Shared>) -> Self {
        OscOut {
            shared,
            link: Mutex::new(None),
        }
    }

    /// Point the sender at `host:port`. Returns the resolved address as text.
    pub fn set_target(&self, host: &str, port: u16) -> Result<String, String> {
        let host = if host.trim().is_empty() {
            "127.0.0.1"
        } else {
            host.trim()
        };
        let addr = (host, port)
            .to_socket_addrs()
            .map_err(|e| format!("Cannot resolve OSC target {host}:{port}: {e}"))?
            .next()
            .ok_or_else(|| format!("Cannot resolve OSC target {host}:{port}"))?;
        // Bind on all interfaces so targets on other machines are reachable.
        let bind = if addr.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" };
        let socket = UdpSocket::bind(bind).map_err(|e| format!("Cannot open OSC send socket: {e}"))?;
        *lock(&self.link) = Some((socket, addr));
        Ok(addr.to_string())
    }

    pub fn clear_target(&self) {
        *lock(&self.link) = None;
    }

    pub fn send(&self, addr: &str, arg: OscType) {
        let guard = lock(&self.link);
        let Some((socket, target)) = guard.as_ref() else {
            return;
        };
        let packet = OscPacket::Message(OscMessage {
            addr: addr.to_string(),
            args: vec![arg],
        });
        let bytes = match encoder::encode(&packet) {
            Ok(b) => b,
            Err(e) => {
                self.shared.error(format!("[OSC] Cannot encode {addr}: {e}"));
                return;
            }
        };
        match socket.send_to(&bytes, target) {
            Ok(n) => self
                .shared
                .debug_log(|| format!("[OSC] Sent {n} bytes to {target}: {addr}")),
            Err(e) => self
                .shared
                .debug_log(|| format!("[OSC] Failed to send to {target}: {e}")),
        }
    }

    pub fn note(&self, note: u8, on: bool) {
        self.send(notes::note_address(note), OscType::Int(on as i32));
    }

    pub fn pitch_bend(&self, lsb: u8, msb: u8) {
        if let Some((addr, value)) = notes::pitch_bend(lsb, msb) {
            self.send(addr, OscType::Float(value));
        }
    }
}
