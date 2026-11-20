//! The MQTT client thread. It can be stopped and started again at any time.
//!
//! Incoming commands become [`EngineCmd`]s for the supervisor. State is
//! published by the supervisor through [`MqttHandle`] when it changes, so
//! Home Assistant sees changes from the console, OSC and the UI immediately.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rumqttc::{Client, Connection, Event, LastWill, MqttOptions, Outgoing, Packet, QoS};

use super::topics::{self, Topics};
use crate::project::Project;
use crate::state::{LastCommand, MqttConnState, Shared};
use crate::{EngineCmd, Source};

const KEEP_ALIVE: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(200);
const RECONNECT_DELAY: Duration = Duration::from_secs(1);
/// After connecting, wait this long for retained discovery topics before removing stale ones.
const CLEANUP_WINDOW: Duration = Duration::from_millis(1200);

/// Reports from the client thread to the supervisor.
#[derive(Debug)]
pub enum MqttEvent {
    Status(MqttConnState, String),
    /// Connected and set up; the number of discovery entities published.
    Ready(usize),
    /// A command arrived; text for the "last command" display.
    Command(LastCommand),
}

pub struct MqttHandle {
    stop: Arc<AtomicBool>,
    client: Client,
    topics: Arc<Topics>,
    join: Option<JoinHandle<()>>,
}

impl MqttHandle {
    /// `generation` tags every report so the supervisor can ignore reports
    /// from a client it has already replaced.
    pub fn start(
        generation: u64,
        project: Arc<Project>,
        shared: Arc<Shared>,
        cmd_tx: Sender<EngineCmd>,
    ) -> MqttHandle {
        let settings = &project.mqtt;
        let controls = &project.osc.sending_addresses;
        let topics = Arc::new(Topics::new(&settings.base_topic, controls));

        let mut options = MqttOptions::new(
            topics::CLIENT_ID,
            settings.broker_host.trim(),
            settings.broker_port,
        );
        options.set_keep_alive(KEEP_ALIVE);
        if !settings.username.is_empty() {
            options.set_credentials(settings.username.clone(), settings.password.clone());
        }
        options.set_last_will(LastWill::new(
            topics.availability.clone(),
            "offline",
            QoS::AtLeastOnce,
            true,
        ));

        // Everything queued on connect (subscriptions, discovery, states) must fit
        // without blocking: the thread that fills the queue is the one that drains it.
        let capacity = 64 + 8 * controls.len();
        let (client, connection) = Client::new(options, capacity);

        let stop = Arc::new(AtomicBool::new(false));
        let worker = Worker {
            generation,
            client: client.clone(),
            topics: topics.clone(),
            project,
            shared,
            cmd_tx,
            stop: stop.clone(),
        };
        let join = thread::Builder::new()
            .name("mqtt".into())
            .spawn(move || worker.run(connection))
            .expect("spawn mqtt thread");
        MqttHandle {
            stop,
            client,
            topics,
            join: Some(join),
        }
    }

    fn publish(&self, topic: &str, payload: impl Into<Vec<u8>>) {
        // Never block the supervisor: if the queue is full the state is
        // published again on the next change or reconnect.
        let _ = self.client.try_publish(topic, QoS::AtLeastOnce, true, payload);
    }

    pub fn publish_transpose(&self, value: i32) {
        self.publish(&self.topics.transpose_state, value.to_string());
    }

    pub fn publish_osc_sending(&self, on: bool) {
        self.publish(&self.topics.osc_sending_state, topics::bool_payload(on));
    }

    pub fn publish_send_original(&self, on: bool) {
        self.publish(&self.topics.send_original_state, topics::bool_payload(on));
    }

    pub fn publish_debug(&self, on: bool) {
        self.publish(&self.topics.debug_state, topics::bool_payload(on));
    }

    pub fn publish_custom(&self, index: usize, payload: String) {
        if let Some(topic) = self.topics.custom_state.get(index) {
            self.publish(topic, payload);
        }
    }

