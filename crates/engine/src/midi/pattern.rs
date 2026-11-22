//! Arpeggiator and step sequencer, driven by MIDI clock ticks (24 per quarter note).
//!
//! Pure logic: it receives key presses, clock messages and the time, and
//! writes the notes to play into a buffer. The forwarder applies transpose,
//! sends them and shows them. Nothing here allocates after warm-up.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::project::{ArpStyle, ClockSource, PatternMode, PatternSettings};

/// Ticks per quarter note in MIDI clock.
pub const PPQN: u64 = 24;
/// Ticks per 4/4 bar.
pub const BAR: u64 = PPQN * 4;
/// External clock counts as present while ticks arrived within this time.
const CLOCK_TIMEOUT: Duration = Duration::from_millis(500);

/// MIDI system real-time / common messages that move the clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockMsg {
    Tick,
    Start,
    Continue,
    Stop,
    /// Song Position Pointer, in sixteenth notes.
    SongPosition(u16),
}

impl ClockMsg {
    pub fn parse(bytes: &[u8]) -> Option<ClockMsg> {
        match *bytes.first()? {
            0xF8 => Some(ClockMsg::Tick),
            0xFA => Some(ClockMsg::Start),
            0xFB => Some(ClockMsg::Continue),
            0xFC => Some(ClockMsg::Stop),
            0xF2 if bytes.len() >= 3 => Some(ClockMsg::SongPosition(
                ((bytes[2] as u16 & 0x7F) << 7) | (bytes[1] as u16 & 0x7F),
            )),
            _ => None,
        }
    }
}

/// Where a clock message came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockOrigin {
    /// The MIDI input that is being played.
    Input,
    /// The separate clock device.
    Device,
    /// The app's own tempo.
    Internal,
}

/// A note the pattern wants played. Note numbers are before transposing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gen {
    On { channel: u8, note: u8, velocity: u8 },
    Off { channel: u8, note: u8 },
}

/// Which clock currently drives the pattern.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClockState {
    /// No clock: nothing advances.
    #[default]
    None,
    External,
    Internal,
}

/// What the front end shows about the pattern.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Transport {
    pub clock: ClockState,
    /// Tempo × 10 (1205 = 120.5 BPM); 0 while unknown.
    pub bpm_x10: u16,
    /// Beat in the bar, 0-3.
    pub beat: u8,
    /// Sequencer is running.
    pub playing: bool,
    /// Sequencer waits for the next bar to start.
    pub armed: bool,
    /// Current sequencer step, -1 when not playing.
    pub step: i16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Seq {
    Stopped,
    /// Starts on the next bar line.
    Armed,
    /// Running since this clock position.
    Playing(u64),
}

#[derive(Clone, Copy, Debug)]
struct Sounding {
    channel: u8,
    note: u8,
    off_at: u64,
}

pub struct PatternEngine {
    settings: Arc<PatternSettings>,

    // Clock
    /// Position in ticks of the last tick.
    pos: u64,
    /// The next tick sets `pos` to this (after Start or Song Position).
    next_pos: Option<u64>,
    seen_tick: bool,
    last_external: Option<Instant>,
    last_internal: Option<Instant>,
    last_external_tick: Option<Instant>,
    /// Smoothed external tick interval in seconds.
    interval: f64,

    // Keys
    /// Held (or latched) keys in the order they were pressed: (note, velocity).
    held: Vec<(u8, u8)>,
    /// Keys physically down.
    down: u8,
    last_channel: u8,

    // Arpeggiator
    order: Vec<(u8, u8)>,
    index: usize,
    rng: u64,

    // Sequencer
    seq: Seq,
    step: i16,

    sounding: Vec<Sounding>,
    transport: Transport,
}

/// Fires once per step; every second step is delayed by the swing.
fn step_at(pos: u64, base: u64, rate: u64, swing: u8) -> Option<u64> {
    let rel = pos.checked_sub(base)?;
    let step = rel / rate;
    let delay = if step % 2 == 1 {
        rate * swing as u64 / 200
    } else {
        0
    };
    (rel % rate == delay).then_some(step)
}

