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
  /** Index of the custom control waiting for MIDI Learn, or null. */
  learning: number | null;
  /** Last MIDI Learn result. `seq` grows with every result, so a new one can be told apart. */
  learned: Learned | null;
}

export interface Learned {
  seq: number;
  index: number;
  trigger: MidiTrigger;
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

/** Every listening role, in menu order. Labels and hints are in the dictionaries (`osc.roles.<role>`). */
export const OSC_ROLES: readonly OscRole[] = [
  'transpose',
  'transpose_up',
  'transpose_down',
  'debug_set',
  'midi_list',
  'osc_send_original',
  'osc_send_transposed',
];

export interface PortRef {
  id: string;
  name: string;
}

export interface ListenAddress {
  addr: string;
  role: OscRole | null;
}

export type TriggerKind = 'cc' | 'note' | 'program';

/** A MIDI message that sets a custom control (see the transposer-protocols skill). */
export interface MidiTrigger {
  kind: TriggerKind;
  /** 1-16, or null for any channel. */
  channel: number | null;
  /** CC number, note number or program number, 0-127. */
  number: number;
  /** Notes only; the engine stores `toggle` for the other kinds. */
  mode: 'toggle' | 'momentary';
  /** Also send the message on to the MIDI output. */
  forward: boolean;
}

export interface SendAddress {
  name: string;
  addr: string;
  type: 'bool' | 'float';
  default: number;
  min: number | null;
  max: number | null;
  /** Absent when the control has no MIDI trigger (the shell also accepts null). */
  midi?: MidiTrigger | null;
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

export interface RecentFile {
  path: string;
  /** Project name stored in the file, or the file name if it cannot be read. */
  name: string;
  exists: boolean;
}

/** Returned by every project command. */
export interface ProjectState {
  project: Project;
  /** True when the working copy differs from the saved file. */
  dirty: boolean;
  /** The project file, or null for a project that was never saved (new, or an imported v1 config.json). */
  path: string | null;
  /** Recently used project files, newest first. */
  recent: RecentFile[];
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
  /** Where Save As starts for a project that has no file yet. */
  documents_dir: string;
  /**
   * `auto`: the app downloads and installs updates (Windows installer, macOS app, AppImage).
   * `notify`: only tell the user and link to the release page (deb, rpm).
   * `none`: development build, no updates.
   */
  update_mode: UpdateMode;
}

export type UpdateMode = 'auto' | 'notify' | 'none';

/** A newer version, as found by `checkUpdate`. */
export interface UpdateInfo {
  version: string;
  current: string;
  /** Publication date as given in latest.json (RFC 3339), or null. */
  date: string | null;
  /** Release notes, plain text. */
  notes: string;
}

/** Progress of `installUpdate`, sent as `Frame.update`. */
export type UpdateProgress =
  | { state: 'downloading'; downloaded: number; total: number | null }
  | { state: 'installing' }
  | { state: 'error'; detail: string };

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
  /**
   * The system asked to open a file (double-click on a .vrcmt while the app
   * runs). Ask about unsaved changes, then call `takePendingOpen`.
   */
  open_pending?: boolean;
  /** Progress of `installUpdate`. */
  update?: UpdateProgress;
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
  | { type: 'panic' }
  /** MIDI Learn for a custom control; `index: null` cancels. The result arrives as `Snapshot.learned`. */
  | { type: 'midi_learn'; index: number | null };

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
  /** Write the project to its file. Rejects with `NO_PATH` when it has none: use `saveProjectAs`. */
  saveProject: () => call<ProjectState>('save_project'),
  /** Native save dialog titled `title`, then save there. Null if cancelled. */
  saveProjectAs: (title: string) => call<ProjectState | null>('save_project_as', { title }),
  revertProject: () => call<ProjectState>('revert_project'),
  /** Replace the open project with an untitled default one (no file yet). */
  newProject: () => call<ProjectState>('new_project'),
  /** Native open dialog titled `title`; accepts .vrcmt and a v1 config.json. Null if cancelled. */
  openProjectDialog: (title: string) => call<ProjectState | null>('open_project_dialog', { title }),
  /** Open a file by path (recent files, files the system asked to open). */
  openFile: (path: string) => call<ProjectState>('open_file', { path }),
  removeRecent: (path: string) => call<ProjectState>('remove_recent', { path }),
  /** The file the system asked to open, if any; clears the request. */
  takePendingOpen: () => call<string | null>('take_pending_open'),

  saveUi: (ui: Record<string, unknown>) => call<void>('save_ui', { ui }),
  openRepository: () => call<void>('open_repository'),
  /** Open the release page of a version in the default browser. */
  openRelease: (version: string) => call<void>('open_release', { version }),
  /** Null when up to date (also when no release has update data yet). Rejects with a plain English sentence. */
  checkUpdate: () => call<UpdateInfo | null>('check_update'),
  /** Only in `auto` mode. Progress arrives as `Frame.update`; the app restarts afterwards. Ask about unsaved changes first. */
  installUpdate: () => call<void>('install_update'),
  /** Close the app. With `force: false` the shell asks again if there are unsaved changes. */
  quit: (force: boolean) => call<void>('quit', { force }),
};
