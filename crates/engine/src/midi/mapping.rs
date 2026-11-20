//! Custom controls triggered by MIDI messages, and MIDI Learn.
//!
//! Runs in the forwarder before a message is transposed or forwarded, so it
//! matches what the player pressed, not what is sent on.

use std::sync::Arc;

use crate::project::{MidiTrigger, SendAddress, TriggerKind, TriggerMode, ValueType};

/// One custom control with a MIDI trigger, prepared for the forwarder.
#[derive(Clone, Debug, PartialEq)]
pub struct Mapping {
    /// Index into `project.osc.sending_addresses`.
    pub index: usize,
    pub trigger: MidiTrigger,
    pub ty: ValueType,
    pub min: f32,
    pub max: f32,
}

impl Mapping {
    /// The mappings of a project's custom controls.
    pub fn from_controls(controls: &[SendAddress]) -> Arc<[Mapping]> {
        controls
            .iter()
            .enumerate()
            .filter_map(|(index, c)| {
                let (min, max) = c.range();
                c.midi.map(|trigger| Mapping {
                    index,
                    trigger,
                    ty: c.ty,
                    min,
                    max,
                })
            })
            .collect()
    }
}

/// What a MIDI message asks of a custom control.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ControlEvent {
    /// Set the control to a value (already within its range).
    Set { index: usize, value: f32 },
    /// Flip the control between its lowest and highest value.
    Toggle { index: usize },
    /// MIDI Learn: this message should trigger control `index`.
    Learned { index: usize, trigger: MidiTrigger },
}

#[derive(Default)]
pub struct Mappings {
    list: Option<Arc<[Mapping]>>,
    /// Last CC value per mapping, so a knob held still does not repeat itself.
    last_cc: Vec<Option<u8>>,
    /// MIDI Learn waits for a message for this control.
    learn: Option<usize>,
}

impl Mappings {
    pub fn set(&mut self, list: Arc<[Mapping]>) {
        self.last_cc = vec![None; list.len()];
        self.list = (!list.is_empty()).then_some(list);
    }

    pub fn learn(&mut self, index: Option<usize>) {
        self.learn = index;
    }

    pub fn is_idle(&self) -> bool {
        self.list.is_none() && self.learn.is_none()
    }