fn gate_ticks(rate: u64, gate: u8) -> u64 {
    (rate * gate as u64 / 100).max(1)
}

impl PatternEngine {
    pub fn new(settings: Arc<PatternSettings>) -> Self {
        PatternEngine {
            settings,
            pos: 0,
            next_pos: None,
            seen_tick: false,
            last_external: None,
            last_internal: None,
            last_external_tick: None,
            interval: 0.0,
            held: Vec::with_capacity(128),
            down: 0,
            last_channel: 0,
            order: Vec::with_capacity(512),
            index: 0,
            rng: 0x9E37_79B9_7F4A_7C15,
            seq: Seq::Stopped,
            step: -1,
            sounding: Vec::with_capacity(128),
            transport: Transport {
                step: -1,
                ..Transport::default()
            },
        }
    }

    pub fn active(&self) -> bool {
        self.settings.mode != PatternMode::Off
    }

    pub fn transport(&self) -> Transport {
        self.transport
    }

    /// New settings. A change of mode ends whatever was playing.
    pub fn set_settings(&mut self, settings: Arc<PatternSettings>, out: &mut Vec<Gen>) {
        let mode_changed = settings.mode != self.settings.mode;
        self.settings = settings;
        if mode_changed {
            self.release_all(out);
            self.seq = Seq::Stopped;
            self.step = -1;
            self.index = 0;
            if !self.active() {
                self.held.clear();
                self.down = 0;
            }
        }
        self.rebuild_order();
        self.refresh(Instant::now());
    }

    /// A key on the MIDI input (only while a pattern mode is on).
    pub fn note(&mut self, channel: u8, note: u8, velocity: u8, pressed: bool) {
        let latch = self.settings.mode == PatternMode::Arp && self.settings.arp.latch;
        if pressed {
            self.last_channel = channel;
            if latch && self.down == 0 {
                // First key after letting go: a new chord replaces the latched one.
                self.held.clear();
            }
            self.down = self.down.saturating_add(1);
            self.held.retain(|(n, _)| *n != note);
            if self.held.len() < self.held.capacity() {
                self.held.push((note, velocity.max(1)));
            }
            if self.held.len() == 1 && self.order.is_empty() {
                // A fresh start of the arpeggio begins at its first note.
                self.index = 0;
            }
        } else {
            self.down = self.down.saturating_sub(1);
            if !latch {
                self.held.retain(|(n, _)| *n != note);
            }
        }
        self.rebuild_order();
    }

    /// Start (true) or stop (false) the sequencer. Start waits for the next bar line.
    pub fn play(&mut self, on: bool, out: &mut Vec<Gen>) {
        if self.settings.mode != PatternMode::Seq {
            return;
        }
        if on {
            if self.seq == Seq::Stopped {
                self.seq = Seq::Armed;
            }
        } else {
            self.seq = Seq::Stopped;
            self.step = -1;
            self.release_all(out);
        }
        self.refresh(Instant::now());
    }

    /// Release everything and forget held keys.
    pub fn panic(&mut self, out: &mut Vec<Gen>) {
        self.release_all(out);
        self.held.clear();
        self.order.clear();
        self.down = 0;
        self.seq = Seq::Stopped;
        self.step = -1;
        self.refresh(Instant::now());
    }

    fn release_all(&mut self, out: &mut Vec<Gen>) {
        for s in self.sounding.drain(..) {
            out.push(Gen::Off {
                channel: s.channel,
                note: s.note,
            });
        }
    }

    fn external_origin(&self) -> ClockOrigin {
        match self.settings.clock.source {
            ClockSource::Input => ClockOrigin::Input,
            ClockSource::Device => ClockOrigin::Device,
        }
    }

    fn external_alive(&self, now: Instant) -> bool {
        self.last_external
            .is_some_and(|t| now.saturating_duration_since(t) < CLOCK_TIMEOUT)
    }

