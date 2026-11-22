// Small pure helpers shared by views and the mock.

import { formatClock, formatNumber, formatRelative, localeTag, t } from './i18n/i18n.svelte';
import type { ListenAddress, MidiTrigger, OscRole } from './ipc';

const NOTE_NAMES = ['C', 'C♯', 'D', 'D♯', 'E', 'F', 'F♯', 'G', 'G♯', 'A', 'A♯', 'B'];

/** Scientific pitch name of a MIDI note: 60 is "C4", 61 "C♯4". The same in every language. */
export function noteName(n: number): string {
  return `${NOTE_NAMES[((n % 12) + 12) % 12]}${Math.floor(n / 12) - 1}`;
}

/** Short label of a MIDI trigger: "CC 20 · Ch 1", "Note C4", "PC 5". */
export function triggerLabel(m: MidiTrigger): string {
  const what =
    m.kind === 'cc'
      ? t('midi.badgeCc', { number: m.number })
      : m.kind === 'note'
        ? t('midi.badgeNote', { note: noteName(m.number) })
        : t('midi.badgeProgram', { number: m.number });
  return m.channel ? `${what} · ${t('midi.badgeChannel', { channel: m.channel })}` : what;
}

/** Last part of a path, Windows or Unix. */
export function fileName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

/** "C:\…\Projects\Live.vrcmt": the root and the last two parts of a long path. */
export function shortPath(path: string, max = 44): string {
  if (path.length <= max) return path;
  const sep = path.includes('\\') ? '\\' : '/';
  const parts = path.split(/[\\/]/);
  if (parts.length <= 3) return path;
  const head = parts[0] || '';
  return [head, '…', ...parts.slice(-2)].join(sep);
}

/** "4.2 MB" in the active language. */
export function formatBytes(bytes: number): string {
  const mb = bytes / 1_000_000;
  return formatNumber(mb, { style: 'unit', unit: 'megabyte', maximumFractionDigits: mb < 10 ? 1 : 0 });
}

const dateFormats = new Map<string, Intl.DateTimeFormat>();

/** A calendar date in the active language; the input as given when it cannot be read. */
export function formatDate(value: string | number): string {
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return String(value);
  const tag = localeTag();
  let f = dateFormats.get(tag);
  if (!f) dateFormats.set(tag, (f = new Intl.DateTimeFormat(tag, { dateStyle: 'medium' })));
  return f.format(d);
}

/** Default OSC path of each listening role (see the transposer-protocols skill). */
export const DEFAULT_LISTEN_PATHS: Record<OscRole, string> = {
  transpose: '/transpose',
  transpose_up: '/transposeUp',
  transpose_down: '/transposeDown',
  debug_set: '/debug',
  midi_list: '/midi/list',
  osc_send_original: '/osc/original',
  osc_send_transposed: '/osc/transposed',
};

export function defaultListenAddresses(): ListenAddress[] {
  return (Object.keys(DEFAULT_LISTEN_PATHS) as OscRole[]).map((role) => ({
    addr: DEFAULT_LISTEN_PATHS[role],
    role,
  }));
}

/**
 * "+3", "0", "−3" with a real minus sign so widths stay even with tabular
 * numbers. Digits follow the active language. For display only.
 */
export function signed(n: number): string {
  if (n > 0) return `+${formatNumber(n)}`;
  if (n < 0) return `−${formatNumber(Math.abs(n))}`;
  return formatNumber(0);
}

/** MQTT slug of a custom control: lowercased, spaces replaced by underscores. */
export function slug(name: string): string {
  return name.toLowerCase().replaceAll(' ', '_');
}

export function clamp(n: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, n));
}

/** Trims float noise for display: 0.30000000000000004 becomes "0.3" (or "0,3"). */
export function shortNumber(n: number, digits = 2): string {
  return formatNumber(Number(n.toFixed(digits)), { maximumFractionDigits: digits });
}

/** 0-100 as "50 %" (or "50%", "%50") in the active language. */
export function percent(n: number): string {
  return formatNumber(n / 100, { style: 'percent', maximumFractionDigits: 0 });
}

/** Beats per minute with one decimal: "120.0". */
export function bpmNumber(bpm: number): string {
  return formatNumber(bpm, { minimumFractionDigits: 1, maximumFractionDigits: 1 });
}

/** "just now", "42 seconds ago", ... in the active language. */
export function relativeTime(ts: number, now: number): string {
  return formatRelative(ts, now);
}

/** HH:MM:SS, 24-hour, in the active language. */
export function clockTime(ts: number): string {
  return formatClock(ts);
}
