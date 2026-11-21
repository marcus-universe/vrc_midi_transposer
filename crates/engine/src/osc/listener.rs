//! OSC listener: receives control messages and turns them into engine commands.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rosc::{decoder, OscPacket, OscType};

use crate::project::{ListenAddress, OscRole};
use crate::state::Shared;
use crate::{EngineCmd, Source};

pub type Routes = Arc<RwLock<HashMap<String, OscRole>>>;

pub fn build_routes(addresses: &[ListenAddress]) -> HashMap<String, OscRole> {
    let mut map = HashMap::new();
    for entry in addresses {
        if let Some(role) = entry.role {
            // First entry for a path wins.
            map.entry(entry.addr.clone()).or_insert(role);
        }
    }
    map
}

/// `1`, `1.0` or `true`: the "button pressed" value. Releases (0) are ignored.
fn is_press(arg: &OscType) -> bool {
    match *arg {
        OscType::Int(v) => v == 1,
        OscType::Long(v) => v == 1,
        OscType::Float(v) => (v - 1.0).abs() < f32::EPSILON,
        OscType::Double(v) => (v - 1.0).abs() < f64::EPSILON,
        OscType::Bool(b) => b,
        _ => false,
    }
}

/// What a message means. `Err` is a problem to report to the user.
pub fn interpret(role: OscRole, addr: &str, args: &[OscType]) -> Result<Option<EngineCmd>, String> {
    let first = args.first();
    match role {
        OscRole::Transpose => {
            let arg = first.ok_or_else(|| format!("[OSC] {addr} without argument ignored"))?;
            let value = match *arg {
                OscType::Int(v) => Some(v),
                OscType::Long(v) => i32::try_from(v).ok(),
                OscType::Float(v) => Some(v.round() as i32),
                OscType::Double(v) => Some(v.round() as i32),
                _ => None,
            };
            match value {
                Some(v) => Ok(Some(EngineCmd::SetTranspose(v, Source::Osc))),
                None => Err(format!("[OSC] {addr} requires numeric argument (got {arg:?})")),
            }
        }
        OscRole::TransposeUp | OscRole::TransposeDown => {
            let arg = first.ok_or_else(|| format!("[OSC] {addr} without argument ignored"))?;
            let delta = if role == OscRole::TransposeUp { 1 } else { -1 };
            Ok(is_press(arg).then_some(EngineCmd::Nudge(delta, Source::Osc)))
        }
        OscRole::DebugSet => {
            let arg = first.ok_or_else(|| format!("[OSC] {addr} without argument ignored"))?;
            let on = match *arg {
                OscType::Int(v) => v != 0,
                OscType::Long(v) => v != 0,
                OscType::Float(v) => v != 0.0,
                OscType::Double(v) => v != 0.0,
                OscType::Bool(b) => b,
                _ => false,
            };
            Ok(Some(EngineCmd::SetDebug(on, Source::Osc)))
        }
        OscRole::MidiList => Ok(Some(EngineCmd::ListMidi)),
        OscRole::OscSendOriginal => Ok(Some(EngineCmd::SetSendOriginal(true, Source::Osc))),
        OscRole::OscSendTransposed => Ok(Some(EngineCmd::SetSendOriginal(false, Source::Osc))),
    }
}

fn handle_packet(packet: OscPacket, routes: &Routes, shared: &Shared, cmd_tx: &Sender<EngineCmd>) {
    match packet {
        OscPacket::Message(msg) => {
            let role = routes
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .get(&msg.addr)
                .copied();
            let Some(role) = role else { return };
            match interpret(role, &msg.addr, &msg.args) {
                Ok(Some(cmd)) => {
                    let _ = cmd_tx.send(cmd);
                }
                Ok(None) => {}
                Err(text) => shared.warn(text),
            }
        }
        OscPacket::Bundle(bundle) => {
            for inner in bundle.content {
                handle_packet(inner, routes, shared, cmd_tx);
            }
        }
    }
}

pub struct ListenerHandle {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    pub bound: String,
}