    /// A clock message. Messages from a source that is not selected are ignored,
    /// except that the internal clock fills in while no external clock arrives.
    pub fn clock(&mut self, origin: ClockOrigin, msg: ClockMsg, now: Instant, out: &mut Vec<Gen>) {
        let accept = if origin == self.external_origin() {
            if msg == ClockMsg::Tick {
                self.last_external = Some(now);
            }
            true
        } else if origin == ClockOrigin::Internal {
            self.last_internal = Some(now);
            msg == ClockMsg::Tick && self.settings.clock.internal_fallback && !self.external_alive(now)
        } else {
            false
        };
        if !accept {
            self.refresh(now);
            return;
        }
        match msg {
            ClockMsg::Tick => {
                if origin != ClockOrigin::Internal {
                    if let Some(last) = self.last_external_tick {
                        let dt = now.saturating_duration_since(last).as_secs_f64();
                        if dt > 0.0 && dt < 0.5 {
                            self.interval = if self.interval == 0.0 {
                                dt
                            } else {
                                self.interval * 0.9 + dt * 0.1
                            };
                        }
                    }
                    self.last_external_tick = Some(now);
                }
                self.pos = match self.next_pos.take() {
                    Some(p) => p,
                    None if !self.seen_tick => 0,
                    None => self.pos + 1,
                };
                self.seen_tick = true;
                self.tick(out);
            }
            ClockMsg::Start => {
                self.next_pos = Some(0);
                self.index = 0;
                if self.settings.mode == PatternMode::Seq && self.settings.seq.follow_transport {
                    self.release_all(out);
                    self.seq = Seq::Playing(0);
                }
            }
            ClockMsg::Continue => {}
            ClockMsg::Stop => {
                if self.settings.mode == PatternMode::Seq && self.settings.seq.follow_transport {
                    self.seq = Seq::Stopped;
                    self.step = -1;
                }
                self.release_all(out);
            }
            ClockMsg::SongPosition(sixteenths) => {
                self.next_pos = Some(sixteenths as u64 * 6);
            }
        }
        self.refresh(now);
    }

    fn refresh(&mut self, now: Instant) {
        let external = self.external_alive(now);
        let internal = !external
            && self.settings.clock.internal_fallback
            && self
                .last_internal
                .is_some_and(|t| now.saturating_duration_since(t) < CLOCK_TIMEOUT);
        let (clock, bpm) = if external {
            let bpm = if self.interval > 0.0 {
                60.0 / (self.interval * PPQN as f64)
            } else {
                0.0
            };
            (ClockState::External, bpm)
        } else if internal {
            (ClockState::Internal, self.settings.clock.bpm as f64)
        } else {
            (ClockState::None, 0.0)
        };
        self.transport = Transport {
            clock,
            bpm_x10: (bpm * 10.0).round().clamp(0.0, u16::MAX as f64) as u16,
            beat: ((self.pos / PPQN) % 4) as u8,
            playing: matches!(self.seq, Seq::Playing(_)),
            armed: self.seq == Seq::Armed,
            step: self.step,
        };
    }

    fn tick(&mut self, out: &mut Vec<Gen>) {
        let pos = self.pos;
        // Notes whose gate ended.
        let mut i = 0;
        while i < self.sounding.len() {
            if self.sounding[i].off_at <= pos {
                let s = self.sounding.swap_remove(i);
                out.push(Gen::Off {
                    channel: s.channel,
                    note: s.note,
                });
            } else {
                i += 1;
            }
        }
        match self.settings.mode {
            PatternMode::Off => {}
            PatternMode::Arp => self.arp_tick(pos, out),
            PatternMode::Seq => self.seq_tick(pos, out),
        }
    }

    fn start(&mut self, channel: u8, note: u8, velocity: u8, off_at: u64, out: &mut Vec<Gen>) {
        // The same note still sounding (gate 100 %, ties into a repeat): end it first.
        if let Some(i) = self
            .sounding
            .iter()
            .position(|s| s.channel == channel && s.note == note)
        {
            self.sounding.swap_remove(i);
            out.push(Gen::Off { channel, note });
        }
        out.push(Gen::On {
            channel,
            note,
            velocity,
        });
        if self.sounding.len() < self.sounding.capacity() {
            self.sounding.push(Sounding {
                channel,
                note,
                off_at,
            });
        }
    }

