// The single reactive store. Engine state arrives as coalesced frames and lands
// here; components read from it and never keep their own copy of engine state.

import {
  api,
  type AppInfo,
  type EngineCmd,
  type Frame,
  type LogLine,
  type Project,
  type ProjectInfo,
  type ProjectState,
  type Snapshot,
} from './ipc';

export type View = 'dashboard' | 'osc' | 'mqtt' | 'settings';
export const VIEWS: View[] = ['dashboard', 'osc', 'mqtt', 'settings'];

export interface UiState {
  view: View;
  console_open: boolean;
  console_height: number;
  piano_source: 'input' | 'output';
}

export type DialogState =
  | { kind: 'quit' }
  | { kind: 'unsaved'; action: string; proceed: () => void | Promise<void> }
  | { kind: 'revert' }
  | { kind: 'name'; mode: 'new' | 'duplicate'; initial: string }
  | { kind: 'delete' }
  | { kind: 'about' };

export interface Notice {
  kind: 'error' | 'info';
  text: string;
}

export const MAX_LOG_LINES = 2000;
export const CONSOLE_MIN_HEIGHT = 120;
const UPDATE_DEBOUNCE_MS = 400;
const UI_SAVE_DEBOUNCE_MS = 500;

const EMPTY_SNAPSHOT: Snapshot = {
  transpose: 0,
  transpose_min: -24,
  transpose_max: 24,
  debug: false,
  osc_sending: false,
  send_original: true,
  mqtt_enabled: false,
  custom_values: [],
  inputs: [],
  outputs: [],
  midi_in: { state: 'none', id: '', name: '', detail: '' },
  midi_out: { state: 'none', id: '', name: '', detail: '' },
  osc_listener: { state: 'stopped', detail: '', bound: '' },
  osc_target: '',
  mqtt: { state: 'disabled', detail: '', entities: 0, ha: 'unknown', last_command: '', last_command_ts: 0 },
};

const EMPTY_PROJECT: Project = {
  schema: 1,
  id: '',
  name: '',
  midi: { input: null, output: null },
  osc: {
    listen_host: '127.0.0.1',
    listen_port: 9069,
    target_host: '127.0.0.1',
    target_port: 9000,
    sending_enabled: false,
    send_original: true,
    listening_addresses: [],
    sending_addresses: [],
  },
  mqtt: {
    enabled: false,
    broker_host: '127.0.0.1',
    broker_port: 1883,
    base_topic: 'midi_transposer',
    username: '',
    password: '',
    password_set: false,
  },
  transpose: { min: -24, max: 24 },
  debug: false,
};

const EMPTY_APP: AppInfo = {
  name: 'VRC MIDI Transposer',
  version: '',
  owner: '',
  email: '',
  license: '',
  repository: '',
  data_dir: '',
};

type KeysListener = (keys: readonly number[]) => void;
type LogListener = (added: readonly LogLine[], cleared: boolean) => void;

function errorText(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return String(e);
}

class AppStore {
  ready = $state(false);
  snapshot = $state.raw<Snapshot>(EMPTY_SNAPSHOT);
  /** Working copy. Edits mutate it and are sent to the engine debounced. */
  project = $state<Project>(structuredClone(EMPTY_PROJECT));
  dirty = $state(false);
  projects = $state.raw<ProjectInfo[]>([]);
  app = $state.raw<AppInfo>(EMPTY_APP);
  banner = $state('');
  ui = $state<UiState>({
    view: 'dashboard',
    console_open: false,
    console_height: 240,
    piano_source: 'input',
  });
  /** Bumped whenever the working copy is replaced, so forms rebuild from it. */
  epoch = $state(0);
  dialog = $state.raw<DialogState | null>(null);
  notice = $state.raw<Notice | null>(null);
  busy = $state(false);
  /** Wall clock, advanced only when a frame arrives. Drives relative times. */
  now = $state(Date.now());

  // Not reactive on purpose: the piano and the console subscribe and patch the DOM.
  logs: LogLine[] = [];
  keys: number[] = [0, 0, 0, 0, 0, 0, 0, 0];
  /** Set when the user opens the console, so its input takes focus once mounted. */
  focusConsole = false;

  #keysListeners = new Set<KeysListener>();
  #logListeners = new Set<LogListener>();
  #updateTimer: ReturnType<typeof setTimeout> | undefined;
  #uiTimer: ReturnType<typeof setTimeout> | undefined;
  #editSeq = 0;
  /** The working copy has edits the engine has not received yet. */
  #unsent = false;
  #inFlight: Promise<void> = Promise.resolve();
  #pendingCmds = new Map<string, EngineCmd>();
  #raf = 0;

  // -------------------------------------------------------------------------
  // Startup and frames
  // -------------------------------------------------------------------------

