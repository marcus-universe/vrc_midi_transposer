// Typed contract between the Svelte UI and the Rust shell (src-tauri/src/gui.rs).
// This is the only module that talks to Tauri. Keep it in sync with gui.rs.

import { Channel, invoke } from '@tauri-apps/api/core';

// ---------------------------------------------------------------------------
// Engine state
// ---------------------------------------------------------------------------

export type LogLevel = 'info' | 'debug' | 'warn' | 'error' | 'success' | 'accent';

export interface LogLine {
  /** Increasing sequence number; use as the list key. */
  id: number;
  /** Milliseconds since the Unix epoch. */
  ts: number;
  level: LogLevel;
  text: string;
}

export interface PortInfo {
  id: string;
  name: string;
}

/** `none`: nothing selected. `missing`: selected but the device is not present. */
export type MidiConnState = 'none' | 'connected' | 'missing' | 'error';

export interface MidiStatus {
  state: MidiConnState;
  /** Id of the connected port (matches a PortInfo.id); empty unless state is `connected`. */
  id: string;
  /** Name of the connected port, or of the selected port when not connected; empty when none. */
  name: string;
  /** Error text when state is `error`. */
  detail: string;
}

export type LastCommand =
  | { kind: 'transpose'; value: number }
  | { kind: 'transpose_up' }
  | { kind: 'transpose_down' }
  | { kind: 'osc_sending'; on: boolean }
  | { kind: 'send_original'; on: boolean }
  | { kind: 'debug'; on: boolean }
  /** A custom control; `value` is the state payload (`1`/`0` or a number). */
  | { kind: 'custom'; name: string; value: string };

export type ServiceState = 'stopped' | 'running' | 'error';
export type MqttConnState = 'disabled' | 'connecting' | 'connected' | 'error';

export interface Snapshot {
  transpose: number;
  transpose_min: number;
  transpose_max: number;
  debug: boolean;
  osc_sending: boolean;
  send_original: boolean;
  /** Session state of the MQTT client (the console can toggle it). */
  mqtt_enabled: boolean;
  /** Current value of each custom control, same order as project.osc.sending_addresses. */
  custom_values: number[];
  inputs: PortInfo[];
  outputs: PortInfo[];
  midi_in: MidiStatus;
  midi_out: MidiStatus;
  osc_listener: { state: ServiceState; detail: string; bound: string };
  /** Resolved OSC target, `host:port`. */
  osc_target: string;
  mqtt: {
    state: MqttConnState;
    detail: string;
    /** Number of Home Assistant discovery entities published. */
    entities: number;
    /** What the last command received over MQTT did; null if none. */
    last_command: LastCommand | null;
    /** Milliseconds since the Unix epoch, 0 if none. */
    last_command_ts: number;
  };
}

// ---------------------------------------------------------------------------
// Project
// ---------------------------------------------------------------------------

export type OscRole =
  | 'transpose'
  | 'transpose_up'
  | 'transpose_down'
  | 'debug_set'
  | 'midi_list'
  | 'osc_send_original'
  | 'osc_send_transposed';

export const OSC_ROLES: { role: OscRole; label: string; hint: string }[] = [
  { role: 'transpose', label: 'Set transpose', hint: 'Number: semitones' },
  { role: 'transpose_up', label: 'Transpose up', hint: '1 or true: +1' },
  { role: 'transpose_down', label: 'Transpose down', hint: '1 or true: −1' },
  { role: 'debug_set', label: 'Debug on/off', hint: 'Number or bool' },
  { role: 'midi_list', label: 'List MIDI devices', hint: 'Any message' },
  { role: 'osc_send_original', label: 'Send original notes', hint: 'Any message' },
  { role: 'osc_send_transposed', label: 'Send transposed notes', hint: 'Any message' },
];

export interface PortRef {
  id: string;
  name: string;
}

export interface ListenAddress {
  addr: string;
  role: OscRole | null;
}

export interface SendAddress {
  name: string;
  addr: string;
  type: 'bool' | 'float';
  default: number;
  min: number | null;
  max: number | null;
}

export interface Project {
  schema: number;
  id: string;
  name: string;
  midi: { input: PortRef | null; output: PortRef | null };
  osc: {
    listen_host: string;
    listen_port: number;
    target_host: string;
    target_port: number;
    /** Applied when the project is opened. */
    sending_enabled: boolean;
    /** Applied when the project is opened. */
    send_original: boolean;
    listening_addresses: ListenAddress[];
    sending_addresses: SendAddress[];
  };
  mqtt: {
    enabled: boolean;
    broker_host: string;
    broker_port: number;
    base_topic: string;
    username: string;
    /**
     * Write-only. Always arrives empty. Send a non-empty string to replace the
     * stored password; send empty with `password_set: true` to keep it, or with
     * `password_set: false` to clear it.
     */
    password: string;
    password_set: boolean;
  };
  transpose: { min: number; max: number };
  /** Applied when the project is opened. */
  debug: boolean;
}

