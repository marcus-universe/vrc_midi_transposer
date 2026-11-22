//! State shared between the supervisor, the services and the front ends.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU16, AtomicU32, AtomicU8, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::midi::pattern::{ClockState, RecState, Transport};
use crate::project::{MidiTrigger, PatternAction, PatternMode, Project, Step};

fn pack_transport(t: &Transport) -> u64 {
    let clock = match t.clock {
        ClockState::None => 0u64,
        ClockState::External => 1,
        ClockState::Internal => 2,
    };
    clock
        | (t.bpm_x10 as u64) << 2
        | (t.beat as u64 & 3) << 18
        | (t.playing as u64) << 20
        | (t.armed as u64) << 21
        | ((t.step + 1) as u16 as u64) << 22
        | (t.rec as u64) << 38
}

fn unpack_transport(v: u64) -> Transport {
    Transport {
        clock: match v & 3 {
            1 => ClockState::External,
            2 => ClockState::Internal,
            _ => ClockState::None,
        },
        bpm_x10: (v >> 2) as u16,
        beat: ((v >> 18) & 3) as u8,
        playing: (v >> 20) & 1 == 1,
        armed: (v >> 21) & 1 == 1,
        step: ((v >> 22) as u16) as i16 - 1,
        rec: match (v >> 38) & 3 {
            1 => RecState::Step,
            2 => RecState::Waiting,
            3 => RecState::Live,
            _ => RecState::Off,
        },
    }
}

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

/// What the last command received over MQTT did. The UI words it in the user's language.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LastCommand {
    Transpose {
        value: i32,
    },
    TransposeUp,
    TransposeDown,
    OscSending {
        on: bool,
    },
    SendOriginal {
        on: bool,
    },
    Debug {
        on: bool,
    },
    /// A custom control; `value` is the state payload (`1`/`0` or a number).
    Custom {
        name: String,
        value: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct MqttStatus {
    pub state: MqttConnState,
    pub detail: String,
    pub entities: usize,
    pub last_command: Option<LastCommand>,
    pub last_command_ts: u64,
}

/// Result of MIDI Learn. `seq` grows with every result, so a front end can tell a new one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Learned {
    pub seq: u64,
    pub index: usize,
    pub trigger: MidiTrigger,
}

/// Result of MIDI Learn for an arpeggiator / sequencer action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PatternLearned {
    pub seq: u64,
    pub action: PatternAction,
    pub trigger: MidiTrigger,
}

/// Steps recorded into the sequencer, for the front end to put into the project.
/// `seq` grows with every result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SeqRecorded {
    pub seq: u64,
    pub steps: Vec<Step>,
    pub length: u8,
    /// Recording ended.
    pub done: bool,
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
    /// The separate MIDI clock device, when the clock source is a device.
    pub clock_device: MidiStatus,
    pub osc_listener: ListenerStatus,
    pub osc_target: String,
    pub mqtt: MqttStatus,
    /// Custom control waiting for MIDI Learn, if any.
    pub learning: Option<usize>,
    /// Last MIDI Learn result.
    pub learned: Option<Learned>,
    /// Arpeggiator / sequencer action waiting for MIDI Learn, if any.
    pub pattern_learning: Option<PatternAction>,
    pub pattern_learned: Option<PatternLearned>,
    /// The pattern mode playing now. Switching by MIDI changes it for the
    /// session; the project keeps its own.
    pub pattern_mode: PatternMode,
    /// Last recording result.
    pub seq_recorded: Option<SeqRecorded>,
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
    /// Arpeggiator / sequencer display state, packed (see `set_transport`).
    transport: std::sync::atomic::AtomicU64,
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
            transport: std::sync::atomic::AtomicU64::new(pack_transport(&Transport::default())),
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

    pub fn set_transport(&self, t: Transport) {
        self.transport.store(pack_transport(&t), Ordering::Relaxed);
    }

    pub fn transport(&self) -> Transport {
        unpack_transport(self.transport.load(Ordering::Relaxed))
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

    #[test]
    fn transport_round_trips() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let shared = Shared::new(tx, Arc::new(Project::default()));
        let t = Transport {
            clock: ClockState::Internal,
            bpm_x10: 1205,
            beat: 3,
            playing: true,
            armed: false,
            step: 63,
            rec: RecState::Live,
        };
        shared.set_transport(t);
        assert_eq!(shared.transport(), t);
        let idle = Transport {
            step: -1,
            ..Transport::default()
        };
        shared.set_transport(idle);
        assert_eq!(shared.transport(), idle);
    }
}
