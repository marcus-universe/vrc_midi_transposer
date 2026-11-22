//! Arpeggiator and step sequencer settings, and where their tempo comes from.

use serde::{Deserialize, Serialize};

use super::model::PortRef;

/// Longest sequence.
pub const MAX_STEPS: usize = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternMode {
    /// Notes are forwarded as played.
    #[default]
    Off,
    Arp,
    Seq,
}

/// Where the tempo comes from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClockSource {
    /// MIDI clock arriving on the MIDI input.
    #[default]
    Input,
    /// MIDI clock from another device (`ClockSettings::device`).
    Device,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClockSettings {
    pub source: ClockSource,
    /// The clock device when `source` is `Device`.
    pub device: Option<PortRef>,
    /// Use the internal tempo while no MIDI clock arrives.
    pub internal_fallback: bool,
    /// Internal tempo, beats per minute.
    pub bpm: f32,
}

impl Default for ClockSettings {
    fn default() -> Self {
        ClockSettings {
            source: ClockSource::Input,
            device: None,
            internal_fallback: true,
            bpm: 120.0,
        }
    }
}

/// Note length of one step, as a fraction of a whole note (4/4 bar).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rate {
    #[serde(rename = "1/1")]
    Whole,
    #[serde(rename = "1/2")]
    Half,
    #[serde(rename = "1/2T")]
    HalfTriplet,
    #[serde(rename = "1/4D")]
    QuarterDotted,
    #[serde(rename = "1/4")]
    Quarter,
    #[serde(rename = "1/4T")]
    QuarterTriplet,
    #[serde(rename = "1/8D")]
    EighthDotted,
    #[serde(rename = "1/8")]
    Eighth,
    #[serde(rename = "1/8T")]
    EighthTriplet,
    #[serde(rename = "1/16D")]
    SixteenthDotted,
    #[default]
    #[serde(rename = "1/16")]
    Sixteenth,
    #[serde(rename = "1/16T")]
    SixteenthTriplet,
    #[serde(rename = "1/32")]
    ThirtySecond,
    #[serde(rename = "1/32T")]
    ThirtySecondTriplet,
}

