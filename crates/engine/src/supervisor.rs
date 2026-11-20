//! The supervisor thread: owns every service and applies commands in order.
//!
//! Front ends, the OSC listener and the MQTT client only send [`EngineCmd`]s
//! here. Nothing else starts, stops or reconfigures a service.

use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use midir::MidiInputConnection;
use rosc::OscType;

use crate::command::{self, Command};
use crate::midi::forwarder::{self, FwdMsg};
use crate::midi::ports::{self, Scanner};
use crate::midi::MidiBytes;
use crate::mqtt::topics::custom_payload;
use crate::mqtt::{MqttEvent, MqttHandle};
use crate::osc::listener::{build_routes, ListenerHandle, Routes};
use crate::osc::out::OscOut;
use crate::project::{Project, ValueType};
use crate::state::{
    now_ms, EngineEvent, Level, MidiConnState, MidiStatus, MqttConnState, PortInfo, ServiceState, Shared,
};
use crate::{EngineCmd, Source};

/// How often the MIDI port lists are polled for plugged and unplugged devices.
const SCAN_INTERVAL: Duration = Duration::from_secs(2);

pub(crate) struct Supervisor {
    shared: Arc<Shared>,
    cmd_tx: Sender<EngineCmd>,
    project: Arc<Project>,

    fwd_tx: Sender<FwdMsg>,
    fwd_join: Option<JoinHandle<()>>,
    osc_out: Arc<OscOut>,

    scanner: Scanner,
    midi_warned: bool,
    inputs: Vec<PortInfo>,
    outputs: Vec<PortInfo>,
    conn_in: Option<(MidiInputConnection<()>, PortInfo)>,
    conn_out: Option<PortInfo>,
    in_error: String,
    out_error: String,
    last_scan: Instant,

    routes: Routes,
    listener: Option<ListenerHandle>,

    mqtt: Option<MqttHandle>,
    mqtt_generation: u64,
    mqtt_enabled: bool,

    osc_sending: bool,
    send_original: bool,
    custom_values: Vec<f32>,

    /// Last overall health announced in the log: `Some(true)` = "Connections active".
    health: Option<bool>,
}

impl Supervisor {
    pub(crate) fn new(shared: Arc<Shared>, cmd_tx: Sender<EngineCmd>) -> Self {
        let osc_out = Arc::new(OscOut::new(shared.clone()));
        let (fwd_tx, fwd_rx) = std::sync::mpsc::channel();
        let fwd_join = forwarder::spawn(fwd_rx, shared.clone(), osc_out.clone(), cmd_tx.clone());
        Supervisor {
            project: shared.project(),
            shared,
            cmd_tx,
            fwd_tx,
            fwd_join: Some(fwd_join),
            osc_out,
            scanner: Scanner::default(),
            midi_warned: false,
            inputs: Vec::new(),
            outputs: Vec::new(),
            conn_in: None,
            conn_out: None,
            in_error: String::new(),
            out_error: String::new(),
            last_scan: Instant::now(),
            routes: Arc::new(RwLock::new(Default::default())),
            listener: None,
            mqtt: None,
            mqtt_generation: 0,
            mqtt_enabled: false,
            osc_sending: false,
            send_original: true,
            custom_values: Vec::new(),
            health: None,
        }
    }