    /// Announce "offline", disconnect and wait for the thread to finish.
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

struct Worker {
    generation: u64,
    client: Client,
    topics: Arc<Topics>,
    project: Arc<Project>,
    shared: Arc<Shared>,
    cmd_tx: Sender<EngineCmd>,
    stop: Arc<AtomicBool>,
}

impl Worker {
    fn report(&self, event: MqttEvent) {
        let _ = self.cmd_tx.send(EngineCmd::Mqtt(self.generation, event));
    }

    fn stopping(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    fn publish(&self, topic: &str, payload: impl Into<Vec<u8>>) {
        if let Err(e) = self.client.try_publish(topic, QoS::AtLeastOnce, true, payload) {
            self.shared
                .debug_log(|| format!("[MQTT] Publish to {topic} dropped: {e}"));
        }
    }

    fn run(self, mut connection: Connection) {
        let broker = format!(
            "{}:{}",
            self.project.mqtt.broker_host.trim(),
            self.project.mqtt.broker_port
        );
        self.report(MqttEvent::Status(MqttConnState::Connecting, String::new()));
        self.shared.debug_log(|| format!("[MQTT] Connecting to {broker}"));

        let mut connected = false;
        let mut failing = false;
        let mut cleanup_at: Option<Instant> = None;
        let mut observed: HashSet<String> = HashSet::new();

        while !self.stopping() {
            match connection.recv_timeout(POLL) {
                Ok(Ok(Event::Incoming(Packet::ConnAck(_)))) => {
                    connected = true;
                    failing = false;
                    observed.clear();
                    let entities = self.on_connect();
                    cleanup_at = Some(Instant::now() + CLEANUP_WINDOW);
                    self.report(MqttEvent::Ready(entities));
                    self.shared
                        .debug_log(|| format!("[MQTT] Connected to {broker}, {entities} entities announced"));
                }
                Ok(Ok(Event::Incoming(Packet::Publish(publish)))) => {
                    if topics::is_custom_discovery_topic(&publish.topic) && !publish.payload.is_empty() {
                        observed.insert(publish.topic.clone());
                    }
                    self.on_message(&publish.topic, &publish.payload);
                }
                Ok(Ok(_)) => {}
                Ok(Err(err)) => {
                    connected = false;
                    cleanup_at = None;
                    if !failing {
                        failing = true;
                        let detail = err.to_string();
                        self.shared
                            .error(format!("[MQTT] Connection error: {detail} (retrying)"));
                        self.report(MqttEvent::Status(MqttConnState::Error, detail));
                    }
                    // Wait before the next attempt, but stay responsive to stop.
                    let until = Instant::now() + RECONNECT_DELAY;
                    while Instant::now() < until && !self.stopping() {
                        thread::sleep(Duration::from_millis(50));
                    }
                }
                // Poll timeout: nothing happened.
                Err(_) => {}
            }

            if cleanup_at.is_some_and(|at| Instant::now() >= at) {
                cleanup_at = None;
                self.remove_stale_entities(&mut observed);
            }
        }

        if connected {
            self.say_goodbye(&mut connection);
        }
        self.shared.debug_log(|| "[MQTT] Client stopped".to_string());
    }

    /// Subscribe, announce the entities and publish the current state.
    fn on_connect(&self) -> usize {
        for topic in self.topics.subscriptions() {
            if let Err(e) = self.client.try_subscribe(topic, QoS::AtLeastOnce) {
                self.shared
                    .error(format!("[MQTT] Subscription to {topic} failed: {e}"));
            }
        }
        let docs = topics::discovery(&self.project, &self.topics);
        let entities = docs.len();
        for (topic, doc) in docs {
            self.publish(&topic, doc);
        }
        self.publish(&self.topics.availability, "online");

        let snap = self.shared.snapshot();
        self.publish(
            &self.topics.transpose_state,
            self.shared.transpose.load(Ordering::Relaxed).to_string(),
        );
        self.publish(
            &self.topics.osc_sending_state,
            topics::bool_payload(snap.osc_sending),
        );
        self.publish(
            &self.topics.send_original_state,
            topics::bool_payload(snap.send_original),
        );
        self.publish(&self.topics.debug_state, topics::bool_payload(snap.debug));
        for (idx, control) in self.project.osc.sending_addresses.iter().enumerate() {
            let value = snap
                .custom_values
                .get(idx)
                .copied()
                .unwrap_or_else(|| control.initial());
            self.publish(
                &self.topics.custom_state[idx],
                topics::custom_payload(control, value),
            );
        }
        entities
    }

    fn on_message(&self, topic: &str, payload: &[u8]) {
        let t = &self.topics;
        let text = || String::from_utf8_lossy(payload).trim().to_string();
        let cmd = if topic == t.transpose_set {
            match topics::parse_transpose(payload) {
                Some(v) => Some((
                    EngineCmd::SetTranspose(v, Source::Mqtt),
                    LastCommand::Transpose { value: v },
                )),
                None => {
                    self.shared
                        .warn(format!("[MQTT] Invalid transpose payload: {:?}", text()));
                    None
                }
            }
        } else if topic == t.transpose_up {
            topics::parse_bool(payload)
                .then_some((EngineCmd::Nudge(1, Source::Mqtt), LastCommand::TransposeUp))
        } else if topic == t.transpose_down {
            topics::parse_bool(payload)
                .then_some((EngineCmd::Nudge(-1, Source::Mqtt), LastCommand::TransposeDown))
        } else if topic == t.osc_sending_set {
            let on = topics::parse_bool(payload);
            Some((
                EngineCmd::SetOscSending(on, Source::Mqtt),
                LastCommand::OscSending { on },
            ))
        } else if topic == t.send_original_set {
            let on = topics::parse_bool(payload);
            Some((
                EngineCmd::SetSendOriginal(on, Source::Mqtt),
                LastCommand::SendOriginal { on },
            ))
        } else if topic == t.debug_set {
            let on = topics::parse_bool(payload);
            Some((EngineCmd::SetDebug(on, Source::Mqtt), LastCommand::Debug { on }))
        } else if let Some(index) = t.custom_set.iter().position(|s| s == topic) {
            let control = &self.project.osc.sending_addresses[index];
            let value = topics::parse_custom(control, payload);
            Some((
                EngineCmd::CustomOsc {
                    index,
                    value,
                    source: Source::Mqtt,
                },
                LastCommand::Custom {
                    name: control.name.clone(),
                    value: topics::custom_payload(control, value),
                },
            ))
        } else {
            None
        };
        if let Some((cmd, description)) = cmd {
            self.report(MqttEvent::Command(description));
            let _ = self.cmd_tx.send(cmd);
        }
    }

    /// Delete Home Assistant entities of custom controls that no longer exist.
    fn remove_stale_entities(&self, observed: &mut HashSet<String>) {
        let expected: HashSet<String> = self
            .project
            .osc
            .sending_addresses
            .iter()
            .map(topics::custom_discovery_topic)
            .collect();
        for topic in observed.drain() {
            if !expected.contains(&topic) {
                self.shared
                    .debug_log(|| format!("[MQTT] Removing stale Home Assistant entity: {topic}"));
                // An empty retained payload deletes the entity.
                self.publish(&topic, Vec::new());
            }
        }
    }

    /// A clean disconnect does not trigger the last will, so say "offline" explicitly.
    fn say_goodbye(&self, connection: &mut Connection) {
        self.publish(&self.topics.availability, "offline");
        let _ = self.client.try_disconnect();
        let deadline = Instant::now() + Duration::from_millis(600);
        while Instant::now() < deadline {
            match connection.recv_timeout(Duration::from_millis(50)) {
                Ok(Ok(Event::Outgoing(Outgoing::Disconnect))) | Ok(Err(_)) => break,
                _ => {}
            }
        }
    }
}
