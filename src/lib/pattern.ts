// Arpeggiator and sequencer helpers shared by the views and the mock. Defaults
// match crates/engine/src/project/pattern.rs.

import { t } from './i18n/i18n.svelte';
import { MAX_STEPS, type PatternSettings, type Rate, type Step, type Transport } from './ipc';

export const BPM_MIN = 20;
export const BPM_MAX = 300;
export const OFFSET_MAX = 48;

export function defaultStep(): Step {
  return { on: true, offset: 0, velocity: 100, gate: 50, tie: false };
}

export function defaultPattern(): PatternSettings {
  // The engine's default: a simple pattern so Play does something right away.
  const offsets = [0, 7, 12, 7, 3, 7, 10, 7, 0, 7, 12, 7, 5, 7, 10, 12];
  const steps = Array.from({ length: MAX_STEPS }, (_, i) => ({ ...defaultStep(), offset: offsets[i] ?? 0 }));
  return {
    mode: 'off',
    clock: { source: 'input', device: null, internal_fallback: true, bpm: 120 },
    arp: { style: 'up', rate: '1/16', octaves: 1, gate: 50, swing: 0, latch: false, velocity: null },
    seq: { rate: '1/16', length: 16, root: 48, swing: 0, channel: 1, follow_transport: true, steps },
    midi: {},
  };
}

export const IDLE_TRANSPORT: Transport = {
  clock: 'none',
  bpm: 0,
  beat: 0,
  playing: false,
  armed: false,
  step: -1,
  rec: 'off',
};

/** MIDI clock ticks (24 per quarter note) of one step. */
export function rateTicks(rate: Rate): number {
  const base = { '1/1': 96, '1/2': 48, '1/4': 24, '1/8': 12, '1/16': 6, '1/32': 3 }[rate.replace(/[TD]$/, '')] ?? 6;
  if (rate.endsWith('T')) return (base * 2) / 3;
  if (rate.endsWith('D')) return (base * 3) / 2;
  return base;
}

/** "1/16", "1/8 triplet", "1/4 dotted" in the active language. */
export function rateLabel(rate: Rate): string {
  const plain = rate.replace(/[TD]$/, '');
  if (rate.endsWith('T')) return t('pattern.rateTriplet', { rate: plain });
  if (rate.endsWith('D')) return t('pattern.rateDotted', { rate: plain });
  return plain;
}

// ---------------------------------------------------------------------------
// Quick actions on the steps that play (the first `length`)
// ---------------------------------------------------------------------------

/** Every step off, with default values. */
export function clearSteps(steps: Step[], length: number) {
  for (let i = 0; i < length; i++) Object.assign(steps[i], defaultStep(), { on: false });
}

/** New offsets within an octave up or down; no scale. */
export function randomizeSteps(steps: Step[], length: number) {
  for (let i = 0; i < length; i++) steps[i].offset = Math.floor(Math.random() * 25) - 12;
}

/** Rotate by one step; `dir` -1 moves every step one to the left (the first wraps to the end). */
export function shiftSteps(steps: Step[], length: number, dir: -1 | 1) {
  if (length < 2) return;
  const values = steps.slice(0, length).map((s) => ({ ...s }));
  for (let i = 0; i < length; i++) {
    Object.assign(steps[i], values[(i - dir + length) % length]);
  }
}
