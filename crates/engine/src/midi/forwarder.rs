//! The forwarder thread: receives MIDI from the input callback, transposes
//! notes, writes to the output, mirrors notes to OSC and tracks pressed keys.
//!
//! This is the hot path. No locks, logging or allocation per message outside
//! debug mode.

use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use midir::MidiOutputConnection;

use super::held_notes::{transposed, HeldNotes};
use super::mapping::{ControlEvent, LearnTarget, Mapping, Mappings};
use super::pattern::{ClockMsg, ClockOrigin, Gen, PatternEngine, Recorded, Transport};
use super::MidiBytes;
use crate::osc::out::OscOut;
use crate::project::{MidiTrigger, PatternAction, PatternSettings};
use crate::state::Shared;
use crate::EngineCmd;

pub enum FwdMsg {
    Midi(MidiBytes),
    /// Replace the output. Held notes are released on the old one first.
    SetOutput(Option<MidiOutputConnection>),
    /// Change OSC note sending. Held keys are released over OSC first.
    SetOsc {
        enabled: bool,
        original: bool,
    },
    /// Release everything: Note Off for held notes, OSC keys to 0, clear the key display.
    Panic,
    /// Custom controls triggered by MIDI. `generation` tags the events they produce.
    SetMappings(Arc<[Mapping]>, u64),
    /// MIDI triggers of the arpeggiator / sequencer.
    SetPatternMappings(Arc<[(PatternAction, MidiTrigger)]>),
    /// MIDI Learn for a custom control or a pattern action; `None` cancels.
    Learn(Option<LearnTarget>),
    /// A clock message from the clock device or the internal tempo.
    Clock(ClockOrigin, ClockMsg),
    /// Arpeggiator / sequencer settings.
    SetPattern(Arc<PatternSettings>),
    /// Start (waits for the next bar) or stop the sequencer.
    SeqPlay(bool),
    /// Start or end recording into the sequencer.
    SeqRecord(bool),
    /// Step recording: leave the next step empty.
    SeqRest,
    Stop,
}

/// Where a note goes, split out from the thread loop so it can be tested.
pub trait Sink {
    fn midi(&mut self, bytes: &[u8]);
    fn osc_note(&mut self, note: u8, on: bool);
    fn osc_pitch_bend(&mut self, lsb: u8, msb: u8);
    /// `half` 0 = input, 1 = output. `velocity` 0 releases the key.
    fn key(&mut self, half: usize, note: u8, velocity: u8);
    /// Pitch bend 0..=16383 (8192 = centre) and mod wheel 0..=127, for display.
    fn wheels(&mut self, pitch: Option<u16>, modwheel: Option<u8>);
    /// A custom control was triggered by MIDI (or learned).
    fn control(&mut self, generation: u64, event: ControlEvent);
    /// Arpeggiator / sequencer state for the display.
    fn transport(&mut self, transport: Transport);
    /// Steps recorded into the sequencer.
    fn recorded(&mut self, recorded: Recorded);
}

pub struct Core {
    held: HeldNotes,
    in_count: [u8; 128],
    out_count: [u8; 128],
    osc_on: [bool; 128],
    osc_enabled: bool,
    osc_original: bool,
    mappings: Mappings,
    generation: u64,
    pattern: PatternEngine,
    /// Output notes of the arpeggiator / sequencer, like `held` for played keys.
    gen_held: HeldNotes,
    /// Notes the pattern produced, applied right after each call. Reused.
    gen_buf: Vec<Gen>,
    last_transport: Transport,
}

impl Default for Core {
    fn default() -> Self {
        Core {
            held: HeldNotes::default(),
            in_count: [0; 128],
            out_count: [0; 128],
            osc_on: [false; 128],
            osc_enabled: false,
            osc_original: true,
            mappings: Mappings::default(),
            generation: 0,
            pattern: PatternEngine::new(Arc::new(PatternSettings::default())),
            gen_held: HeldNotes::default(),
            gen_buf: Vec::with_capacity(256),
            last_transport: Transport {
                step: -1,
                ..Transport::default()
            },
        }
    }
}

impl Core {
    fn press(&mut self, sink: &mut impl Sink, input: u8, output: u8, velocity: u8) {
        let (i, o) = (input as usize, output as usize);
        self.in_count[i] = self.in_count[i].saturating_add(1);
        self.out_count[o] = self.out_count[o].saturating_add(1);
        sink.key(0, input, velocity);
        sink.key(1, output, velocity);
        if self.osc_enabled {
            let note = if self.osc_original { input } else { output };
            self.osc_on[note as usize] = true;
            sink.osc_note(note, true);
        }
    }

