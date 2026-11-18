// Small pure helpers shared by views and the mock.

import type { ListenAddress, OscRole } from './ipc';

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

/** "+3", "0", "−3" with a real minus sign so widths stay even with tabular numbers. */
export function signed(n: number): string {
  if (n > 0) return `+${n}`;
  if (n < 0) return `−${Math.abs(n)}`;
  return '0';
}

/** MQTT slug of a custom control: lowercased, spaces replaced by underscores. */
export function slug(name: string): string {
  return name.toLowerCase().replaceAll(' ', '_');
}

export function clamp(n: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, n));
}

/** Trims float noise for display: 0.30000000000000004 becomes "0.3". */
export function shortNumber(n: number, digits = 2): string {
  return String(Number(n.toFixed(digits)));
}

export function relativeTime(ts: number, now: number): string {
  const s = Math.max(0, Math.round((now - ts) / 1000));
  if (s < 5) return 'just now';
  if (s < 60) return `${s} seconds ago`;
  const m = Math.round(s / 60);
  if (m < 60) return m === 1 ? '1 minute ago' : `${m} minutes ago`;
  const h = Math.round(m / 60);
  if (h < 24) return h === 1 ? '1 hour ago' : `${h} hours ago`;
  const d = Math.round(h / 24);
  return d === 1 ? 'yesterday' : `${d} days ago`;
}

export function clockTime(ts: number): string {
  const d = new Date(ts);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}