impl Rate {
    /// Length in MIDI clock ticks (24 per quarter note).
    pub fn ticks(self) -> u32 {
        match self {
            Rate::Whole => 96,
            Rate::Half => 48,
            Rate::HalfTriplet => 32,
            Rate::QuarterDotted => 36,
            Rate::Quarter => 24,
            Rate::QuarterTriplet => 16,
            Rate::EighthDotted => 18,
            Rate::Eighth => 12,
            Rate::EighthTriplet => 8,
            Rate::SixteenthDotted => 9,
            Rate::Sixteenth => 6,
            Rate::SixteenthTriplet => 4,
            Rate::ThirtySecond => 3,
            Rate::ThirtySecondTriplet => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArpStyle {
    #[default]
    Up,
    Down,
    /// Up then down, without repeating the top and bottom note.
    UpDown,
    /// Down then up, without repeating the top and bottom note.
    DownUp,
    /// Up then down, playing the top and bottom note twice.
    UpDownInclusive,
    /// In the order the keys were pressed.
    Played,
    /// In the reverse order the keys were pressed.
    Reverse,
    Random,
    /// All held notes together on every step.
    Chord,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ArpSettings {
    pub style: ArpStyle,
    pub rate: Rate,
    /// How many octaves the pattern spans, 1-4.
    pub octaves: u8,
    /// Note length in percent of a step, 5-100.
    pub gate: u8,
    /// Delay of every second step in percent of half a step, 0-75.
    pub swing: u8,
    /// Keep playing after the keys are released.
    pub latch: bool,
    /// Fixed velocity 1-127; `None` uses the velocity of the key.
    pub velocity: Option<u8>,
}

impl Default for ArpSettings {
    fn default() -> Self {
        ArpSettings {
            style: ArpStyle::Up,
            rate: Rate::Sixteenth,
            octaves: 1,
            gate: 50,
            swing: 0,
            latch: false,
            velocity: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Step {
    /// The step plays a note.
    pub on: bool,
    /// Semitones from the root (or from the held key), -48 to +48.
    pub offset: i8,
    /// 1-127.
    pub velocity: u8,
    /// Note length in percent of a step, 5-100.
    pub gate: u8,
    /// Hold the previous note instead of playing a new one.
    pub tie: bool,
}

impl Default for Step {
    fn default() -> Self {
        Step {
            on: true,
            offset: 0,
            velocity: 100,
            gate: 50,
            tie: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SeqSettings {
    pub rate: Rate,
    /// Number of steps that play, 1-64.
    pub length: u8,
    /// Note the steps are relative to while no key is held.
    pub root: u8,
    /// Delay of every second step in percent of half a step, 0-75.
    pub swing: u8,
    /// MIDI channel 1-16 when no key was pressed yet.
    pub channel: u8,
    /// Start and stop with the MIDI Start/Stop of the clock source.
    pub follow_transport: bool,
    /// Always `MAX_STEPS` long; `length` says how many play.
    pub steps: Vec<Step>,
}

impl Default for SeqSettings {
    fn default() -> Self {
        // A simple pattern so Play does something right away.
        let offsets = [0, 7, 12, 7, 3, 7, 10, 7, 0, 7, 12, 7, 5, 7, 10, 12];
        let mut steps = vec![Step::default(); MAX_STEPS];
        for (step, offset) in steps.iter_mut().zip(offsets) {
            step.offset = offset;
        }
        SeqSettings {
            rate: Rate::Sixteenth,
            length: 16,
            root: 48,
            swing: 0,
            channel: 1,
            follow_transport: true,
            steps,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PatternSettings {
    pub mode: PatternMode,
    pub clock: ClockSettings,
    pub arp: ArpSettings,
    pub seq: SeqSettings,
}

impl PatternSettings {
    /// Repair values that would break the engine.
    pub fn normalize(&mut self) {
        let c = &mut self.clock;
        c.bpm = if c.bpm.is_finite() {
            c.bpm.clamp(20.0, 300.0)
        } else {
            120.0
        };
        if c.device
            .as_ref()
            .is_some_and(|p| p.id.is_empty() && p.name.is_empty())
        {
            c.device = None;
        }
        let a = &mut self.arp;
        a.octaves = a.octaves.clamp(1, 4);
        a.gate = a.gate.clamp(5, 100);
        a.swing = a.swing.min(75);
        a.velocity = a.velocity.map(|v| v.clamp(1, 127));
        let s = &mut self.seq;
        s.length = s.length.clamp(1, MAX_STEPS as u8);
        s.root &= 0x7F;
        s.swing = s.swing.min(75);
        s.channel = s.channel.clamp(1, 16);
        s.steps.resize(MAX_STEPS, Step::default());
        for step in &mut s.steps {
            step.offset = step.offset.clamp(-48, 48);
            step.velocity = step.velocity.clamp(1, 127);
            step.gate = step.gate.clamp(5, 100);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_are_in_clock_ticks() {
        assert_eq!(Rate::Quarter.ticks(), 24);
        assert_eq!(Rate::Sixteenth.ticks(), 6);
        assert_eq!(Rate::EighthTriplet.ticks() * 3, Rate::Quarter.ticks());
        assert_eq!(Rate::EighthDotted.ticks(), Rate::Eighth.ticks() * 3 / 2);
    }

    #[test]
    fn serialises_rates_as_fractions_and_repairs_values() {
        let mut p: PatternSettings = serde_json::from_str(
            r#"{"mode":"seq","arp":{"rate":"1/8T","octaves":9,"gate":0},"seq":{"length":200,"steps":[{"offset":100}]},"clock":{"bpm":1000}}"#,
        )
        .unwrap();
        p.normalize();
        assert_eq!(p.mode, PatternMode::Seq);
        assert_eq!(p.arp.rate, Rate::EighthTriplet);
        assert_eq!((p.arp.octaves, p.arp.gate), (4, 5));
        assert_eq!(p.seq.length, 64);
        assert_eq!(p.seq.steps.len(), MAX_STEPS);
        assert_eq!(p.seq.steps[0].offset, 48);
        assert_eq!(p.clock.bpm, 300.0);
        assert!(serde_json::to_string(&p).unwrap().contains(r#""rate":"1/8T""#));
    }

    #[test]
    fn missing_section_gives_defaults() {
        let p: PatternSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(p.mode, PatternMode::Off);
        assert_eq!(p.seq.steps.len(), MAX_STEPS);
        assert!(p.clock.internal_fallback);
    }
}