export interface ProjectInfo {
  id: string;
  name: string;
  /** Milliseconds since the Unix epoch. */
  modified: number;
}

/** Returned by every project command. */
export interface ProjectState {
  project: Project;
  /** True when the working copy differs from the saved file. */
  dirty: boolean;
  projects: ProjectInfo[];
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

export interface AppInfo {
  name: string;
  version: string;
  owner: string;
  email: string;
  license: string;
  repository: string;
  data_dir: string;
}

export interface Init extends ProjectState {
  app: AppInfo;
  /** ASCII art banner, printed at the top of the console. */
  banner: string;
  snapshot: Snapshot;
  /** Free-form UI settings saved with `saveUi` (console height, last view, ...). */
  ui: Record<string, unknown>;
  logs: LogLine[];
  keys: number[];
  vel: number[];
  wheels: [number, number];
}

/**
 * Coalesced update, at most one per display frame. Every field is optional and
 * carries full state, never a delta (except `logs`, which are new lines).
 */
export interface Frame {
  snap?: Snapshot;
  logs?: LogLine[];
  /**
   * Pressed keys as eight unsigned 32-bit words: words 0-3 are the input notes
   * 0-127, words 4-7 the transposed output notes. Bit `n % 32` of word
   * `n >> 5` is note `n`.
   */
  keys?: number[];
  /**
   * Velocity (1-127) of every key set in `keys`, in bit order: input words
   * first, lowest note first. Always sent together with `keys`.
   */
  vel?: number[];
  /** `[pitch bend 0..16383 (8192 = centre), mod wheel 0..127]`. Sent together with `keys`. */
  wheels?: [number, number];
  /** The log was cleared; drop all lines before applying `logs`. */
  cleared?: boolean;
  /** The user tried to close the window with unsaved changes. Ask, then call `quit`. */
  close_requested?: boolean;
}

export type EngineCmd =
  | { type: 'set_transpose'; value: number }
  | { type: 'nudge'; delta: number }
  | { type: 'set_osc_sending'; on: boolean }
  | { type: 'set_send_original'; on: boolean }
  | { type: 'set_debug'; on: boolean }
  | { type: 'custom_osc'; index: number; value: number }
  | { type: 'refresh_ports' }
  /** Release every held note on the output and over OSC. */
  | { type: 'panic' };

// ---------------------------------------------------------------------------
// Transport
// ---------------------------------------------------------------------------

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) return invoke<T>(cmd, args);
  const mock = await import('./mock');
  return mock.mockInvoke(cmd, args) as Promise<T>;
}

export const api = {
  /** Start receiving frames. Call once at startup; calling again replaces the listener. */
  async subscribe(onFrame: (frame: Frame) => void): Promise<Init> {
    if (isTauri) {
      const channel = new Channel<Frame>();
      channel.onmessage = onFrame;
      return invoke<Init>('subscribe', { channel });
    }
    const mock = await import('./mock');
    return mock.mockSubscribe(onFrame);
  },
  /** Pause frames while the window is hidden. A full frame follows when it becomes visible. */
  setVisible: (visible: boolean) => call<void>('set_visible', { visible }),
  engine: (cmd: EngineCmd) => call<void>('engine_cmd', { cmd }),
  /** Run a console command line. Output arrives as log lines. */
  consoleLine: (line: string) => call<void>('console_line', { line }),
  clearLog: () => call<void>('clear_log'),

  /** Apply the edited working copy to the running engine. Does not write to disk. */
  updateProject: (project: Project) => call<ProjectState>('update_project', { project }),
  saveProject: () => call<ProjectState>('save_project'),
  revertProject: () => call<ProjectState>('revert_project'),
  openProject: (id: string) => call<ProjectState>('open_project', { id }),
  newProject: (name: string) => call<ProjectState>('new_project', { name }),
  /** Save the working copy as a new project and switch to it. */
  duplicateProject: (name: string) => call<ProjectState>('duplicate_project', { name }),
  deleteProject: (id: string) => call<ProjectState>('delete_project', { id }),
  /** Opens a file dialog. Accepts a v1 `config.json` or an exported project. Null if cancelled. */
  importProject: () => call<ProjectState | null>('import_project'),
  /** Opens a save dialog. Returns the written path, or null if cancelled. The password is not exported. */
  exportProject: () => call<string | null>('export_project'),

  saveUi: (ui: Record<string, unknown>) => call<void>('save_ui', { ui }),
  openRepository: () => call<void>('open_repository'),
  /** Close the app. With `force: false` the shell asks again if there are unsaved changes. */
  quit: (force: boolean) => call<void>('quit', { force }),
};