    fn release(&mut self, sink: &mut impl Sink, input: u8, output: u8) {
        let (i, o) = (input as usize, output as usize);
        self.in_count[i] = self.in_count[i].saturating_sub(1);
        self.out_count[o] = self.out_count[o].saturating_sub(1);
        if self.in_count[i] == 0 {
            sink.key(0, input, 0);
        }
        if self.out_count[o] == 0 {
            sink.key(1, output, 0);
        }
        if self.osc_enabled {
            let note = if self.osc_original { input } else { output };
            self.osc_on[note as usize] = false;
            sink.osc_note(note, false);
        }
    }

    /// Play the notes the pattern produced and update its display state.
    fn apply_gen(&mut self, sink: &mut impl Sink, semitones: i32) {
        for i in 0..self.gen_buf.len() {
            match self.gen_buf[i] {
                Gen::On {
                    channel,
                    note,
                    velocity,
                } => {
                    let (out, previous) = self.gen_held.note_on(channel, note, semitones);
                    if let Some(old) = previous {
                        sink.midi(&[0x80 | channel, old, 0]);
                        self.gen_release(sink, note, old);
                    }
                    sink.midi(&[0x90 | channel, out, velocity]);
                    let o = out as usize;
                    self.out_count[o] = self.out_count[o].saturating_add(1);
                    sink.key(1, out, velocity);
                    if self.osc_enabled {
                        let n = if self.osc_original { note } else { out };
                        self.osc_on[n as usize] = true;
                        sink.osc_note(n, true);
                    }
                }
                Gen::Off { channel, note } => {
                    let out = self.gen_held.note_off(channel, note, semitones);
                    sink.midi(&[0x80 | channel, out, 0]);
                    self.gen_release(sink, note, out);
                }
            }
        }
        self.gen_buf.clear();
        let t = self.pattern.transport();
        if t != self.last_transport {
            self.last_transport = t;
            sink.transport(t);
        }
        if let Some(recorded) = self.pattern.take_recorded() {
            sink.recorded(recorded);
        }
    }

    fn gen_release(&mut self, sink: &mut impl Sink, note: u8, out: u8) {
        let o = out as usize;
        self.out_count[o] = self.out_count[o].saturating_sub(1);
        if self.out_count[o] == 0 {
            sink.key(1, out, 0);
        }
        if self.osc_enabled {
            let n = if self.osc_original { note } else { out };
            self.osc_on[n as usize] = false;
            sink.osc_note(n, false);
        }
    }

    /// A clock message from any source.
    pub fn clock(
        &mut self,
        sink: &mut impl Sink,
        origin: ClockOrigin,
        msg: ClockMsg,
        semitones: i32,
        now: Instant,
    ) {
        if !self.pattern.active() {
            return;
        }
        self.pattern.clock(origin, msg, now, &mut self.gen_buf);
        self.apply_gen(sink, semitones);
    }

    pub fn set_pattern(&mut self, sink: &mut impl Sink, settings: Arc<PatternSettings>, semitones: i32) {
        self.pattern.set_settings(settings, &mut self.gen_buf);
        self.apply_gen(sink, semitones);
    }

    pub fn seq_play(&mut self, sink: &mut impl Sink, on: bool, semitones: i32) {
        self.pattern.play(on, &mut self.gen_buf);
        self.apply_gen(sink, semitones);
    }

    pub fn seq_record(&mut self, sink: &mut impl Sink, on: bool, semitones: i32) {
        self.pattern.record(on, &mut self.gen_buf);
        self.apply_gen(sink, semitones);
    }

    pub fn seq_rest(&mut self, sink: &mut impl Sink, semitones: i32) {
        self.pattern.rest(&mut self.gen_buf);
        self.apply_gen(sink, semitones);
    }

    /// Process one incoming MIDI message.
    pub fn handle(&mut self, sink: &mut impl Sink, bytes: &[u8], semitones: i32) {
        self.handle_at(sink, bytes, semitones, Instant::now());
    }