    pub(crate) fn run(mut self, rx: Receiver<EngineCmd>) {
        let initial = (*self.project).clone();
        self.apply_project(initial, true);

        loop {
            let wait = SCAN_INTERVAL.saturating_sub(self.last_scan.elapsed());
            match rx.recv_timeout(wait) {
                Ok(EngineCmd::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(cmd) => self.handle(cmd),
                Err(RecvTimeoutError::Timeout) => {}
            }
            if self.last_scan.elapsed() >= SCAN_INTERVAL {
                self.rescan();
            }
        }
        self.shutdown();
    }

    fn handle(&mut self, cmd: EngineCmd) {
        match cmd {
            EngineCmd::SetTranspose(value, source) => self.set_transpose(value, source),
            EngineCmd::Nudge(delta, source) => {
                let current = self.shared.transpose.load(Ordering::Relaxed);
                self.set_transpose(current.saturating_add(delta), source);
            }
            EngineCmd::SetOscSending(on, source) => self.set_osc_sending(on, source),
            EngineCmd::SetSendOriginal(on, source) => self.set_send_original(on, source),
            EngineCmd::SetDebug(on, source) => self.set_debug(on, source),
            EngineCmd::SetMqttEnabled(on) => self.set_mqtt_enabled(on),
            EngineCmd::CustomOsc { index, value, source } => self.custom_osc(index, value, source),
            EngineCmd::Console(line) => self.console(&line),
            EngineCmd::Log(level, text) => self.shared.log(level, text),
            EngineCmd::ClearLog => self.shared.clear_log(),
            EngineCmd::ListMidi => self.list_midi(),
            EngineCmd::RefreshPorts => self.rescan(),
            EngineCmd::Panic => {
                let _ = self.fwd_tx.send(FwdMsg::Panic);
            }
            EngineCmd::LoadProject(project) => self.apply_project(*project, true),
            EngineCmd::UpdateProject(project) => self.apply_project(*project, false),
            EngineCmd::OutputLost(detail) => self.output_lost(detail),
            EngineCmd::Mqtt(generation, event) => {
                if generation == self.mqtt_generation {
                    self.mqtt_event(event);
                }
            }
            EngineCmd::Shutdown => {}
        }
    }

    // -- Runtime values -------------------------------------------------------

    fn set_transpose(&mut self, value: i32, source: Source) {
        let range = self.project.transpose;
        let clamped = value.clamp(range.min, range.max);
        if clamped != value {
            self.shared.warn(format!(
                "[TRANSPOSE] Clamped {value} to range [{}, {}] -> {clamped}",
                range.min, range.max
            ));
        }
        let old = self.shared.transpose.swap(clamped, Ordering::Relaxed);
        match source {
            Source::Console => self.shared.info(format!("Transpose set to {clamped}")),
            Source::Osc => self
                .shared
                .debug_log(|| format!("[OSC] Transpose {old} -> {clamped}")),
            Source::Mqtt => self
                .shared
                .debug_log(|| format!("[MQTT] Transpose {old} -> {clamped}")),
            Source::Ui => {}
        }
        // Home Assistant expects an answer to its own command even if nothing changed.
        if old != clamped || source == Source::Mqtt {
            if let Some(mqtt) = &self.mqtt {
                mqtt.publish_transpose(clamped);
            }
        }
        self.shared.update(|s| s.transpose = clamped);
    }

    fn sync_osc_mode(&self) {
        let _ = self.fwd_tx.send(FwdMsg::SetOsc {
            enabled: self.osc_sending,
            original: self.send_original,
        });
    }

    fn set_osc_sending(&mut self, on: bool, source: Source) {
        let changed = self.osc_sending != on;
        self.osc_sending = on;
        self.sync_osc_mode();
        let text = if on {
            "OSC sending enabled"
        } else {
            "OSC sending disabled"
        };
        match source {
            Source::Console => self.shared.info(text),
            Source::Mqtt => self.shared.debug_log(|| format!("[MQTT] {text}")),
            Source::Osc => self.shared.debug_log(|| format!("[OSC] {text}")),
            Source::Ui => {}
        }
        if changed || source == Source::Mqtt {
            if let Some(mqtt) = &self.mqtt {
                mqtt.publish_osc_sending(on);
            }
        }
        self.shared.update(|s| s.osc_sending = on);
    }

    fn set_send_original(&mut self, on: bool, source: Source) {
        let changed = self.send_original != on;
        self.send_original = on;
        self.sync_osc_mode();
        let text = if on {
            "original input MIDI"
        } else {
            "transposed MIDI"
        };
        match source {
            Source::Console => self.shared.info(format!("OSC sending {text}")),
            Source::Osc => self.shared.info(format!("[OSC] Sending {text}")),
            Source::Mqtt => self.shared.debug_log(|| format!("[MQTT] OSC sending {text}")),
            Source::Ui => {}
        }
        if changed || source == Source::Mqtt {
            if let Some(mqtt) = &self.mqtt {
                mqtt.publish_send_original(on);
            }
        }
        self.shared.update(|s| s.send_original = on);
    }

    fn set_debug(&mut self, on: bool, source: Source) {
        let changed = self.shared.debug.swap(on, Ordering::Relaxed) != on;
        let text = if on { "Debug enabled" } else { "Debug disabled" };
        match source {
            Source::Console => self.shared.info(text),
            Source::Osc => self.shared.info(format!("[OSC] {text}")),
            Source::Mqtt => self.shared.info(format!("[MQTT] {text}")),
            Source::Ui => {}
        }
        if changed || source == Source::Mqtt {
            if let Some(mqtt) = &self.mqtt {
                mqtt.publish_debug(on);
            }
        }
        self.shared.update(|s| s.debug = on);
    }

    fn custom_osc(&mut self, index: usize, value: f32, source: Source) {
        let project = self.project.clone();
        let Some(control) = project.osc.sending_addresses.get(index) else {
            return;
        };
        let value = control.clamp(value);
        let arg = match control.ty {
            ValueType::Bool => OscType::Int((value != 0.0) as i32),
            ValueType::Float => OscType::Float(value),
        };
        self.osc_out.send(&control.addr, arg);
        if let Some(slot) = self.custom_values.get_mut(index) {
            *slot = value;
        }
        if let Some(mqtt) = &self.mqtt {
            mqtt.publish_custom(index, custom_payload(control, value));
        }
        if source != Source::Ui {
            self.shared.debug_log(|| {
                format!(
                    "[{source:?}] {} = {}",
                    control.name,
                    custom_payload(control, value)
                )
            });
        }
        let values = self.custom_values.clone();
        self.shared.update(|s| s.custom_values = values);
    }

    // -- Console --------------------------------------------------------------

    fn console(&mut self, line: &str) {
        match command::parse(line) {
            Command::Empty => {}
            Command::Exit => self.shared.emit(EngineEvent::ExitRequested),
            Command::Debug(on) => self.set_debug(on, Source::Console),
            Command::Osc(on) => self.set_osc_sending(on, Source::Console),
            Command::SendOriginal(on) => self.set_send_original(on, Source::Console),
            Command::MidiList => self.list_midi(),
            Command::Help => {
                for line in command::HELP {
                    self.shared.info(*line);
                }
            }
            Command::Mqtt(on) => {
                self.set_mqtt_enabled(on);
                self.shared
                    .info(if on { "MQTT enabled" } else { "MQTT disabled" });
            }
            Command::Transpose(value) => self.set_transpose(value, Source::Console),
            Command::Unknown(text) => self.shared.warn(format!(
                "Unrecognized command: '{text}'. Type 'help' for available commands."
            )),
        }
    }

    fn list_midi(&mut self) {
        self.rescan();
        for (title, ports) in [("MIDI Inputs:", &self.inputs), ("MIDI Outputs:", &self.outputs)] {
            if ports.is_empty() {
                self.shared.info(format!("{title} (none found)"));
            } else {
                self.shared.info(title);
                for (i, port) in ports.iter().enumerate() {
                    self.shared.info(format!("  [{i}] {}", port.name));
                }
            }
        }
    }

    // -- Project --------------------------------------------------------------

    /// Apply a project. `load` is true when a project is opened: startup values
    /// are applied and every service is rebuilt. Otherwise only the services
    /// whose settings changed are restarted.
    fn apply_project(&mut self, new: Project, load: bool) {
        let new = Arc::new(new.normalized());
        let old = std::mem::replace(&mut self.project, new.clone());
        self.shared.set_project(new.clone());

        if load {
            self.shared.debug.store(new.debug, Ordering::Relaxed);
            self.osc_sending = new.osc.sending_enabled;
            self.send_original = new.osc.send_original;
            self.mqtt_enabled = new.mqtt.enabled;
            self.sync_osc_mode();
            self.shared
                .debug_log(|| format!("[PROJECT] Opened '{}'", new.name));
        } else if old.mqtt.enabled != new.mqtt.enabled {
            self.mqtt_enabled = new.mqtt.enabled;
        }

        // Transpose range
        let current = self.shared.transpose.load(Ordering::Relaxed);
        let clamped = current.clamp(new.transpose.min, new.transpose.max);
        if clamped != current {
            self.shared.transpose.store(clamped, Ordering::Relaxed);
        }

        // MIDI: drop connections whose selection changed; reconcile reconnects.
        if load || old.midi.input != new.midi.input {
            self.disconnect_input();
            self.in_error.clear();
        }
        if load || old.midi.output != new.midi.output {
            self.disconnect_output();
            self.out_error.clear();
        }

        // OSC listener
        *self.routes.write().unwrap_or_else(|e| e.into_inner()) = build_routes(&new.osc.listening_addresses);
        if load || old.osc.listen_host != new.osc.listen_host || old.osc.listen_port != new.osc.listen_port {
            self.restart_listener();
        }

        // OSC target
        if load || old.osc.target_host != new.osc.target_host || old.osc.target_port != new.osc.target_port {
            match self.osc_out.set_target(&new.osc.target_host, new.osc.target_port) {
                Ok(target) => self.shared.update(|s| s.osc_target = target),
                Err(e) => {
                    self.osc_out.clear_target();
                    self.shared.error(format!("[OSC] {e}"));
                    self.shared.update(|s| s.osc_target = String::new());
                }
            }
        }

        // Custom control values: keep the value of controls that still exist.
        let controls_changed = old.osc.sending_addresses != new.osc.sending_addresses;
        if load || controls_changed {
            let values = new
                .osc
                .sending_addresses
                .iter()
                .map(|control| {
                    let kept = (!load)
                        .then(|| {
                            old.osc
                                .sending_addresses
                                .iter()
                                .position(|c| c.name == control.name && c.ty == control.ty)
                                .and_then(|i| self.custom_values.get(i).copied())
                        })
                        .flatten();
                    control.clamp(kept.unwrap_or(control.default))
                })
                .collect();
            self.custom_values = values;
        }

        // MQTT: anything that changes the connection, the topics or the discovery documents.
        let mqtt_changed = load
            || old.mqtt != new.mqtt
            || controls_changed
            || old.transpose != new.transpose
            || self.mqtt_enabled != self.mqtt.is_some();
        if mqtt_changed {
            self.restart_mqtt();
        }

        let (debug, osc_sending, send_original, values) = (
            self.shared.is_debug(),
            self.osc_sending,
            self.send_original,
            self.custom_values.clone(),
        );
        self.shared.update(|s| {
            s.transpose = clamped;
            s.transpose_min = new.transpose.min;
            s.transpose_max = new.transpose.max;
            s.debug = debug;
            s.osc_sending = osc_sending;
            s.send_original = send_original;
            s.custom_values = values;
        });
        if clamped != current {
            if let Some(mqtt) = &self.mqtt {
                mqtt.publish_transpose(clamped);
            }
        }

        self.rescan();
        self.check_health();
    }

    // -- OSC listener ---------------------------------------------------------

    fn restart_listener(&mut self) {
        if let Some(listener) = self.listener.take() {
            listener.stop();
        }
        let osc = &self.project.osc;
        match ListenerHandle::start(
            &osc.listen_host,
            osc.listen_port,
            self.routes.clone(),
            self.shared.clone(),
            self.cmd_tx.clone(),
        ) {
            Ok(listener) => {
                let bound = listener.bound.clone();
                self.shared.debug_log(|| format!("[OSC] Listening on {bound}"));
                self.listener = Some(listener);
                self.shared.update(|s| {
                    s.osc_listener.state = ServiceState::Running;
                    s.osc_listener.detail.clear();
                    s.osc_listener.bound = bound;
                });
            }
            Err(detail) => {
                self.shared.error(format!("[OSC] {detail}"));
                self.shared.update(|s| {
                    s.osc_listener.state = ServiceState::Error;
                    s.osc_listener.detail = detail;
                    s.osc_listener.bound.clear();
                });
            }
        }
    }

    // -- MQTT -----------------------------------------------------------------

    fn set_mqtt_enabled(&mut self, on: bool) {
        if on != self.mqtt_enabled || on != self.mqtt.is_some() {
            self.mqtt_enabled = on;
            self.restart_mqtt();
            self.check_health();
        }
    }

    fn restart_mqtt(&mut self) {
        if let Some(mqtt) = self.mqtt.take() {
            mqtt.stop();
        }
        // Reports still queued from the old client are ignored from here on.
        self.mqtt_generation += 1;
        let enabled = self.mqtt_enabled;
        let start = enabled && !self.project.mqtt.broker_host.trim().is_empty();
        if start {
            self.mqtt = Some(MqttHandle::start(
                self.mqtt_generation,
                self.project.clone(),
                self.shared.clone(),
                self.cmd_tx.clone(),
            ));
        }
        self.shared.update(|s| {
            s.mqtt_enabled = enabled;
            s.mqtt.state = if start {
                MqttConnState::Connecting
            } else {
                MqttConnState::Disabled
            };
            s.mqtt.detail.clear();
            s.mqtt.entities = 0;
        });
    }

    fn mqtt_event(&mut self, event: MqttEvent) {
        match event {
            MqttEvent::Status(state, detail) => {
                self.shared.update(|s| {
                    s.mqtt.state = state;
                    s.mqtt.detail = detail;
                });
            }
            MqttEvent::Ready(entities) => {
                let broker = format!(
                    "{}:{}",
                    self.project.mqtt.broker_host.trim(),
                    self.project.mqtt.broker_port
                );
                self.shared
                    .log(Level::Success, format!("[MQTT] Connected to {broker}"));
                self.shared.update(|s| {
                    s.mqtt.state = MqttConnState::Connected;
                    s.mqtt.detail.clear();
                    s.mqtt.entities = entities;
                });
            }
            MqttEvent::Command(text) => {
                self.shared.update(|s| {
                    s.mqtt.last_command = Some(text);
                    s.mqtt.last_command_ts = now_ms();
                });
            }
        }
        self.check_health();
    }

    // -- MIDI -----------------------------------------------------------------

    fn disconnect_input(&mut self) {
        if self.conn_in.take().is_some() {
            // Keys can no longer be released by the device.
            let _ = self.fwd_tx.send(FwdMsg::Panic);
        }
    }

    fn disconnect_output(&mut self) {
        if self.conn_out.take().is_some() {
            let _ = self.fwd_tx.send(FwdMsg::SetOutput(None));
        }
    }

    fn output_lost(&mut self, detail: String) {
        if let Some(port) = self.conn_out.take() {
            self.shared
                .error(format!("[MIDI] Output '{}' failed: {detail}", port.name));
            self.out_error = detail;
            self.publish_midi_status();
        }
    }

    /// Poll the port lists and (re)connect the selected ports.
    fn rescan(&mut self) {
        self.last_scan = Instant::now();
        match self.scanner.scan() {
            Ok((inputs, outputs)) => {
                self.midi_warned = false;
                if inputs != self.inputs || outputs != self.outputs {
                    self.inputs = inputs.clone();
                    self.outputs = outputs.clone();
                    self.shared.update(|s| {
                        s.inputs = inputs;
                        s.outputs = outputs;
                    });
                }
            }
            Err(err) => {
                if !self.midi_warned {
                    self.midi_warned = true;
                    self.shared
                        .warn(format!("[MIDI] MIDI is not available on this system: {err}"));
                }
                self.inputs.clear();
                self.outputs.clear();
                self.shared.update(|s| {
                    s.inputs.clear();
                    s.outputs.clear();
                });
            }
        }
        self.reconcile_midi();
    }

    fn reconcile_midi(&mut self) {
        let project = self.project.clone();

        // Input
        if let Some((_, port)) = &self.conn_in {
            if !self.inputs.contains(port) {
                self.shared
                    .warn(format!("[MIDI] Input '{}' disconnected", port.name));
                self.disconnect_input();
            }
        }
        if self.conn_in.is_none() {
            if let Some(index) = project
                .midi
                .input
                .as_ref()
                .and_then(|want| ports::resolve(&self.inputs, want, None))
            {
                let port = self.inputs[index].clone();
                let tx = self.fwd_tx.clone();
                let result = ports::open_input(&port, move |bytes| {
                    let _ = tx.send(FwdMsg::Midi(MidiBytes::from_slice(bytes)));
                });
                match result {
                    Ok(conn) => {
                        self.shared
                            .log(Level::Success, format!("[MIDI] Input: {}", port.name));
                        self.in_error.clear();
                        self.conn_in = Some((conn, port));
                    }
                    Err(err) => {
                        if self.in_error != err {
                            self.shared
                                .error(format!("[MIDI] Cannot open input '{}': {err}", port.name));
                            self.in_error = err;
                        }
                    }
                }
            }
        }

        // Output
        if let Some(port) = &self.conn_out {
            if !self.outputs.contains(port) {
                self.shared
                    .warn(format!("[MIDI] Output '{}' disconnected", port.name));
                self.disconnect_output();
            }
        }
        if self.conn_out.is_none() {
            // A name pattern for the output must not pick the port named like the input.
            let input_name = self.conn_in.as_ref().map(|(_, p)| p.name.clone());
            let found = project
                .midi
                .output
                .as_ref()
                .and_then(|want| ports::resolve(&self.outputs, want, input_name.as_deref()));
            if let Some(index) = found {
                let port = self.outputs[index].clone();
                match ports::open_output(&port) {
                    Ok(conn) => {
                        let _ = self.fwd_tx.send(FwdMsg::SetOutput(Some(conn)));
                        self.shared
                            .log(Level::Success, format!("[MIDI] Output: {}", port.name));
                        self.out_error.clear();
                        self.conn_out = Some(port);
                    }
                    Err(err) => {
                        if self.out_error != err {
                            self.shared
                                .error(format!("[MIDI] Cannot open output '{}': {err}", port.name));
                            self.out_error = err;
                        }
                    }
                }
            }
        }

        self.publish_midi_status();
    }

    fn publish_midi_status(&self) {
        fn status(
            wanted: Option<&crate::project::PortRef>,
            connected: Option<&PortInfo>,
            error: &str,
        ) -> MidiStatus {
            match (connected, wanted) {
                (Some(port), _) => MidiStatus {
                    state: MidiConnState::Connected,
                    id: port.id.clone(),
                    name: port.name.clone(),
                    detail: String::new(),
                },
                (None, None) => MidiStatus::default(),
                (None, Some(want)) => MidiStatus {
                    state: if error.is_empty() {
                        MidiConnState::Missing
                    } else {
                        MidiConnState::Error
                    },
                    id: String::new(),
                    name: want.name.clone(),
                    detail: error.to_string(),
                },
            }
        }
        let midi_in = status(
            self.project.midi.input.as_ref(),
            self.conn_in.as_ref().map(|(_, p)| p),
            &self.in_error,
        );
        let midi_out = status(
            self.project.midi.output.as_ref(),
            self.conn_out.as_ref(),
            &self.out_error,
        );
        self.shared.update(|s| {
            s.midi_in = midi_in;
            s.midi_out = midi_out;
        });
    }

    // -- Health banner --------------------------------------------------------

    /// Log the "Connections active / broken" line when the overall state flips.
    fn check_health(&mut self) {
        let snap = self.shared.snapshot();
        let broken =
            snap.osc_listener.state == ServiceState::Error || snap.mqtt.state == MqttConnState::Error;
        let settled = matches!(
            snap.mqtt.state,
            MqttConnState::Disabled | MqttConnState::Connected
        );
        if broken {
            if self.health != Some(false) {
                self.health = Some(false);
                self.shared
                    .error("Connections broken | Program tries reconnecting");
            }
        } else if settled && self.health != Some(true) {
            let first = self.health.is_none();
            self.health = Some(true);
            self.shared
                .log(Level::Success, "Connections active | Program started");
            if first {
                self.shared
                    .log(Level::Accent, "Type 'help' for commands, 'exit' to quit");
            }
        }
    }

    // -- Shutdown -------------------------------------------------------------

    fn shutdown(&mut self) {
        self.shared
            .debug_log(|| "[SHUTDOWN] Stopping services".to_string());
        self.conn_in = None;
        let _ = self.fwd_tx.send(FwdMsg::Stop);
        if let Some(join) = self.fwd_join.take() {
            let _ = join.join();
        }
        if let Some(listener) = self.listener.take() {
            listener.stop();
        }
        if let Some(mqtt) = self.mqtt.take() {
            mqtt.stop();
        }
    }
}
