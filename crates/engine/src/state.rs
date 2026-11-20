//! State shared between the supervisor, the services and the front ends.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU16, AtomicU32, AtomicU8, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::project::Project;

/// Pitch bend value of a centred wheel.
pub const PITCH_CENTRE: u16 = 8192;

/// Number of log lines kept for front ends that attach late.
pub const LOG_CAPACITY: usize = 2000;

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Lock a mutex, recovering the data if another thread panicked while holding it.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Info,
    Debug,
    Warn,
    Error,
    Success,
    Accent,
}

#[derive(Clone, Debug, Serialize)]
pub struct LogLine {
    pub id: u64,
    pub ts: u64,
    pub level: Level,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct PortInfo {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MidiConnState {
    /// Nothing selected.
    #[default]
    None,
    Connected,
    /// Selected, but the device is not present.
    Missing,
    Error,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct MidiStatus {
    pub state: MidiConnState,
    /// Id of the connected port, empty unless connected.
    pub id: String,
    pub name: String,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    #[default]
    Stopped,
    Running,
    Error,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ListenerStatus {
    pub state: ServiceState,
    pub detail: String,
    pub bound: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MqttConnState {
    #[default]
    Disabled,
    Connecting,
    Connected,
    Error,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct MqttStatus {
    pub state: MqttConnState,
    pub detail: String,
    pub entities: usize,
    pub last_command: String,
    pub last_command_ts: u64,
}

/// Everything a front end needs to draw the current state. Sent whole, never as a delta.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Snapshot {
    pub transpose: i32,
    pub transpose_min: i32,
    pub transpose_max: i32,
    pub debug: bool,
    pub osc_sending: bool,
    pub send_original: bool,
    pub mqtt_enabled: bool,
    pub custom_values: Vec<f32>,
    pub inputs: Vec<PortInfo>,
    pub outputs: Vec<PortInfo>,
    pub midi_in: MidiStatus,
    pub midi_out: MidiStatus,
    pub osc_listener: ListenerStatus,
    pub osc_target: String,
    pub mqtt: MqttStatus,
}

/// Notifications for the front end. Wake-up tokens: the data is read from [`Shared`].
#[derive(Clone, Debug)]
pub enum EngineEvent {
    Log(LogLine),
    /// The snapshot changed.
    Changed,
    /// The pressed-key bitmap changed.
    Keys,
    LogCleared,
    /// `exit` was typed in the console.
    ExitRequested,
}

struct LogBuf {
    next_id: u64,
    lines: VecDeque<LogLine>,
}

pub struct Shared {
    /// Current transpose in semitones; read by the forwarder on every note.
    pub transpose: AtomicI32,
    pub debug: AtomicBool,
    /// Pressed keys: words 0-3 input notes, words 4-7 output notes.
    keys: [AtomicU32; 8],
    /// Velocity of each pressed key: 0-127 input notes, 128-255 output notes.
    vel: [AtomicU8; 256],
    /// Last pitch bend, 0..=16383, 8192 = centre.
    pitch: AtomicU16,
    /// Last mod wheel (CC 1) value from any channel.
    modw: AtomicU8,
    keys_dirty: AtomicBool,
    events: Sender<EngineEvent>,
    log: Mutex<LogBuf>,
    snapshot: Mutex<Snapshot>,
    project: Mutex<Arc<Project>>,
}

impl Shared {
    pub fn new(events: Sender<EngineEvent>, project: Arc<Project>) -> Self {
        Shared {
            transpose: AtomicI32::new(0),
            debug: AtomicBool::new(false),
            keys: Default::default(),
            vel: std::array::from_fn(|_| AtomicU8::new(0)),
            pitch: AtomicU16::new(PITCH_CENTRE),
            modw: AtomicU8::new(0),
            keys_dirty: AtomicBool::new(false),
            events,
            log: Mutex::new(LogBuf {
                next_id: 1,
                lines: VecDeque::with_capacity(256),
            }),
            snapshot: Mutex::new(Snapshot::default()),
            project: Mutex::new(project),
        }
    }

    pub fn emit(&self, event: EngineEvent) {
        let _ = self.events.send(event);
    }

    pub fn is_debug(&self) -> bool {
        self.debug.load(Ordering::Relaxed)
    }

    pub fn log(&self, level: Level, text: impl Into<String>) {
        let line = {
            let mut buf = lock(&self.log);
            let line = LogLine {
                id: buf.next_id,
                ts: now_ms(),
                level,
                text: text.into(),
            };
            buf.next_id += 1;
            if buf.lines.len() >= LOG_CAPACITY {
                buf.lines.pop_front();
            }
            buf.lines.push_back(line.clone());
            line
        };
        self.emit(EngineEvent::Log(line));
    }

    pub fn info(&self, text: impl Into<String>) {
        self.log(Level::Info, text);
    }

    pub fn warn(&self, text: impl Into<String>) {
        self.log(Level::Warn, text);
    }

    pub fn error(&self, text: impl Into<String>) {
        self.log(Level::Error, text);
    }

    /// Log only in debug mode. The closure is not run otherwise.
    pub fn debug_log(&self, text: impl FnOnce() -> String) {
        if self.is_debug() {
            self.log(Level::Debug, text());
        }
    }

    pub fn logs(&self) -> Vec<LogLine> {
        lock(&self.log).lines.iter().cloned().collect()
    }

    pub fn clear_log(&self) {
        lock(&self.log).lines.clear();
        self.emit(EngineEvent::LogCleared);
    }

    pub fn snapshot(&self) -> Snapshot {
        lock(&self.snapshot).clone()
    }

    /// Change the snapshot; front ends are notified only if something changed.
    pub fn update(&self, f: impl FnOnce(&mut Snapshot)) {
        let changed = {
            let mut snap = lock(&self.snapshot);
            let before = snap.clone();
            f(&mut snap);
            *snap != before
        };
        if changed {
            self.emit(EngineEvent::Changed);
        }
    }

    pub fn project(&self) -> Arc<Project> {
        lock(&self.project).clone()
    }

    pub fn set_project(&self, project: Arc<Project>) {
        *lock(&self.project) = project;
    }

    /// `half` 0 = input notes, 1 = output notes. `velocity` 0 releases the key.
    pub fn set_key(&self, half: usize, note: u8, velocity: u8) {
        let note = (note & 0x7F) as usize;
        self.vel[half * 128 + note].store(velocity & 0x7F, Ordering::Relaxed);
        let word = &self.keys[half * 4 + (note >> 5)];
        let bit = 1u32 << (note & 31);
        if velocity > 0 {
            word.fetch_or(bit, Ordering::Relaxed);
        } else {
            word.fetch_and(!bit, Ordering::Relaxed);
        }
    }

    pub fn set_pitch(&self, value: u16) {
        self.pitch.store(value.min(16383), Ordering::Relaxed);
    }

    pub fn set_mod_wheel(&self, value: u8) {
        self.modw.store(value & 0x7F, Ordering::Relaxed);
    }

    /// `[pitch bend 0..=16383, mod wheel 0..=127]`.
    pub fn wheels(&self) -> [u16; 2] {
        [
            self.pitch.load(Ordering::Relaxed),
            self.modw.load(Ordering::Relaxed) as u16,
        ]
    }

    /// Velocities of the pressed keys in `keys`, in bit order (input words first).
    pub fn velocities(&self, keys: &[u32; 8]) -> Vec<u8> {
        let mut out = Vec::new();
        for (w, word) in keys.iter().enumerate() {
            let mut bits = *word;
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let index = (w / 4) * 128 + (w % 4) * 32 + bit;
                // Never report 0 for a pressed key; 0 means released.
                out.push(self.vel[index].load(Ordering::Relaxed).max(1));
            }
        }
        out
    }

    /// Tell the front end the key bitmap changed. Emits at most one token until it is read.
    pub fn mark_keys_dirty(&self) {
        if !self.keys_dirty.swap(true, Ordering::AcqRel) {
            self.emit(EngineEvent::Keys);
        }
    }

    pub fn keys(&self) -> [u32; 8] {
        std::array::from_fn(|i| self.keys[i].load(Ordering::Relaxed))
    }

    /// Read the key bitmap and re-arm the change notification.
    pub fn take_keys(&self) -> [u32; 8] {
        self.keys_dirty.store(false, Ordering::Release);
        self.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn velocities_follow_the_bit_order() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let shared = Shared::new(tx, Arc::new(Project::default()));
        shared.set_key(0, 60, 100);
        shared.set_key(0, 40, 20);
        shared.set_key(1, 67, 90);
        let keys = shared.keys();
        assert_eq!(shared.velocities(&keys), [20, 100, 90]);
        shared.set_key(0, 40, 0);
        let keys = shared.keys();
        assert_eq!(shared.velocities(&keys), [100, 90]);
        assert_eq!(shared.wheels(), [PITCH_CENTRE, 0]);
    }
}