    /// Process one incoming MIDI message received at `now`.
    pub fn handle_at(&mut self, sink: &mut impl Sink, bytes: &[u8], semitones: i32, now: Instant) {
        let Some(&status_byte) = bytes.first() else {
            return;
        };
        let kind = status_byte & 0xF0;
        let channel = status_byte & 0x0F;
        if !self.mappings.is_idle() {
            let generation = self.generation;
            // Play, Record and Rest act here right away; everything else goes to the supervisor.
            let (mut play, mut record, mut rest) = (false, false, false);
            let swallow = self.mappings.process(bytes, |event| match event {
                ControlEvent::Pattern(PatternAction::Play) => play = !play,
                ControlEvent::Pattern(PatternAction::Record) => record = !record,
                ControlEvent::Pattern(PatternAction::Rest) => rest = true,
                _ => sink.control(generation, event),
            });
            if play || record || rest {
                if record {
                    self.pattern.toggle_record(&mut self.gen_buf);
                }
                if play {
                    self.pattern.toggle_play(&mut self.gen_buf);
                }
                if rest {
                    self.pattern.rest(&mut self.gen_buf);
                }
                self.apply_gen(sink, semitones);
            }
            if swallow {
                // A key that is still sounding must always be released, even if
                // its Note Off is now mapped to a control.
                let note = bytes.get(1).copied().unwrap_or(0) & 0x7F;
                let release = kind == 0x80 || (kind == 0x90 && bytes.get(2).copied().unwrap_or(0) == 0);
                if !(release && self.held.get(channel, note).is_some()) {
                    return;
                }
            }
        }
        if let Some(msg) = ClockMsg::parse(bytes) {
            // MIDI clock from the input: forwarded as before, and drives the
            // arpeggiator / sequencer when the input is the clock source.
            sink.midi(bytes);
            self.clock(sink, ClockOrigin::Input, msg, semitones, now);
            return;
        }
        if self.pattern.active() && bytes.len() >= 2 && matches!(kind, 0x80 | 0x90 | 0xA0) {
            let note = bytes[1] & 0x7F;
            let velocity = bytes.get(2).copied().unwrap_or(0);
            let pressed = kind == 0x90 && velocity > 0;
            let released = kind == 0x80 || (kind == 0x90 && velocity == 0);
            // A key that was already sounding before the pattern was switched on
            // still ends normally (below).
            if !(released && self.held.get(channel, note).is_some()) {
                let i = note as usize;
                if pressed {
                    self.in_count[i] = self.in_count[i].saturating_add(1);
                    sink.key(0, note, velocity);
                    self.pattern
                        .note(channel, note, velocity, true, &mut self.gen_buf);
                } else if released {
                    self.in_count[i] = self.in_count[i].saturating_sub(1);
                    if self.in_count[i] == 0 {
                        sink.key(0, note, 0);
                    }
                    self.pattern.note(channel, note, 0, false, &mut self.gen_buf);
                }
                self.apply_gen(sink, semitones);
                // Polyphonic aftertouch has no matching note while a pattern plays.
                return;
            }
        }
        if kind == 0xB0 && bytes.len() >= 3 && bytes[1] == 1 {
            // Mod wheel (CC 1): forwarded unchanged, shown on the wheel.
            sink.midi(bytes);
            sink.wheels(None, Some(bytes[2] & 0x7F));
            return;
        }
        if bytes.len() < 2 || !matches!(kind, 0x80 | 0x90 | 0xA0 | 0xE0) {
            sink.midi(bytes);
            return;
        }
        let note = bytes[1] & 0x7F;
        let velocity = bytes.get(2).copied().unwrap_or(0);
        match kind {
            0x90 if velocity > 0 => {
                let (out, previous) = self.held.note_on(channel, note, semitones);
                if let Some(old) = previous {
                    // Same key struck again after the transpose changed: end the old note.
                    sink.midi(&[0x80 | channel, old, 0]);
                    self.release(sink, note, old);
                }
                sink.midi(&[status_byte, out, velocity]);
                self.press(sink, note, out, velocity);
            }
            0x80 | 0x90 => {
                let out = self.held.note_off(channel, note, semitones);
                sink.midi(&[status_byte, out, velocity]);
                self.release(sink, note, out);
            }
            0xA0 => {
                let out = self
                    .held
                    .get(channel, note)
                    .unwrap_or_else(|| transposed(note, semitones));
                sink.midi(&[status_byte, out, velocity]);
            }
            _ => {
                // Pitch bend: forwarded unchanged, mirrored to OSC and shown on the wheel.
                sink.midi(bytes);
                if bytes.len() >= 3 {
                    let value = ((bytes[2] as u16 & 0x7F) << 7) | (bytes[1] as u16 & 0x7F);
                    sink.wheels(Some(value), None);
                    if self.osc_enabled {
                        sink.osc_pitch_bend(bytes[1], bytes[2]);
                    }
                }
            }
        }
    }