impl ListenerHandle {
    /// Bind and start listening. A bind failure is returned, not logged.
    pub fn start(
        host: &str,
        port: u16,
        routes: Routes,
        shared: Arc<Shared>,
        cmd_tx: Sender<EngineCmd>,
    ) -> Result<ListenerHandle, String> {
        let host = if host.trim().is_empty() {
            "127.0.0.1"
        } else {
            host.trim()
        };
        let socket = UdpSocket::bind((host, port)).map_err(|e| match e.kind() {
            ErrorKind::AddrInUse => {
                format!("Port {port} on {host} is already in use. Close the other program or choose another port.")
            }
            _ => format!("Cannot listen on {host}:{port}: {e}"),
        })?;
        // Wake up regularly to notice the stop flag.
        socket
            .set_read_timeout(Some(Duration::from_millis(200)))
            .map_err(|e| e.to_string())?;
        let bound = socket
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| format!("{host}:{port}"));
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();
        let join = thread::Builder::new()
            .name("osc-listener".into())
            .spawn(move || {
                let mut buf = [0u8; decoder::MTU];
                while !stop_flag.load(Ordering::Relaxed) {
                    match socket.recv_from(&mut buf) {
                        Ok((size, peer)) => match decoder::decode_udp(&buf[..size]) {
                            Ok((_, packet)) => handle_packet(packet, &routes, &shared, &cmd_tx),
                            Err(e) => shared.warn(format!("[OSC] Decode error from {peer}: {e}")),
                        },
                        Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                        // Windows reports ICMP "port unreachable" from an earlier send here.
                        Err(e) if e.kind() == ErrorKind::ConnectionReset => {}
                        Err(e) => {
                            shared.debug_log(|| format!("[OSC] Receive error: {e}"));
                            thread::sleep(Duration::from_millis(50));
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(ListenerHandle {
            stop,
            join: Some(join),
            bound,
        })
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(role: OscRole, args: &[OscType]) -> Result<Option<EngineCmd>, String> {
        interpret(role, "/x", args)
    }

    #[test]
    fn transpose_accepts_numbers() {
        assert!(matches!(
            cmd(OscRole::Transpose, &[OscType::Int(3)]),
            Ok(Some(EngineCmd::SetTranspose(3, Source::Osc)))
        ));
        assert!(matches!(
            cmd(OscRole::Transpose, &[OscType::Long(-2)]),
            Ok(Some(EngineCmd::SetTranspose(-2, _)))
        ));
        assert!(matches!(
            cmd(OscRole::Transpose, &[OscType::Float(2.6)]),
            Ok(Some(EngineCmd::SetTranspose(3, _)))
        ));
        assert!(matches!(
            cmd(OscRole::Transpose, &[OscType::Double(-0.4)]),
            Ok(Some(EngineCmd::SetTranspose(0, _)))
        ));
        assert!(cmd(OscRole::Transpose, &[OscType::String("3".into())]).is_err());
        assert!(cmd(OscRole::Transpose, &[]).is_err());
    }

    #[test]
    fn up_and_down_only_react_to_a_press() {
        assert!(matches!(
            cmd(OscRole::TransposeUp, &[OscType::Int(1)]),
            Ok(Some(EngineCmd::Nudge(1, _)))
        ));
        assert!(matches!(
            cmd(OscRole::TransposeUp, &[OscType::Bool(true)]),
            Ok(Some(EngineCmd::Nudge(1, _)))
        ));
        assert!(matches!(
            cmd(OscRole::TransposeUp, &[OscType::Float(1.0)]),
            Ok(Some(EngineCmd::Nudge(1, _)))
        ));
        assert!(matches!(
            cmd(OscRole::TransposeDown, &[OscType::Int(1)]),
            Ok(Some(EngineCmd::Nudge(-1, _)))
        ));
        assert!(matches!(cmd(OscRole::TransposeUp, &[OscType::Int(0)]), Ok(None)));
        assert!(matches!(cmd(OscRole::TransposeUp, &[OscType::Int(5)]), Ok(None)));
        assert!(matches!(
            cmd(OscRole::TransposeDown, &[OscType::Bool(false)]),
            Ok(None)
        ));
        assert!(cmd(OscRole::TransposeUp, &[]).is_err());
    }

    #[test]
    fn debug_and_argument_free_roles() {
        assert!(matches!(
            cmd(OscRole::DebugSet, &[OscType::Int(1)]),
            Ok(Some(EngineCmd::SetDebug(true, _)))
        ));
        assert!(matches!(
            cmd(OscRole::DebugSet, &[OscType::Float(0.0)]),
            Ok(Some(EngineCmd::SetDebug(false, _)))
        ));
        assert!(matches!(
            cmd(OscRole::DebugSet, &[OscType::Bool(true)]),
            Ok(Some(EngineCmd::SetDebug(true, _)))
        ));
        assert!(matches!(
            cmd(OscRole::MidiList, &[]),
            Ok(Some(EngineCmd::ListMidi))
        ));
        assert!(matches!(
            cmd(OscRole::OscSendOriginal, &[]),
            Ok(Some(EngineCmd::SetSendOriginal(true, _)))
        ));
        assert!(matches!(
            cmd(OscRole::OscSendTransposed, &[OscType::Int(0)]),
            Ok(Some(EngineCmd::SetSendOriginal(false, _)))
        ));
    }

    #[test]
    fn routes_use_roles_and_skip_entries_without_one() {
        let routes = build_routes(&[
            ListenAddress {
                addr: "/transpose".into(),
                role: Some(OscRole::Transpose),
            },
            ListenAddress {
                addr: "/t".into(),
                role: Some(OscRole::Transpose),
            },
            ListenAddress {
                addr: "/spare".into(),
                role: None,
            },
            ListenAddress {
                addr: "/t".into(),
                role: Some(OscRole::DebugSet),
            },
        ]);
        assert_eq!(routes.len(), 2);
        assert_eq!(routes["/transpose"], OscRole::Transpose);
        assert_eq!(routes["/t"], OscRole::Transpose, "first entry for a path wins");
    }
}