  async start() {
    try {
      const init = await api.subscribe(this.#applyFrame);
      this.app = init.app;
      this.banner = init.banner;
      this.snapshot = init.snapshot;
      this.project = init.project;
      this.dirty = init.dirty;
      this.projects = init.projects;
      this.logs = init.logs.slice(-MAX_LOG_LINES);
      this.keys = init.keys;
      this.#restoreUi(init.ui);
      this.ready = true;
      this.#emitLogs([], true);
      this.#emitKeys();
    } catch (e) {
      this.notice = { kind: 'error', text: `Could not connect to the engine. ${errorText(e)}` };
      this.ready = true;
    }
  }

  #applyFrame = (frame: Frame) => {
    if (frame.snap) this.snapshot = frame.snap;
    if (frame.cleared) this.logs = [];
    if (frame.logs?.length) {
      this.logs.push(...frame.logs);
      if (this.logs.length > MAX_LOG_LINES) this.logs.splice(0, this.logs.length - MAX_LOG_LINES);
    }
    if (frame.cleared || frame.logs?.length) this.#emitLogs(frame.logs ?? [], !!frame.cleared);
    if (frame.keys) {
      this.keys = frame.keys;
      this.#emitKeys();
    }
    if (frame.close_requested && this.dialog?.kind !== 'quit') this.dialog = { kind: 'quit' };
    const t = Date.now();
    if (t - this.now >= 1000) this.now = t;
  };

  onKeys(fn: KeysListener): () => void {
    this.#keysListeners.add(fn);
    return () => this.#keysListeners.delete(fn);
  }

  onLogs(fn: LogListener): () => void {
    this.#logListeners.add(fn);
    return () => this.#logListeners.delete(fn);
  }

  #emitKeys() {
    for (const fn of this.#keysListeners) fn(this.keys);
  }

  #emitLogs(added: readonly LogLine[], cleared: boolean) {
    for (const fn of this.#logListeners) fn(added, cleared);
  }

  // -------------------------------------------------------------------------
  // Engine commands (session state)
  // -------------------------------------------------------------------------

  /** Resolves to false (after showing the error) when the engine refuses the command. */
  engine(cmd: EngineCmd): Promise<boolean> {
    return api.engine(cmd).then(
      () => true,
      (e) => {
        this.fail(e);
        return false;
      },
    );
  }

  /** For sliders: at most one command per key per display frame, last value wins. */
  engineCoalesced(key: string, cmd: EngineCmd) {
    this.#pendingCmds.set(key, cmd);
    if (this.#raf) return;
    this.#raf = requestAnimationFrame(() => {
      this.#raf = 0;
      const cmds = [...this.#pendingCmds.values()];
      this.#pendingCmds.clear();
      for (const c of cmds) void this.engine(c);
    });
  }

  consoleLine(line: string) {
    return api.consoleLine(line).catch((e) => this.fail(e));
  }

  clearLog() {
    return api.clearLog().catch((e) => this.fail(e));
  }

  // -------------------------------------------------------------------------
  // Project edits
  // -------------------------------------------------------------------------

  /**
   * Call after mutating `project`. Sends the working copy to the engine after a
   * short pause, or right away with `now` (selects, switches, blur).
   */
  touch = (now = false) => {
    this.#editSeq++;
    this.#unsent = true;
    this.dirty = true;
    clearTimeout(this.#updateTimer);
    if (now) void this.flush();
    else this.#updateTimer = setTimeout(() => void this.flush(), UPDATE_DEBOUNCE_MS);
  };

  /** Send pending edits now. Resolves when the engine has them. */
  flush = (): Promise<void> => {
    clearTimeout(this.#updateTimer);
    if (!this.#unsent) return this.#inFlight;
    this.#unsent = false;
    const seq = this.#editSeq;
    const sent = $state.snapshot(this.project) as Project;
    this.#inFlight = this.#inFlight
      .then(() => api.updateProject(sent))
      .then((st) => this.#applyLight(st, seq, sent))
      .catch((e) => this.fail(e));
    return this.#inFlight;
  };

  /**
   * Result of an update or a save: take `dirty` and the project list, but leave
   * the working copy alone so a field that is being typed in is not overwritten.
   */
  #applyLight(st: ProjectState, seq: number, sent?: Project) {
    this.projects = st.projects;
    if (seq === this.#editSeq) this.dirty = st.dirty;
    // The password is write-only: once the engine has it, forget it here.
    if (sent && sent.mqtt.password !== '' && this.project.mqtt.password === sent.mqtt.password) {
      this.project.mqtt.password = '';
    }
    if (this.project.mqtt.password === '') {
      this.project.mqtt.password_set = st.project.mqtt.password_set;
    }
  }

  #dropUnsent() {
    clearTimeout(this.#updateTimer);
    this.#unsent = false;
  }

  /** Result of a command that replaces the working copy. */
  #applyFull(st: ProjectState) {
    this.#dropUnsent();
    this.#editSeq++;
    this.project = st.project;
    this.dirty = st.dirty;
    this.projects = st.projects;
    this.epoch++;
  }

  async #run(fn: () => Promise<void>): Promise<boolean> {
    this.busy = true;
    try {
      await fn();
      return true;
    } catch (e) {
      this.fail(e);
      return false;
    } finally {
      this.busy = false;
    }
  }

  save(): Promise<boolean> {
    return this.#run(async () => {
      await this.flush();
      const seq = this.#editSeq;
      this.#applyLight(await api.saveProject(), seq);
    });
  }