    /// Send Note Off for every held note without forgetting them (the keys are
    /// still physically down). Used before the output is replaced.
    pub fn silence_output(&self, sink: &mut impl Sink) {
        for (channel, out) in self.held.held().into_iter().chain(self.gen_held.held()) {
            sink.midi(&[0x80 | channel, out, 0]);
        }
    }

    fn release_osc(&mut self, sink: &mut impl Sink) {
        for note in 0..128u8 {
            if std::mem::take(&mut self.osc_on[note as usize]) {
                sink.osc_note(note, false);
            }
        }
    }

    pub fn set_mappings(&mut self, list: Arc<[Mapping]>, generation: u64) {
        self.mappings.set(list);
        self.generation = generation;
    }

    pub fn set_pattern_mappings(&mut self, list: Arc<[(PatternAction, MidiTrigger)]>) {
        self.mappings.set_pattern(list);
    }

    pub fn learn(&mut self, target: Option<LearnTarget>) {
        self.mappings.learn(target);
    }

    pub fn set_osc(&mut self, sink: &mut impl Sink, enabled: bool, original: bool) {
        if enabled != self.osc_enabled || original != self.osc_original {
            self.release_osc(sink);
            self.osc_enabled = enabled;
            self.osc_original = original;
        }
    }

    pub fn panic(&mut self, sink: &mut impl Sink) {
        self.pattern.panic(&mut self.gen_buf);
        self.gen_buf.clear();
        for (channel, out) in self.held.drain().into_iter().chain(self.gen_held.drain()) {
            sink.midi(&[0x80 | channel, out, 0]);
        }
        let t = self.pattern.transport();
        if t != self.last_transport {
            self.last_transport = t;
            sink.transport(t);
        }
        self.release_osc(sink);
        for note in 0..128u8 {
            if std::mem::take(&mut self.in_count[note as usize]) > 0 {
                sink.key(0, note, 0);
            }
            if std::mem::take(&mut self.out_count[note as usize]) > 0 {
                sink.key(1, note, 0);
            }
        }
        sink.wheels(Some(crate::state::PITCH_CENTRE), Some(0));
    }
}

struct LiveSink<'a> {
    output: &'a mut Option<MidiOutputConnection>,
    osc: &'a OscOut,
    shared: &'a Shared,
    cmd_tx: &'a Sender<EngineCmd>,
    keys_changed: bool,
}

impl Sink for LiveSink<'_> {
    fn midi(&mut self, bytes: &[u8]) {
        if let Some(conn) = self.output.as_mut() {
            if let Err(err) = conn.send(bytes) {
                // Drop the connection on the first failure; the supervisor reconnects.
                *self.output = None;
                let _ = self.cmd_tx.send(EngineCmd::OutputLost(err.to_string()));
            }
        }
    }

    fn osc_note(&mut self, note: u8, on: bool) {
        self.osc.note(note, on);
    }

    fn osc_pitch_bend(&mut self, lsb: u8, msb: u8) {
        self.osc.pitch_bend(lsb, msb);
    }

    fn key(&mut self, half: usize, note: u8, velocity: u8) {
        self.shared.set_key(half, note, velocity);
        self.keys_changed = true;
    }

    fn wheels(&mut self, pitch: Option<u16>, modwheel: Option<u8>) {
        if let Some(p) = pitch {
            self.shared.set_pitch(p);
        }
        if let Some(m) = modwheel {
            self.shared.set_mod_wheel(m);
        }
        self.keys_changed = true;
    }

    fn control(&mut self, generation: u64, event: ControlEvent) {
        let _ = self.cmd_tx.send(EngineCmd::MidiControl { generation, event });
    }

    fn transport(&mut self, transport: Transport) {
        self.shared.set_transport(transport);
        self.keys_changed = true;
    }

    fn recorded(&mut self, recorded: Recorded) {
        let _ = self.cmd_tx.send(EngineCmd::SeqRecorded(recorded));
    }
}

