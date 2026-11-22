//! Engine of VRC MIDI Transposer: MIDI forwarding with live transposition,
//! OSC in and out, MQTT with Home Assistant discovery, projects, and the
//! console. It has no GUI dependency; the Tauri shell and the headless front
//! end both drive it through [`Engine`].

pub mod command;
pub mod headless;
pub mod info;
pub mod project;
pub mod state;

mod midi;
mod mqtt;
mod osc;
mod supervisor;

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

pub use midi::mapping::ControlEvent;
pub use midi::pattern::{ClockState, Transport};
pub use mqtt::MqttEvent;
pub use project::{Project, Store};
pub use state::{EngineEvent, Level, LogLine, Snapshot};

use state::{lock, Shared};

/// Who asked for a change. Decides what is logged and echoed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Ui,
    Console,
    Osc,
    Mqtt,
    /// A custom control triggered by a MIDI message.
    Midi,
}

/// Everything that can be asked of the engine. Processed in order by the supervisor.
pub enum EngineCmd {
    SetTranspose(i32, Source),
    /// Change transpose by a relative amount.
    Nudge(i32, Source),
    SetOscSending(bool, Source),
    SetSendOriginal(bool, Source),
    SetDebug(bool, Source),
    /// Start or stop the MQTT client for this session.
    SetMqttEnabled(bool),
    /// Set a custom control (index into `project.osc.sending_addresses`).
    CustomOsc {
        index: usize,
        value: f32,
        source: Source,
    },
    /// MIDI Learn for custom control `index`: the next Note On, CC or Program
    /// Change becomes its trigger (reported in `Snapshot::learned`). `None` cancels.
    MidiLearn(Option<usize>),
    /// Start (on the next bar of the clock) or stop the sequencer.
    SeqPlay(bool),
    /// Run a console command line.
    Console(String),
    /// Add a line to the log.
    Log(Level, String),
    ClearLog,
    ListMidi,
    RefreshPorts,
    /// Release every held note.
    Panic,
    /// Open a project: startup values are applied.
    LoadProject(Box<Project>),
    /// Apply edits to the open project; only affected services restart.
    UpdateProject(Box<Project>),
    Shutdown,

    // Reports from the engine's own threads.
    #[doc(hidden)]
    OutputLost(String),
    #[doc(hidden)]
    Mqtt(u64, MqttEvent),
    #[doc(hidden)]
    MidiControl {
        generation: u64,
        event: ControlEvent,
    },
}

/// Handle to a running engine.
pub struct Engine {
    shared: Arc<Shared>,
    cmd_tx: Sender<EngineCmd>,
    join: Mutex<Option<JoinHandle<()>>>,
}

impl Engine {
    /// Start the engine with a project. Events arrive on the returned receiver.
    pub fn start(project: Project) -> (Engine, Receiver<EngineEvent>) {
        let (event_tx, event_rx) = mpsc::channel();
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let shared = Arc::new(Shared::new(event_tx, Arc::new(project.normalized())));
        let supervisor = supervisor::Supervisor::new(shared.clone(), cmd_tx.clone());
        let join = thread::Builder::new()
            .name("supervisor".into())
            .spawn(move || supervisor.run(cmd_rx))
            .expect("spawn supervisor thread");
        (
            Engine {
                shared,
                cmd_tx,
                join: Mutex::new(Some(join)),
            },
            event_rx,
        )
    }

    pub fn send(&self, cmd: EngineCmd) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn snapshot(&self) -> Snapshot {
        self.shared.snapshot()
    }

    /// The project the engine is running, as last applied.
    pub fn project(&self) -> Arc<Project> {
        self.shared.project()
    }

    /// Log lines kept for front ends that attach late.
    pub fn logs(&self) -> Vec<LogLine> {
        self.shared.logs()
    }

    /// Pressed keys: words 0-3 input notes, words 4-7 output notes.
    pub fn keys(&self) -> [u32; 8] {
        self.shared.keys()
    }

    /// Velocities of the pressed keys in `keys`, in bit order (input words first).
    pub fn velocities(&self, keys: &[u32; 8]) -> Vec<u8> {
        self.shared.velocities(keys)
    }

    /// Arpeggiator / sequencer state for the display.
    pub fn transport(&self) -> Transport {
        self.shared.transport()
    }

    /// `[pitch bend 0..=16383 (8192 = centre), mod wheel 0..=127]`.
    pub fn wheels(&self) -> [u16; 2] {
        self.shared.wheels()
    }

    /// Like [`Engine::keys`], and re-arms the [`EngineEvent::Keys`] notification.
    pub fn take_keys(&self) -> [u32; 8] {
        self.shared.take_keys()
    }

    /// Stop every service and wait for the engine to finish. Idempotent.
    pub fn shutdown(&self) {
        let join = lock(&self.join).take();
        if let Some(join) = join {
            let _ = self.cmd_tx.send(EngineCmd::Shutdown);
            let _ = join.join();
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.shutdown();
    }
}