  revert(): Promise<boolean> {
    return this.#run(async () => {
      await this.flush();
      this.#applyFull(await api.revertProject());
    });
  }

  /** Runs `proceed` right away, or asks first when there are unsaved changes. */
  guard(action: string, proceed: () => void | Promise<void>) {
    if (this.dirty) this.dialog = { kind: 'unsaved', action, proceed };
    else void proceed();
  }

  openProject(id: string) {
    if (id === this.project.id) return;
    const name = this.projects.find((p) => p.id === id)?.name ?? 'the project';
    this.guard(`open “${name}”`, () =>
      this.#run(async () => {
        await this.flush();
        this.#applyFull(await api.openProject(id));
      }).then(() => undefined),
    );
  }

  askNewProject() {
    this.guard('create a new project', () => {
      this.dialog = { kind: 'name', mode: 'new', initial: 'New project' };
    });
  }

  askDuplicateProject() {
    this.dialog = { kind: 'name', mode: 'duplicate', initial: `${this.project.name} copy` };
  }

  createProject(mode: 'new' | 'duplicate', name: string): Promise<boolean> {
    return this.#run(async () => {
      await this.flush();
      const st = mode === 'new' ? await api.newProject(name) : await api.duplicateProject(name);
      this.#applyFull(st);
    });
  }

  importProject() {
    this.guard('import a project', () =>
      this.#run(async () => {
        await this.flush();
        const st = await api.importProject();
        if (st) {
          this.#applyFull(st);
          this.notice = { kind: 'info', text: `Imported “${st.project.name}”.` };
        }
      }).then(() => undefined),
    );
  }

  exportProject(): Promise<boolean> {
    return this.#run(async () => {
      await this.flush();
      const path = await api.exportProject();
      if (path) this.notice = { kind: 'info', text: `Exported to ${path}. The MQTT password is not included.` };
    });
  }

  deleteProject(): Promise<boolean> {
    return this.#run(async () => {
      this.#dropUnsent();
      await this.#inFlight;
      this.#applyFull(await api.deleteProject(this.project.id));
    });
  }

  // -------------------------------------------------------------------------
  // Quit
  // -------------------------------------------------------------------------

  async saveAndQuit() {
    if (await this.save()) await api.quit(true).catch((e) => this.fail(e));
  }

  discardAndQuit() {
    return api.quit(true).catch((e) => this.fail(e));
  }

  // -------------------------------------------------------------------------
  // UI state
  // -------------------------------------------------------------------------

  setView(view: View) {
    if (this.ui.view === view) return;
    this.ui.view = view;
    this.saveUiSoon();
  }

  toggleConsole(open = !this.ui.console_open) {
    this.focusConsole = open;
    this.ui.console_open = open;
    this.saveUiSoon();
  }

  setPianoSource(source: 'input' | 'output') {
    this.ui.piano_source = source;
    this.saveUiSoon();
  }

  saveUiSoon() {
    clearTimeout(this.#uiTimer);
    this.#uiTimer = setTimeout(() => {
      api.saveUi($state.snapshot(this.ui) as unknown as Record<string, unknown>).catch(() => {});
    }, UI_SAVE_DEBOUNCE_MS);
  }

  #restoreUi(saved: Record<string, unknown>) {
    const view = saved.view;
    if (typeof view === 'string' && (VIEWS as string[]).includes(view)) this.ui.view = view as View;
    if (typeof saved.console_open === 'boolean') this.ui.console_open = saved.console_open;
    const h = saved.console_height;
    if (typeof h === 'number' && Number.isFinite(h)) this.ui.console_height = Math.max(CONSOLE_MIN_HEIGHT, Math.round(h));
    if (saved.piano_source === 'input' || saved.piano_source === 'output') {
      this.ui.piano_source = saved.piano_source;
    }
  }

  fail(e: unknown) {
    this.notice = { kind: 'error', text: errorText(e) };
  }
}

export const app = new AppStore();