pub fn spawn(
    rx: Receiver<FwdMsg>,
    shared: Arc<Shared>,
    osc: Arc<OscOut>,
    cmd_tx: Sender<EngineCmd>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("midi-forwarder".into())
        .spawn(move || {
            let mut output: Option<MidiOutputConnection> = None;
            let mut core = Core::default();
            for msg in rx {
                let mut sink = LiveSink {
                    output: &mut output,
                    osc: &osc,
                    shared: &shared,
                    cmd_tx: &cmd_tx,
                    keys_changed: false,
                };
                match msg {
                    FwdMsg::Midi(bytes) => {
                        let semitones = shared.transpose.load(Ordering::Relaxed);
                        core.handle(&mut sink, bytes.as_slice(), semitones);
                    }
                    FwdMsg::Clock(origin, msg) => {
                        let semitones = shared.transpose.load(Ordering::Relaxed);
                        core.clock(&mut sink, origin, msg, semitones, Instant::now());
                    }
                    FwdMsg::SetPattern(settings) => {
                        let semitones = shared.transpose.load(Ordering::Relaxed);
                        core.set_pattern(&mut sink, settings, semitones);
                    }
                    FwdMsg::SeqPlay(on) => {
                        let semitones = shared.transpose.load(Ordering::Relaxed);
                        core.seq_play(&mut sink, on, semitones);
                    }
                    FwdMsg::SeqRecord(on) => {
                        let semitones = shared.transpose.load(Ordering::Relaxed);
                        core.seq_record(&mut sink, on, semitones);
                    }
                    FwdMsg::SeqRest => {
                        let semitones = shared.transpose.load(Ordering::Relaxed);
                        core.seq_rest(&mut sink, semitones);
                    }
                    FwdMsg::SetPatternMappings(list) => core.set_pattern_mappings(list),
                    FwdMsg::SetOutput(new) => {
                        core.silence_output(&mut sink);
                        *sink.output = new;
                    }
                    FwdMsg::SetOsc { enabled, original } => core.set_osc(&mut sink, enabled, original),
                    FwdMsg::Panic => core.panic(&mut sink),
                    FwdMsg::SetMappings(list, generation) => core.set_mappings(list, generation),
                    FwdMsg::Learn(target) => core.learn(target),
                    FwdMsg::Stop => {
                        core.panic(&mut sink);
                        break;
                    }
                }
                if sink.keys_changed {
                    shared.mark_keys_dirty();
                }
            }
        })
        .expect("spawn forwarder thread")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rec {
        midi: Vec<Vec<u8>>,
        osc: Vec<(u8, bool)>,
        bends: Vec<(u8, u8)>,
        keys: [[bool; 128]; 2],
        vel: [[u8; 128]; 2],
        pitch: u16,
        modw: u8,
        controls: Vec<ControlEvent>,
        transport: Transport,
        recorded: Vec<Recorded>,
    }

    impl Default for Rec {
        fn default() -> Self {
            Rec {
                midi: Vec::new(),
                osc: Vec::new(),
                bends: Vec::new(),
                keys: [[false; 128]; 2],
                vel: [[0; 128]; 2],
                pitch: 8192,
                modw: 0,
                controls: Vec::new(),
                transport: Transport::default(),
                recorded: Vec::new(),
            }
        }
    }

    impl Sink for Rec {
        fn midi(&mut self, bytes: &[u8]) {
            self.midi.push(bytes.to_vec());
        }
        fn osc_note(&mut self, note: u8, on: bool) {
            self.osc.push((note, on));
        }
        fn osc_pitch_bend(&mut self, lsb: u8, msb: u8) {
            self.bends.push((lsb, msb));
        }
        fn key(&mut self, half: usize, note: u8, velocity: u8) {
            self.keys[half][note as usize] = velocity > 0;
            self.vel[half][note as usize] = velocity;
        }
        fn wheels(&mut self, pitch: Option<u16>, modwheel: Option<u8>) {
            if let Some(p) = pitch {
                self.pitch = p;
            }
            if let Some(m) = modwheel {
                self.modw = m;
            }
        }
        fn control(&mut self, _generation: u64, event: ControlEvent) {
            self.controls.push(event);
        }
        fn transport(&mut self, transport: Transport) {
            self.transport = transport;
        }
        fn recorded(&mut self, recorded: Recorded) {
            self.recorded.push(recorded);
        }
    }

    fn arp_settings() -> Arc<PatternSettings> {
        let mut s = PatternSettings {
            mode: crate::project::PatternMode::Arp,
            ..Default::default()
        };
        s.clock.internal_fallback = false;
        Arc::new(s)
    }

    #[test]
    fn arp_notes_are_transposed_shown_and_mirrored_to_osc() {
        let (mut core, mut rec) = (Core::default(), Rec::default());
        core.set_osc(&mut rec, true, false);
        core.set_pattern(&mut rec, arp_settings(), 2);
        let t0 = Instant::now();
        core.handle_at(&mut rec, &[0x90, 60, 80], 2, t0);
        assert!(rec.midi.is_empty(), "the key itself is not played");
        assert!(rec.keys[0][60], "but shown as pressed");
        core.handle_at(&mut rec, &[0xF8], 2, t0);
        assert_eq!(
            rec.midi,
            [vec![0xF8], vec![0x90, 62, 80]],
            "clock forwarded, arp note transposed"
        );
        assert!(rec.keys[1][62]);
        assert_eq!(rec.osc, [(62, true)]);
        // Transpose changes before the note ends: it still ends on 62.
        for _ in 0..3 {
            core.handle_at(&mut rec, &[0xF8], 7, t0);
        }
        assert!(rec.midi.contains(&vec![0x80, 62, 0]));
        assert!(!rec.keys[1][62]);
    }

    #[test]
    fn a_key_held_before_the_arp_started_still_ends() {
        let (mut core, mut rec) = (Core::default(), Rec::default());
        core.handle(&mut rec, &[0x90, 60, 80], 0);
        core.set_pattern(&mut rec, arp_settings(), 0);
        core.handle(&mut rec, &[0x80, 60, 0], 0);
        assert_eq!(rec.midi, [vec![0x90, 60, 80], vec![0x80, 60, 0]]);
    }

    #[test]
    fn panic_stops_arp_notes() {
        let (mut core, mut rec) = (Core::default(), Rec::default());
        core.set_pattern(&mut rec, arp_settings(), 0);
        core.handle(&mut rec, &[0x90, 60, 80], 0);
        core.handle(&mut rec, &[0xF8], 0);
        rec.midi.clear();
        core.panic(&mut rec);
        assert!(rec.midi.contains(&vec![0x80, 60, 0]));
        // Released keys no longer arpeggiate.
        core.handle(&mut rec, &[0xF8], 0);
        assert!(!rec.midi.iter().any(|m| m[0] == 0x90));
    }

    fn note_mapping(number: u8, forward: bool) -> Arc<[Mapping]> {
        use crate::project::{MidiTrigger, TriggerKind, TriggerMode, ValueType};
        vec![Mapping {
            index: 0,
            trigger: MidiTrigger {
                kind: TriggerKind::Note,
                channel: None,
                number,
                mode: TriggerMode::Toggle,
                forward,
            },
            ty: ValueType::Bool,
            min: 0.0,
            max: 1.0,
        }]
        .into()
    }

    #[test]
    fn a_mapped_key_controls_instead_of_playing() {
        let (mut core, mut rec) = (Core::default(), Rec::default());
        core.set_osc(&mut rec, true, true);
        core.set_mappings(note_mapping(36, false), 1);
        core.handle(&mut rec, &[0x90, 36, 100], 5);
        core.handle(&mut rec, &[0x80, 36, 0], 5);
        assert!(rec.midi.is_empty(), "nothing reaches the output");
        assert!(rec.osc.is_empty(), "no avatar key either");
        assert!(!rec.keys[0][36], "not shown on the keyboard");
        assert_eq!(rec.controls, [ControlEvent::Toggle { index: 0 }]);
        // Other keys still play.
        core.handle(&mut rec, &[0x90, 60, 100], 5);
        assert_eq!(rec.midi, [vec![0x90, 65, 100]]);
    }

    #[test]
    fn forwarding_mappings_play_and_control() {
        let (mut core, mut rec) = (Core::default(), Rec::default());
        core.set_mappings(note_mapping(36, true), 1);
        core.handle(&mut rec, &[0x90, 36, 100], 0);
        assert_eq!(rec.midi, [vec![0x90, 36, 100]]);
        assert_eq!(rec.controls, [ControlEvent::Toggle { index: 0 }]);
    }

    #[test]
    fn a_held_note_is_released_even_if_its_key_was_mapped_meanwhile() {
        let (mut core, mut rec) = (Core::default(), Rec::default());
        core.handle(&mut rec, &[0x90, 36, 100], 0);
        core.set_mappings(note_mapping(36, false), 1);
        core.handle(&mut rec, &[0x80, 36, 0], 0);
        assert_eq!(
            rec.midi,
            [vec![0x90, 36, 100], vec![0x80, 36, 0]],
            "no stuck note"
        );
    }

    #[test]
    fn velocity_reaches_both_halves_of_the_display() {
        let (mut core, mut rec) = (Core::default(), Rec::default());
        core.handle(&mut rec, &[0x90, 60, 30], 4);
        assert_eq!((rec.vel[0][60], rec.vel[1][64]), (30, 30));
        // Struck again harder without a release: the display follows.
        core.handle(&mut rec, &[0x90, 60, 120], 4);
        assert_eq!(rec.vel[0][60], 120);
        core.handle(&mut rec, &[0x80, 60, 0], 4);
        core.handle(&mut rec, &[0x80, 60, 0], 4);
        assert_eq!((rec.vel[0][60], rec.vel[1][64]), (0, 0));
    }

    #[test]
    fn wheels_follow_pitch_bend_and_cc1_and_reset_on_panic() {
        let (mut core, mut rec) = (Core::default(), Rec::default());
        core.handle(&mut rec, &[0xE3, 0x7F, 0x7F], 0);
        core.handle(&mut rec, &[0xB5, 1, 90], 0);
        core.handle(&mut rec, &[0xB0, 7, 100], 0); // volume, not the mod wheel
        assert_eq!((rec.pitch, rec.modw), (16383, 90));
        assert_eq!(rec.midi.len(), 3, "everything is still forwarded");
        core.panic(&mut rec);
        assert_eq!((rec.pitch, rec.modw), (8192, 0));
    }

    #[test]
    fn transposes_notes_and_passes_everything_else() {
        let (mut core, mut rec) = (Core::default(), Rec::default());
        core.handle(&mut rec, &[0x90, 60, 100], 3);
        core.handle(&mut rec, &[0xB0, 64, 127], 3); // sustain pedal
        core.handle(&mut rec, &[0xF8], 3); // clock
        core.handle(&mut rec, &[0xF0, 1, 2, 3, 0xF7], 3); // sysex
        core.handle(&mut rec, &[0x80, 60, 0], 3);
        assert_eq!(
            rec.midi,
            [
                vec![0x90, 63, 100],
                vec![0xB0, 64, 127],
                vec![0xF8],
                vec![0xF0, 1, 2, 3, 0xF7],
                vec![0x80, 63, 0]
            ]
        );
    }

    #[test]
    fn no_stuck_note_when_transpose_changes_while_held() {
        let (mut core, mut rec) = (Core::default(), Rec::default());
        core.handle(&mut rec, &[0x91, 60, 90], 0);
        assert!(rec.keys[1][60]);
        core.handle(&mut rec, &[0x81, 60, 0], 7);
        assert_eq!(rec.midi, [vec![0x91, 60, 90], vec![0x81, 60, 0]]);
        assert!(!rec.keys[0][60] && !rec.keys[1][60]);
    }

    #[test]
    fn note_on_with_velocity_zero_is_a_note_off() {
        let (mut core, mut rec) = (Core::default(), Rec::default());
        core.handle(&mut rec, &[0x90, 60, 90], 2);
        core.handle(&mut rec, &[0x90, 60, 0], 9);
        assert_eq!(rec.midi[1], [0x90, 62, 0]);
        assert!(!rec.keys[0][60]);
    }

    #[test]
    fn aftertouch_follows_the_held_note() {
        let (mut core, mut rec) = (Core::default(), Rec::default());
        core.handle(&mut rec, &[0x90, 60, 90], 2);
        core.handle(&mut rec, &[0xA0, 60, 40], 5);
        assert_eq!(rec.midi[1], [0xA0, 62, 40]);
    }

    #[test]
    fn osc_mirrors_original_or_transposed_notes() {
        let (mut core, mut rec) = (Core::default(), Rec::default());
        core.handle(&mut rec, &[0x90, 60, 90], 3);
        assert!(rec.osc.is_empty(), "disabled by default");
        core.handle(&mut rec, &[0x80, 60, 0], 3);

        core.set_osc(&mut rec, true, true);
        core.handle(&mut rec, &[0x90, 60, 90], 3);
        core.handle(&mut rec, &[0x80, 60, 0], 3);
        assert_eq!(rec.osc, [(60, true), (60, false)]);

        rec.osc.clear();
        core.set_osc(&mut rec, true, false);
        core.handle(&mut rec, &[0x90, 60, 90], 3);
        assert_eq!(rec.osc, [(63, true)]);
        // Switching the source releases what was sent.
        core.set_osc(&mut rec, true, true);
        assert_eq!(rec.osc, [(63, true), (63, false)]);
    }

    #[test]
    fn pitch_bend_is_forwarded_and_mirrored() {
        let (mut core, mut rec) = (Core::default(), Rec::default());
        core.set_osc(&mut rec, true, true);
        core.handle(&mut rec, &[0xE0, 0x00, 0x60], 5);
        assert_eq!(rec.midi, [vec![0xE0, 0x00, 0x60]]);
        assert_eq!(rec.bends, [(0x00, 0x60)]);
    }

    #[test]
    fn panic_releases_everything() {
        let (mut core, mut rec) = (Core::default(), Rec::default());
        core.set_osc(&mut rec, true, false);
        core.handle(&mut rec, &[0x90, 60, 90], 1);
        core.handle(&mut rec, &[0x92, 40, 90], 1);
        rec.midi.clear();
        rec.osc.clear();
        core.panic(&mut rec);
        let mut offs = rec.midi.clone();
        offs.sort();
        assert_eq!(offs, [vec![0x80, 61, 0], vec![0x82, 41, 0]]);
        let mut osc = rec.osc.clone();
        osc.sort();
        assert_eq!(osc, [(41, false), (61, false)]);
        assert!(rec.keys.iter().all(|half| half.iter().all(|k| !k)));
    }

    #[test]
    fn two_keys_clamped_onto_one_output_note_keep_it_lit() {
        let (mut core, mut rec) = (Core::default(), Rec::default());
        core.handle(&mut rec, &[0x90, 126, 90], 24);
        core.handle(&mut rec, &[0x90, 127, 90], 24);
        core.handle(&mut rec, &[0x80, 126, 0], 24);
        assert!(rec.keys[1][127], "127 is still held by the second key");
        core.handle(&mut rec, &[0x80, 127, 0], 24);
        assert!(!rec.keys[1][127]);
    }

    #[test]
    fn midi_triggers_start_recording_and_the_keys_are_recorded_and_heard() {
        use crate::project::{TriggerKind, TriggerMode};
        let (mut core, mut rec) = (Core::default(), Rec::default());
        let mut settings = PatternSettings {
            mode: crate::project::PatternMode::Seq,
            ..Default::default()
        };
        settings.seq.root = 60;
        core.set_pattern(&mut rec, Arc::new(settings), 12);
        let button = |number| MidiTrigger {
            kind: TriggerKind::Cc,
            channel: None,
            number,
            mode: TriggerMode::Toggle,
            forward: false,
        };
        core.set_pattern_mappings(
            vec![
                (PatternAction::Record, button(20)),
                (PatternAction::Rest, button(21)),
                (PatternAction::NextMode, button(22)),
            ]
            .into(),
        );
        core.handle(&mut rec, &[0xB0, 20, 127], 12);
        assert_eq!(rec.transport.rec, crate::midi::pattern::RecState::Step);
        core.handle(&mut rec, &[0x90, 62, 100], 12);
        assert_eq!(
            rec.midi,
            [vec![0x90, 74, 100]],
            "heard, transposed like everything else"
        );
        core.handle(&mut rec, &[0x80, 62, 0], 12);
        assert_eq!(rec.midi[1], [0x80, 74, 0]);
        core.handle(&mut rec, &[0xB0, 21, 127], 12);
        assert_eq!(rec.transport.step, 2);
        core.handle(&mut rec, &[0xB0, 20, 0], 12);
        core.handle(&mut rec, &[0xB0, 20, 127], 12);
        let last = rec.recorded.last().unwrap();
        assert!(last.done);
        assert_eq!(
            (last.length, last.steps[0].offset, last.steps[1].on),
            (2, 2, false)
        );
        // Mode changes are for the supervisor.
        core.handle(&mut rec, &[0xB0, 22, 127], 12);
        assert_eq!(rec.controls, [ControlEvent::Pattern(PatternAction::NextMode)]);
    }
}