    // -- Arpeggiator ------------------------------------------------------------

    fn rebuild_order(&mut self) {
        self.order.clear();
        if self.settings.mode != PatternMode::Arp || self.held.is_empty() {
            return;
        }
        let arp = &self.settings.arp;
        let mut base: Vec<(u8, u8)> = Vec::new();
        // Small (at most 128 entries) and only on key changes, not per tick.
        base.extend_from_slice(&self.held);
        match arp.style {
            ArpStyle::Played | ArpStyle::Reverse | ArpStyle::Chord => {}
            _ => base.sort_unstable_by_key(|(n, _)| *n),
        }
        let mut asc: Vec<(u8, u8)> = Vec::with_capacity(base.len() * arp.octaves as usize);
        for octave in 0..arp.octaves {
            for &(n, v) in &base {
                let note = n as u16 + 12 * octave as u16;
                if note <= 127 {
                    asc.push((note as u8, v));
                }
            }
        }
        let desc: Vec<(u8, u8)> = asc.iter().rev().copied().collect();
        let inner = |v: &[(u8, u8)]| -> Vec<(u8, u8)> {
            if v.len() > 2 {
                v[1..v.len() - 1].to_vec()
            } else {
                Vec::new()
            }
        };
        match arp.style {
            ArpStyle::Up | ArpStyle::Played | ArpStyle::Random | ArpStyle::Chord => self.order.extend(asc),
            ArpStyle::Down | ArpStyle::Reverse => self.order.extend(desc),
            ArpStyle::UpDown => {
                self.order.extend(asc.iter().copied());
                self.order.extend(inner(&desc));
            }
            ArpStyle::DownUp => {
                self.order.extend(desc.iter().copied());
                self.order.extend(inner(&asc));
            }
            ArpStyle::UpDownInclusive => {
                self.order.extend(asc.iter().copied());
                self.order.extend(desc);
            }
        }
    }