    /// Look at one incoming message. Calls `emit` for every control it
    /// triggers and returns true if it must not go any further (a matching
    /// mapping that does not forward).
    pub fn process(&mut self, bytes: &[u8], mut emit: impl FnMut(ControlEvent)) -> bool {
        let (Some(&status), Some(&data1)) = (bytes.first(), bytes.get(1)) else {
            return false;
        };
        let kind = status & 0xF0;
        let channel = (status & 0x0F) + 1;
        let number = data1 & 0x7F;
        let value = bytes.get(2).copied().unwrap_or(0) & 0x7F;

        if let Some(index) = self.learn {
            let learned = match kind {
                0x90 if value > 0 => Some(TriggerKind::Note),
                0xB0 => Some(TriggerKind::Cc),
                0xC0 => Some(TriggerKind::Program),
                _ => None,
            };
            if let Some(kind) = learned {
                self.learn = None;
                emit(ControlEvent::Learned {
                    index,
                    trigger: MidiTrigger {
                        kind,
                        channel: Some(channel),
                        number,
                        mode: TriggerMode::Toggle,
                        forward: false,
                    },
                });
            }
        }

        let Some(list) = &self.list else {
            return false;
        };
        let mut swallow = false;
        for (slot, m) in list.iter().enumerate() {
            let t = &m.trigger;
            if t.number != number || t.channel.is_some_and(|c| c != channel) {
                continue;
            }
            let event = match (t.kind, kind) {
                (TriggerKind::Cc, 0xB0) => {
                    if self.last_cc[slot] == Some(value) {
                        None
                    } else {
                        self.last_cc[slot] = Some(value);
                        let v = match m.ty {
                            ValueType::Bool => (value >= 64) as u8 as f32,
                            ValueType::Float => m.min + (value as f32 / 127.0) * (m.max - m.min),
                        };
                        Some(ControlEvent::Set {
                            index: m.index,
                            value: v,
                        })
                    }
                }
                (TriggerKind::Note, 0x90 | 0x80) => {
                    let pressed = kind == 0x90 && value > 0;
                    match (t.mode, pressed) {
                        (TriggerMode::Toggle, true) => Some(ControlEvent::Toggle { index: m.index }),
                        (TriggerMode::Toggle, false) => None,
                        (TriggerMode::Momentary, on) => Some(ControlEvent::Set {
                            index: m.index,
                            value: if on { m.max } else { m.min },
                        }),
                    }
                }
                (TriggerKind::Program, 0xC0) => Some(ControlEvent::Toggle { index: m.index }),
                _ => continue,
            };
            if let Some(event) = event {
                emit(event);
            }
            swallow |= !t.forward;
        }
        swallow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(index: usize, kind: TriggerKind, channel: Option<u8>, number: u8, ty: ValueType) -> Mapping {
        Mapping {
            index,
            trigger: MidiTrigger {
                kind,
                channel,
                number,
                mode: TriggerMode::Toggle,
                forward: false,
            },
            ty,
            min: 0.0,
            max: 1.0,
        }
    }

    fn run(m: &mut Mappings, bytes: &[u8]) -> (bool, Vec<ControlEvent>) {
        let mut out = Vec::new();
        let swallow = m.process(bytes, |e| out.push(e));
        (swallow, out)
    }

    #[test]
    fn cc_moves_a_slider_and_switches_a_toggle() {
        let mut slider = mapping(0, TriggerKind::Cc, None, 20, ValueType::Float);
        slider.min = 0.2;
        slider.max = 0.6;
        let toggle = mapping(1, TriggerKind::Cc, Some(1), 21, ValueType::Bool);
        let mut m = Mappings::default();
        m.set(vec![slider, toggle].into());

        let (swallow, ev) = run(&mut m, &[0xB3, 20, 127]);
        assert!(swallow);
        assert!(matches!(ev[..], [ControlEvent::Set { index: 0, value }] if (value - 0.6).abs() < 1e-6));
        assert_eq!(run(&mut m, &[0xB0, 20, 127]).1, [], "same value again is skipped");
        let (_, ev) = run(&mut m, &[0xB0, 20, 0]);
        assert!(matches!(ev[..], [ControlEvent::Set { index: 0, value }] if (value - 0.2).abs() < 1e-6));

        assert_eq!(
            run(&mut m, &[0xB0, 21, 64]).1,
            [ControlEvent::Set { index: 1, value: 1.0 }]
        );
        assert_eq!(
            run(&mut m, &[0xB0, 21, 63]).1,
            [ControlEvent::Set { index: 1, value: 0.0 }]
        );
        // Wrong channel (channel 2) and an unmapped CC pass through untouched.
        assert_eq!(run(&mut m, &[0xB1, 21, 127]), (false, vec![]));
        assert_eq!(run(&mut m, &[0xB0, 7, 100]), (false, vec![]));
    }

    #[test]
    fn notes_toggle_or_hold() {
        let toggle = mapping(0, TriggerKind::Note, None, 36, ValueType::Bool);
        let mut hold = mapping(1, TriggerKind::Note, None, 38, ValueType::Float);
        hold.trigger.mode = TriggerMode::Momentary;
        hold.max = 0.8;
        let mut m = Mappings::default();
        m.set(vec![toggle, hold].into());

        assert_eq!(
            run(&mut m, &[0x99, 36, 100]),
            (true, vec![ControlEvent::Toggle { index: 0 }])
        );
        assert_eq!(
            run(&mut m, &[0x89, 36, 0]),
            (true, vec![]),
            "release does nothing but is swallowed"
        );
        let (_, ev) = run(&mut m, &[0x90, 38, 90]);
        assert!(matches!(ev[..], [ControlEvent::Set { index: 1, value }] if (value - 0.8).abs() < 1e-6));
        assert_eq!(
            run(&mut m, &[0x90, 38, 0]).1,
            [ControlEvent::Set { index: 1, value: 0.0 }],
            "Note On with velocity 0 is a release"
        );
    }

    #[test]
    fn program_change_toggles_and_forward_lets_it_through() {
        let mut p = mapping(2, TriggerKind::Program, None, 5, ValueType::Bool);
        p.trigger.forward = true;
        let mut m = Mappings::default();
        m.set(vec![p].into());
        assert_eq!(
            run(&mut m, &[0xC0, 5]),
            (false, vec![ControlEvent::Toggle { index: 2 }])
        );
        assert_eq!(run(&mut m, &[0xC0, 6]), (false, vec![]));
    }

    #[test]
    fn learn_takes_the_next_usable_message_once() {
        let mut m = Mappings::default();
        m.learn(Some(4));
        assert!(!m.is_idle());
        assert_eq!(run(&mut m, &[0xE0, 0, 64]).1, [], "pitch bend is not learnable");
        assert_eq!(run(&mut m, &[0x80, 60, 0]).1, [], "note off is not learnable");
        let (swallow, ev) = run(&mut m, &[0xB2, 74, 10]);
        assert!(!swallow, "learning does not swallow");
        assert_eq!(
            ev,
            [ControlEvent::Learned {
                index: 4,
                trigger: MidiTrigger {
                    kind: TriggerKind::Cc,
                    channel: Some(3),
                    number: 74,
                    mode: TriggerMode::Toggle,
                    forward: false
                }
            }]
        );
        assert_eq!(run(&mut m, &[0x90, 60, 100]).1, [], "only once");
        assert!(m.is_idle());
    }
}