    fn next_random(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn arp_tick(&mut self, pos: u64, out: &mut Vec<Gen>) {
        if self.order.is_empty() {
            return;
        }
        let arp = self.settings.arp.clone();
        let rate = arp.rate.ticks() as u64;
        if step_at(pos, 0, rate, arp.swing).is_none() {
            return;
        }
        let off_at = pos + gate_ticks(rate, arp.gate);
        let channel = self.last_channel;
        if arp.style == ArpStyle::Chord {
            for i in 0..self.order.len() {
                let (note, v) = self.order[i];
                self.start(channel, note, arp.velocity.unwrap_or(v), off_at, out);
            }
            return;
        }
        let len = self.order.len();
        let i = if arp.style == ArpStyle::Random {
            (self.next_random() % len as u64) as usize
        } else {
            let i = self.index % len;
            self.index = (i + 1) % len;
            i
        };
        let (note, v) = self.order[i];
        self.start(channel, note, arp.velocity.unwrap_or(v), off_at, out);
    }

    // -- Sequencer ---------------------------------------------------------------

    fn seq_tick(&mut self, pos: u64, out: &mut Vec<Gen>) {
        if self.seq == Seq::Armed && pos.is_multiple_of(BAR) {
            self.seq = Seq::Playing(pos);
        }
        let Seq::Playing(start) = self.seq else {
            return;
        };
        let seq = &self.settings.seq;
        let rate = seq.rate.ticks() as u64;
        let Some(n) = step_at(pos, start, rate, seq.swing) else {
            return;
        };
        let index = (n % seq.length as u64) as usize;
        self.step = index as i16;
        let step = seq.steps[index];
        if !step.on {
            return;
        }
        if step.tie {
            // The note before holds through this step (its length was set when it started).
            return;
        }
        // A note lasts through the tie steps that follow it; the gate of the last one counts.
        let length = seq.length as usize;
        let mut ties = 0u64;
        let mut last_gate = step.gate;
        while (ties as usize) < length - 1 {
            let next = seq.steps[(index + 1 + ties as usize) % length];
            if !(next.on && next.tie) {
                break;
            }
            ties += 1;
            last_gate = next.gate;
        }
        let off_at = pos + rate * ties + gate_ticks(rate, last_gate);
        let (base, channel) = match self.held.last() {
            Some(&(n, _)) => (n as i16, self.last_channel),
            None => (seq.root as i16, seq.channel - 1),
        };
        let note = (base + step.offset as i16).clamp(0, 127) as u8;
        let velocity = step.velocity;
        // A new step ends the previous note (unless tied, handled above).
        let previous: Vec<Sounding> = self.sounding.drain(..).collect();
        for s in previous {
            out.push(Gen::Off {
                channel: s.channel,
                note: s.note,
            });
        }
        self.start(channel, note, velocity, off_at, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{Rate, Step};

    fn engine(f: impl FnOnce(&mut PatternSettings)) -> PatternEngine {
        let mut s = PatternSettings::default();
        s.clock.internal_fallback = false;
        f(&mut s);
        s.normalize();
        PatternEngine::new(Arc::new(s))
    }

    /// Feed `ticks` input clock ticks 20 ms apart; collect note-ons as (tick, note).
    fn run(e: &mut PatternEngine, t0: Instant, from: u64, ticks: u64) -> Vec<(u64, Gen)> {
        let mut all = Vec::new();
        for k in from..from + ticks {
            let mut out = Vec::new();
            e.clock(
                ClockOrigin::Input,
                ClockMsg::Tick,
                t0 + Duration::from_millis(20 * k),
                &mut out,
            );
            all.extend(out.into_iter().map(|g| (k, g)));
        }
        all
    }

    fn ons(events: &[(u64, Gen)]) -> Vec<(u64, u8)> {
        events
            .iter()
            .filter_map(|(k, g)| match g {
                Gen::On { note, .. } => Some((*k, *note)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn parses_clock_messages() {
        assert_eq!(ClockMsg::parse(&[0xF8]), Some(ClockMsg::Tick));
        assert_eq!(ClockMsg::parse(&[0xFA]), Some(ClockMsg::Start));
        assert_eq!(
            ClockMsg::parse(&[0xF2, 0x10, 0x01]),
            Some(ClockMsg::SongPosition(144))
        );
        assert_eq!(ClockMsg::parse(&[0x90, 60, 1]), None);
    }

    #[test]
    fn arp_up_on_sixteenths() {
        let mut e = engine(|s| s.mode = PatternMode::Arp);
        e.note(0, 64, 100, true);
        e.note(0, 60, 100, true);
        e.note(0, 67, 100, true);
        let got = ons(&run(&mut e, Instant::now(), 0, 24));
        assert_eq!(got, [(0, 60), (6, 64), (12, 67), (18, 60)]);
    }

    #[test]
    fn arp_styles() {
        let order = |style: ArpStyle, octaves: u8| {
            let mut e = engine(|s| {
                s.mode = PatternMode::Arp;
                s.arp.style = style;
                s.arp.octaves = octaves;
                s.arp.rate = Rate::ThirtySecondTriplet;
            });
            e.note(0, 64, 100, true);
            e.note(0, 60, 100, true);
            e.note(0, 67, 100, true);
            ons(&run(&mut e, Instant::now(), 0, 16))
                .into_iter()
                .map(|(_, n)| n)
                .collect::<Vec<_>>()
        };
        assert_eq!(order(ArpStyle::Down, 1), [67, 64, 60, 67, 64, 60, 67, 64]);
        assert_eq!(order(ArpStyle::UpDown, 1), [60, 64, 67, 64, 60, 64, 67, 64]);
        assert_eq!(order(ArpStyle::DownUp, 1), [67, 64, 60, 64, 67, 64, 60, 64]);
        assert_eq!(
            order(ArpStyle::UpDownInclusive, 1),
            [60, 64, 67, 67, 64, 60, 60, 64]
        );
        assert_eq!(order(ArpStyle::Played, 1), [64, 60, 67, 64, 60, 67, 64, 60]);
        assert_eq!(order(ArpStyle::Reverse, 1), [67, 60, 64, 67, 60, 64, 67, 60]);
        assert_eq!(order(ArpStyle::Up, 2), [60, 64, 67, 72, 76, 79, 60, 64]);
        let random = order(ArpStyle::Random, 1);
        assert!(random.iter().all(|n| [60, 64, 67].contains(n)));
        // Chord: every step plays all three.
        assert_eq!(order(ArpStyle::Chord, 1).len(), 24);
    }

    #[test]
    fn gate_and_release() {
        let mut e = engine(|s| {
            s.mode = PatternMode::Arp;
            s.arp.gate = 50;
        });
        e.note(0, 60, 90, true);
        let ev = run(&mut e, Instant::now(), 0, 7);
        assert_eq!(
            ev,
            [
                (
                    0,
                    Gen::On {
                        channel: 0,
                        note: 60,
                        velocity: 90
                    }
                ),
                (3, Gen::Off { channel: 0, note: 60 }),
                (
                    6,
                    Gen::On {
                        channel: 0,
                        note: 60,
                        velocity: 90
                    }
                ),
            ]
        );
        // Releasing the key stops new notes (no latch).
        e.note(0, 60, 0, false);
        let ev = run(&mut e, Instant::now(), 7, 12);
        assert_eq!(ons(&ev), []);
        assert!(
            ev.iter().any(|(_, g)| matches!(g, Gen::Off { note: 60, .. })),
            "last note still ends"
        );
    }

    #[test]
    fn latch_keeps_playing_and_a_new_chord_replaces_it() {
        let mut e = engine(|s| {
            s.mode = PatternMode::Arp;
            s.arp.latch = true;
        });
        e.note(0, 60, 100, true);
        e.note(0, 60, 0, false);
        assert_eq!(ons(&run(&mut e, Instant::now(), 0, 12)).len(), 2);
        e.note(0, 72, 100, true);
        let got: Vec<u8> = ons(&run(&mut e, Instant::now(), 12, 12))
            .into_iter()
            .map(|(_, n)| n)
            .collect();
        assert_eq!(got, [72, 72]);
    }

    #[test]
    fn swing_delays_every_second_step() {
        let mut e = engine(|s| {
            s.mode = PatternMode::Arp;
            s.arp.swing = 50;
            s.arp.rate = Rate::Eighth; // 12 ticks; swing 50 % → 3 ticks
        });
        e.note(0, 60, 100, true);
        let got: Vec<u64> = ons(&run(&mut e, Instant::now(), 0, 48))
            .into_iter()
            .map(|(k, _)| k)
            .collect();
        assert_eq!(got, [0, 15, 24, 39]);
    }

    #[test]
    fn sequencer_waits_for_the_next_bar() {
        let mut e = engine(|s| {
            s.mode = PatternMode::Seq;
            s.seq.follow_transport = false;
            s.seq.length = 4;
            s.seq.root = 48;
            for (i, o) in [0i8, 3, 7, 12].iter().enumerate() {
                s.seq.steps[i] = Step {
                    offset: *o,
                    ..Step::default()
                };
            }
        });
        let t0 = Instant::now();
        run(&mut e, t0, 0, 30); // clock already running, now at tick 29
        let mut out = Vec::new();
        e.play(true, &mut out);
        assert!(e.transport().armed);
        let got = ons(&run(&mut e, t0, 30, 96 + 24));
        // Nothing until the bar line at tick 96, then sixteenths from there.
        assert_eq!(got.first(), Some(&(96, 48)));
        assert_eq!(&got[..4], &[(96, 48), (102, 51), (108, 55), (114, 60)]);
        assert_eq!(got[4], (120, 48), "loops after 4 steps");
        assert!(e.transport().playing);
    }

    #[test]
    fn sequencer_follows_the_held_key_and_rests_and_ties() {
        let mut e = engine(|s| {
            s.mode = PatternMode::Seq;
            s.seq.length = 4;
            s.seq.steps[0] = Step {
                offset: 0,
                ..Step::default()
            };
            s.seq.steps[1] = Step {
                on: false,
                ..Step::default()
            };
            s.seq.steps[2] = Step {
                offset: 5,
                gate: 50,
                ..Step::default()
            };
            s.seq.steps[3] = Step {
                tie: true,
                gate: 100,
                ..Step::default()
            };
        });
        let t0 = Instant::now();
        let mut out = Vec::new();
        e.clock(ClockOrigin::Input, ClockMsg::Start, t0, &mut out);
        assert!(e.transport().playing, "MIDI Start starts it right away");
        e.note(2, 62, 100, true);
        let ev = run(&mut e, t0, 0, 25);
        assert_eq!(
            ons(&ev),
            [(0, 62), (12, 67), (24, 62)],
            "D (held key) + 0, rest, D + 5, tie, loop"
        );
        let off67: Vec<u64> = ev
            .iter()
            .filter_map(|(k, g)| matches!(g, Gen::Off { note: 67, .. }).then_some(*k))
            .collect();
        assert_eq!(off67, [24], "the tie holds 67 through step 4");
        assert!(ev
            .iter()
            .all(|(_, g)| !matches!(g, Gen::On { channel, .. } if *channel != 2)));
    }

    #[test]
    fn transport_stop_and_song_position() {
        let mut e = engine(|s| s.mode = PatternMode::Seq);
        let t0 = Instant::now();
        let mut out = Vec::new();
        e.clock(ClockOrigin::Input, ClockMsg::Start, t0, &mut out);
        run(&mut e, t0, 0, 10);
        e.clock(ClockOrigin::Input, ClockMsg::Stop, t0, &mut out);
        assert!(!e.transport().playing);
        e.clock(ClockOrigin::Input, ClockMsg::SongPosition(16), t0, &mut out); // bar 2
        run(&mut e, t0, 10, 1);
        assert_eq!(e.pos, 96);
    }

    #[test]
    fn internal_clock_only_fills_in_without_external_clock() {
        let mut e = engine(|s| {
            s.mode = PatternMode::Arp;
            s.clock.internal_fallback = true;
            s.clock.bpm = 100.0;
        });
        e.note(0, 60, 100, true);
        let t0 = Instant::now();
        let mut out = Vec::new();
        e.clock(ClockOrigin::Internal, ClockMsg::Tick, t0, &mut out);
        assert_eq!(e.transport().clock, ClockState::Internal);
        assert_eq!(e.transport().bpm_x10, 1000);
        assert_eq!(out.len(), 1, "internal tick played the first step");
        // External clock arrives: internal ticks are ignored from now on.
        e.clock(ClockOrigin::Input, ClockMsg::Tick, t0, &mut out);
        let before = e.pos;
        e.clock(
            ClockOrigin::Internal,
            ClockMsg::Tick,
            t0 + Duration::from_millis(10),
            &mut out,
        );
        assert_eq!(e.pos, before);
        assert_eq!(e.transport().clock, ClockState::External);
        // A device clock is ignored while the input is the source.
        e.clock(ClockOrigin::Device, ClockMsg::Tick, t0, &mut out);
        assert_eq!(e.pos, before);
        // Input clock stops: after the timeout the internal clock takes over again.
        let later = t0 + Duration::from_secs(2);
        e.clock(ClockOrigin::Internal, ClockMsg::Tick, later, &mut out);
        assert_eq!(e.pos, before + 1);
        assert_eq!(e.transport().clock, ClockState::Internal);
    }

    #[test]
    fn measures_external_tempo() {
        let mut e = engine(|s| s.mode = PatternMode::Arp);
        // 20 ms per tick = 125 BPM.
        run(&mut e, Instant::now(), 0, 50);
        let bpm = e.transport().bpm_x10;
        assert!((1240..=1260).contains(&bpm), "{bpm}");
    }

    #[test]
    fn switching_mode_releases_sounding_notes() {
        let mut e = engine(|s| s.mode = PatternMode::Arp);
        e.note(0, 60, 100, true);
        run(&mut e, Instant::now(), 0, 1);
        let mut out = Vec::new();
        let mut off = PatternSettings::default();
        off.normalize();
        e.set_settings(Arc::new(off), &mut out);
        assert_eq!(out, [Gen::Off { channel: 0, note: 60 }]);
        assert!(!e.active());
    }
}
