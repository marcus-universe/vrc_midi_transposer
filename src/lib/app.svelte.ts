// The single reactive store. Engine state arrives as coalesced frames and lands
// here; components read from it and never keep their own copy of engine state.

import {
  api,
  type AppInfo,
  type EngineCmd,
  type Frame,
  type LogLine,
  type PatternAction,
  type PatternMode,
  type Project,
  type ProjectState,
  type RecentFile,
  type Snapshot,
  type Transport,
  type UpdateInfo,
  type UpdateProgress,
} from './ipc';
import { fileName } from './format';
import { IDLE_TRANSPORT, defaultPattern } from './pattern';
import { detectLocale, isLocale, setLocale, t, type Locale } from './i18n/i18n.svelte';

export type View = 'dashboard' | 'osc' | 'pattern' | 'mqtt' | 'settings';
export const VIEWS: View[] = ['dashboard', 'osc', 'pattern', 'mqtt', 'settings'];

export interface UiState {
  view: View;
  console_open: boolean;
  console_height: number;
  piano_source: 'input' | 'output';
  /** Chosen in Settings. Unset until the user picks one: the system language is used. */
  language?: Locale;
  /** Look for a new version at start (at most once a day). */
  check_updates: boolean;
  /** When the last check ran, ms since the Unix epoch; 0 if never. */
  last_update_check: number;
}

/** What the user was about to do when the unsaved-changes dialog asked first. */
export type UnsavedAction =
  | { kind: 'open'; name: string }
  | { kind: 'openOther' }
  /** A file the system asked to open (double-click while the app runs). */
  | { kind: 'pending' }
  | { kind: 'new' }
  | { kind: 'update' };

export type DialogState =
  | { kind: 'quit' }
  | { kind: 'unsaved'; action: UnsavedAction; proceed: () => void | Promise<void> }
  | { kind: 'revert' }
  | { kind: 'about' };

export interface Notice {
  kind: 'error' | 'info';
  /** Called while rendering, so the notice follows a change of language. */
  text: () => string;
  /** An optional button in the notice; running it dismisses the notice. */
  action?: { label: () => string; run: () => void };
}

/** Where the update check stands. */
export type UpdateCheck =
  | { state: 'idle' }
  | { state: 'checking' }
  | { state: 'current' }
  | { state: 'available'; info: UpdateInfo }
  | { state: 'error'; detail: string };

/** A sentence that wraps the raw error detail of a failed command. */
type FailKey =
  | 'errors.engine'
  | 'errors.save'
  | 'errors.revert'
  | 'errors.open'
  | 'errors.create'
  | 'errors.recent'
  | 'errors.update'
  | 'errors.command'
  | 'errors.console'
  | 'errors.clearLog'
  | 'errors.quit'
  | 'errors.openLink'
  | 'errors.updateCheck'
  | 'errors.updateInstall';

export const MAX_LOG_LINES = 2000;
export const CONSOLE_MIN_HEIGHT = 120;
const UPDATE_DEBOUNCE_MS = 400;
const UI_SAVE_DEBOUNCE_MS = 500;
/** MIDI Learn gives up after this long without a message. */
export const LEARN_TIMEOUT_MS = 15_000;
const UPDATE_CHECK_INTERVAL_MS = 24 * 3600_000;

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
  clock_device: { state: 'none', id: '', name: '', detail: '' },
  osc_listener: { state: 'stopped', detail: '', bound: '' },
  osc_target: '',
  mqtt: { state: 'disabled', detail: '', entities: 0, last_command: null, last_command_ts: 0 },
  learning: null,
  learned: null,
  pattern_learning: null,
  pattern_learned: null,
  pattern_mode: 'off',
  seq_recorded: null,
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
  pattern: defaultPattern(),
};

const EMPTY_APP: AppInfo = {
  name: 'VRC MIDI Transposer',
  version: '',
  owner: '',
  email: '',
  license: '',
  repository: '',
  data_dir: '',
  documents_dir: '',
  update_mode: 'none',
};

type KeysListener = (keys: readonly number[], vel: readonly number[], wheels: readonly [number, number]) => void;
type TransportListener = (transport: Readonly<Transport>) => void;

/** The parts of the transport that change rarely (not per beat or step). */
export interface PlayState {
  clock: Transport['clock'];
  playing: boolean;
  armed: boolean;
  rec: Transport['rec'];
}
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
  /** The project file; null while the project was never saved. */
  path = $state<string | null>(null);
  recent = $state.raw<RecentFile[]>([]);
  app = $state.raw<AppInfo>(EMPTY_APP);
  banner = $state('');
  ui = $state<UiState>({
    view: 'dashboard',
    console_open: false,
    console_height: 240,
    piano_source: 'input',
    check_updates: true,
    last_update_check: 0,
  });
  /** Bumped whenever the working copy is replaced, so forms rebuild from it. */
  epoch = $state(0);
  #dialog = $state.raw<DialogState | null>(null);
  /** The open modal. Closing it lets a queued request (a file to open) through. */
  get dialog(): DialogState | null {
    return this.#dialog;
  }
  set dialog(value: DialogState | null) {
    this.#dialog = value;
    if (value === null) this.#retryQueued();
  }
  notice = $state.raw<Notice | null>(null);
  busy = $state(false);
  /** Wall clock, advanced only when a frame arrives. Drives relative times. */
  now = $state(Date.now());

  updateCheck = $state.raw<UpdateCheck>({ state: 'idle' });
  /** Latest `Frame.update`; null until an install starts. */
  updateProgress = $state.raw<UpdateProgress | null>(null);
  /** A newer version is known and not being installed: shown as a dot on the Settings rail item. */
  updateAvailable = $derived(this.updateCheck.state === 'available' && this.updateProgress?.state !== 'installing');

  // Not reactive on purpose: the piano and the console subscribe and patch the DOM.
  logs: LogLine[] = [];
  keys: number[] = [0, 0, 0, 0, 0, 0, 0, 0];
  /** Velocities of the set bits of `keys`, in bit order. */
  vel: number[] = [];
  /** [pitch bend 0..16383, mod wheel 0..127] */
  wheels: [number, number] = [8192, 0];
  /**
   * Clock and sequencer, up to once per display frame while playing. Not
   * reactive: beat dots, the BPM readout and the playhead subscribe with
   * `onTransport` and patch the DOM.
   */
  transport: Transport = { ...IDLE_TRANSPORT };
  /** The low-rate part of `transport`, reactive for the Play button and the clock source line. */
  play = $state.raw<PlayState>({ clock: 'none', playing: false, armed: false, rec: 'off' });
  /** Set when the user opens the console, so its input takes focus once mounted. */
  focusConsole = false;

  #keysListeners = new Set<KeysListener>();
  #transportListeners = new Set<TransportListener>();
  #logListeners = new Set<LogListener>();
  #updateTimer: ReturnType<typeof setTimeout> | undefined;
  #uiTimer: ReturnType<typeof setTimeout> | undefined;
  #learnTimer: ReturnType<typeof setTimeout> | undefined;
  #learnedSeq = -1;
  #editSeq = 0;
  /** The working copy has edits the engine has not received yet. */
  #unsent = false;
  #inFlight: Promise<void> = Promise.resolve();
  #pendingCmds = new Map<string, EngineCmd>();
  #raf = 0;
  /** A file is waiting to be opened until startup, a dialog or a file operation is done. */
  #openQueued = false;
  /** The window's close button was pressed before startup finished. */
  #quitQueued = false;
  #retryTimer: ReturnType<typeof setTimeout> | undefined;
  /** A file taken from the shell at startup while a dialog was open. */
  #heldPath: string | null = null;
  /** The learn target the engine has confirmed; the timeout runs only until it ends. */
  #learnSeen = false;

  // -------------------------------------------------------------------------
  // Startup and frames
  // -------------------------------------------------------------------------

  async start() {
    try {
      const init = await api.subscribe(this.#applyFrame);
      // Before `ready`, so the first paint is already in the right language.
      await this.#restoreLanguage(init.ui.language);
      this.app = init.app;
      this.banner = init.banner;
      this.snapshot = init.snapshot;
      // A learn result from before this window existed is not applied again.
      this.#learnedSeq = init.snapshot.learned?.seq ?? -1;
      this.#patternLearnedSeq = init.snapshot.pattern_learned?.seq ?? -1;
      this.#recordedSeq = init.snapshot.seq_recorded?.seq ?? -1;
      this.project = init.project;
      this.dirty = init.dirty;
      this.path = init.path;
      this.recent = init.recent;
      this.logs = init.logs.slice(-MAX_LOG_LINES);
      this.keys = init.keys;
      this.vel = init.vel ?? [];
      this.wheels = init.wheels ?? [8192, 0];
      if (init.transport) this.#takeTransport(init.transport);
      this.#restoreUi(init.ui);
      this.ready = true;
      this.#emitLogs([], true);
      this.#emitKeys();
    } catch (e) {
      await this.#restoreLanguage(undefined);
      this.fail(e, 'errors.engine');
      this.ready = true;
      return;
    }
    // Requests that arrived while starting were held back, so the Init values
    // above could not overwrite what they did.
    if (this.#quitQueued) {
      this.#quitQueued = false;
      if (this.dirty && this.dialog?.kind !== 'quit') this.dialog = { kind: 'quit' };
    }
    // A file the system asked to open before the window was ready (macOS), or
    // while starting.
    this.#openQueued = false;
    await this.#openPendingAtStart();
    this.#retryQueued();
    void this.#autoCheckUpdate();
  }

  #applyFrame = (frame: Frame) => {
    if (frame.snap) {
      this.snapshot = frame.snap;
      this.#takeLearned(frame.snap);
      this.#takeRecorded(frame.snap);
    }
    if (frame.cleared) this.logs = [];
    if (frame.logs?.length) {
      this.logs.push(...frame.logs);
      if (this.logs.length > MAX_LOG_LINES) this.logs.splice(0, this.logs.length - MAX_LOG_LINES);
    }
    if (frame.cleared || frame.logs?.length) this.#emitLogs(frame.logs ?? [], !!frame.cleared);
    if (frame.keys) {
      this.keys = frame.keys;
      this.vel = frame.vel ?? [];
      if (frame.wheels) this.wheels = frame.wheels;
      this.#emitKeys();
    }
    if (frame.transport) this.#takeTransport(frame.transport);
    if (frame.update) this.updateProgress = frame.update;
    if (frame.close_requested) {
      if (!this.ready) this.#quitQueued = true;
      else if (this.dialog?.kind !== 'quit') this.dialog = { kind: 'quit' };
    }
    if (frame.open_pending) this.openPending();
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

  onTransport(fn: TransportListener): () => void {
    this.#transportListeners.add(fn);
    return () => this.#transportListeners.delete(fn);
  }

  #takeTransport(next: Transport) {
    const prev = this.transport;
    this.transport = next;
    const p = this.play;
    if (p.clock !== next.clock || p.playing !== next.playing || p.armed !== next.armed || p.rec !== next.rec) {
      this.play = { clock: next.clock, playing: next.playing, armed: next.armed, rec: next.rec };
    }
    // The transport arrives with every key frame; listeners only hear about real changes.
    if (
      prev.beat !== next.beat ||
      prev.step !== next.step ||
      prev.bpm !== next.bpm ||
      prev.clock !== next.clock ||
      prev.playing !== next.playing ||
      prev.armed !== next.armed ||
      prev.rec !== next.rec
    ) {
      for (const fn of this.#transportListeners) fn(next);
    }
  }

  /** Sequencer Play (from the next bar) or Stop. Play while armed cancels. */
  seqPlay(on: boolean) {
    return this.engine({ type: 'seq_play', on });
  }

  /** Record: stopped, step by step; playing, one loop live from the next loop start. */
  seqRecord(on: boolean) {
    return this.engine({ type: 'seq_record', on });
  }

  /** Step recording: leave the next step empty. */
  seqRest() {
    return this.engine({ type: 'seq_rest' });
  }

  /** The pattern mode playing now (MIDI may have switched it for the session). */
  get patternMode(): PatternMode {
    return this.snapshot.pattern_mode;
  }

  /** Choose the mode in the UI: a project edit, and the session follows right away. */
  setPatternMode(mode: PatternMode) {
    this.project.pattern.mode = mode;
    this.touch(true);
    void this.engine({ type: 'set_pattern_mode', mode });
  }

  #recordedSeq = -1;

  /** Recorded steps go into the project like an edit in the step grid. */
  #takeRecorded(snap: Snapshot) {
    const r = snap.seq_recorded;
    if (!r || r.seq === this.#recordedSeq) return;
    this.#recordedSeq = r.seq;
    const seq = this.project.pattern.seq;
    r.steps.forEach((step, i) => {
      if (seq.steps[i]) Object.assign(seq.steps[i], step);
    });
    seq.length = r.length;
    this.touch(r.done);
  }

  #emitKeys() {
    for (const fn of this.#keysListeners) fn(this.keys, this.vel, this.wheels);
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
        this.fail(e, 'errors.command');
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
    return api.consoleLine(line).catch((e) => this.fail(e, 'errors.console'));
  }

  clearLog() {
    return api.clearLog().catch((e) => this.fail(e, 'errors.clearLog'));
  }

  // -------------------------------------------------------------------------
  // MIDI Learn
  // -------------------------------------------------------------------------

  /** Index of the custom control waiting for MIDI, or null. */
  get learning(): number | null {
    return this.snapshot.learning;
  }

  /** Arpeggiator / sequencer action waiting for MIDI, or null. */
  get patternLearning(): PatternAction | null {
    return this.snapshot.pattern_learning;
  }

  #patternLearnedSeq = -1;

  /** Waits for the next Note On, CC or Program Change and makes it the trigger of `action`. */
  startPatternLearn(action: PatternAction) {
    clearTimeout(this.#learnTimer);
    this.#learnSeen = false;
    this.#learnTimer = setTimeout(() => this.stopLearn(), LEARN_TIMEOUT_MS);
    void this.engine({ type: 'pattern_learn', action });
  }

  /** Waits for the next Note On, CC or Program Change and makes it the trigger of control `index`. */
  startLearn(index: number) {
    clearTimeout(this.#learnTimer);
    this.#learnSeen = false;
    this.#learnTimer = setTimeout(() => this.stopLearn(), LEARN_TIMEOUT_MS);
    void this.engine({ type: 'midi_learn', index });
  }

  stopLearn() {
    clearTimeout(this.#learnTimer);
    this.#learnTimer = undefined;
    this.#learnSeen = false;
    if (this.snapshot.learning !== null) void this.engine({ type: 'midi_learn', index: null });
    if (this.snapshot.pattern_learning !== null) void this.engine({ type: 'pattern_learn', action: null });
  }

  #takeLearned(snap: Snapshot) {
    // The timeout ends when learning ends, not on a frame that was read
    // before the engine started listening (it still says null).
    if (snap.learning !== null || snap.pattern_learning !== null) {
      this.#learnSeen = true;
    } else if (this.#learnSeen) {
      this.#learnSeen = false;
      clearTimeout(this.#learnTimer);
      this.#learnTimer = undefined;
    }
    const pl = snap.pattern_learned;
    if (pl && pl.seq !== this.#patternLearnedSeq) {
      this.#patternLearnedSeq = pl.seq;
      const old = this.project.pattern.midi[pl.action];
      this.project.pattern.midi[pl.action] = { ...pl.trigger, forward: old?.forward ?? false };
      this.touch(true);
    }
    const learned = snap.learned;
    if (!learned || learned.seq === this.#learnedSeq) return;
    this.#learnedSeq = learned.seq;
    const control = this.project.osc.sending_addresses[learned.index];
    if (!control) return;
    // Only the message is learned. A note keeps its mode, and every trigger
    // keeps whether it is also sent to the output.
    const old = control.midi;
    const trigger = { ...learned.trigger };
    if (old) {
      trigger.forward = old.forward;
      if (trigger.kind === 'note' && old.kind === 'note') trigger.mode = old.mode;
    }
    control.midi = trigger;
    this.touch(true);
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
      .catch((e) => this.fail(e, 'errors.update'));
    return this.#inFlight;
  };

  /**
   * Result of an update or a save: take `dirty`, the file and the recent list,
   * but leave the working copy alone so a field that is being typed in is not
   * overwritten.
   */
  #applyLight(st: ProjectState, seq: number, sent?: Project) {
    this.path = st.path;
    this.recent = st.recent;
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
    this.path = st.path;
    this.recent = st.recent;
    this.epoch++;
  }

  async #run(context: FailKey, fn: () => Promise<void>): Promise<boolean> {
    this.busy = true;
    try {
      await fn();
      return true;
    } catch (e) {
      this.fail(e, context);
      return false;
    } finally {
      this.busy = false;
      this.#retryQueued();
    }
  }

  /** Save to the project file; asks where (Save As) when there is none yet. True when saved. */
  async save(): Promise<boolean> {
    if (this.path === null) return this.saveAs();
    let noPath = false;
    const ok = await this.#run('errors.save', async () => {
      await this.flush();
      const seq = this.#editSeq;
      try {
        this.#applyLight(await api.saveProject(), seq);
      } catch (e) {
        if (errorText(e) !== 'NO_PATH') throw e;
        noPath = true;
      }
    });
    // The file was forgotten underneath (cannot happen today, but the shell may say so).
    if (noPath) return this.saveAs();
    return ok;
  }

  /** Ask for a file and save there. False when cancelled or failed. */
  async saveAs(): Promise<boolean> {
    let saved = false;
    await this.#run('errors.save', async () => {
      await this.flush();
      const seq = this.#editSeq;
      const st = await api.saveProjectAs(t('project.saveDialogTitle'));
      if (!st) return;
      this.#applyLight(st, seq);
      saved = true;
      const path = st.path ?? '';
      this.notice = { kind: 'info', text: () => t('project.saved', { path }) };
    });
    return saved;
  }

  revert(): Promise<boolean> {
    return this.#run('errors.revert', async () => {
      await this.flush();
      this.#applyFull(await api.revertProject());
    });
  }

  /** Runs `proceed` right away, or asks first when there are unsaved changes. */
  guard(action: UnsavedAction, proceed: () => void | Promise<void>) {
    if (this.dirty) this.dialog = { kind: 'unsaved', action, proceed };
    else void proceed();
  }

  #opened(st: ProjectState) {
    this.stopLearn();
    this.#applyFull(st);
    const name = st.project.name || t('project.untitled');
    this.notice = {
      kind: 'info',
      text: () => (st.path === null ? t('project.openedConfig', { name }) : t('project.opened', { name })),
    };
  }

  newProject() {
    if (this.busy) return;
    this.guard({ kind: 'new' }, () =>
      this.#run('errors.create', async () => {
        this.stopLearn();
        this.#dropUnsent();
        await this.#inFlight;
        this.#applyFull(await api.newProject());
        this.notice = null;
      }).then(() => undefined),
    );
  }

  /** The native open dialog (.vrcmt or a v1 config.json). */
  openDialog() {
    if (this.busy) return;
    this.guard({ kind: 'openOther' }, () =>
      this.#run('errors.open', async () => {
        await this.flush();
        const st = await api.openProjectDialog(t('project.openDialogTitle'));
        if (st) this.#opened(st);
      }).then(() => undefined),
    );
  }

  /** Open a file by path, after asking about unsaved changes. */
  openFile(path: string, name = fileName(path)) {
    // Another file operation (a Save As or Open dialog) is still running.
    if (this.busy) return;
    if (path === this.path && !this.dirty) return;
    this.guard({ kind: 'open', name }, () => this.#openPath(path).then(() => undefined));
  }

  #openPath(path: string): Promise<boolean> {
    return this.#run('errors.open', async () => {
      await this.flush();
      this.#opened(await api.openFile(path));
    });
  }

  removeRecent(path: string): Promise<boolean> {
    return api.removeRecent(path).then(
      (st) => {
        this.recent = st.recent;
        return true;
      },
      (e) => {
        this.fail(e, 'errors.recent');
        return false;
      },
    );
  }

  /** The system asked to open a file while the app runs: ask first, then take it. */
  openPending() {
    // Already asking about one; the shell keeps only the latest request anyway.
    if (this.dialog?.kind === 'unsaved' && this.dialog.action.kind === 'pending') return;
    // Wait for startup, for a running file operation and for any other
    // question to be answered, instead of replacing it.
    if (!this.ready || this.busy || this.dialog !== null) {
      this.#openQueued = true;
      return;
    }
    this.#openQueued = false;
    this.guard({ kind: 'pending' }, async () => {
      const held = this.#heldPath;
      this.#heldPath = null;
      // A newer request from the system wins over one held since startup.
      const path = await api.takePendingOpen().then((p) => p ?? held, (e) => {
        this.fail(e, 'errors.open');
        return null;
      });
      if (path) await this.#openPath(path);
    });
  }

  /**
   * Run a queued request once nothing else is going on. Deferred to a task, so
   * the action a closing dialog starts (and its `busy`) comes first.
   */
  #retryQueued() {
    if (!this.#openQueued || !this.ready) return;
    clearTimeout(this.#retryTimer);
    this.#retryTimer = setTimeout(() => {
      if (this.#openQueued && !this.busy && this.dialog === null) this.openPending();
    }, 0);
  }

  async #openPendingAtStart() {
    const path = await api.takePendingOpen().catch(() => null);
    if (!path) return;
    if (this.dialog !== null || this.busy) {
      // The quit question came first; ask about the file once it is answered.
      this.#heldPath = path;
      this.#openQueued = true;
      return;
    }
    this.guard({ kind: 'open', name: fileName(path) }, () => this.#openPath(path).then(() => undefined));
  }

  // -------------------------------------------------------------------------
  // Updates
  // -------------------------------------------------------------------------

  /** Look for a newer version. `quiet`: a failure is kept in `updateCheck` but not shown as an error bar. */
  async checkUpdate(quiet = false): Promise<void> {
    if (this.app.update_mode === 'none' || this.updateCheck.state === 'checking') return;
    this.updateCheck = { state: 'checking' };
    try {
      const info = await api.checkUpdate();
      this.updateCheck = info ? { state: 'available', info } : { state: 'current' };
    } catch (e) {
      const detail = errorText(e);
      this.updateCheck = { state: 'error', detail };
      if (!quiet) this.fail(e, 'errors.updateCheck');
    }
    this.ui.last_update_check = Date.now();
    this.saveUiSoon();
  }

  async #autoCheckUpdate() {
    if (this.app.update_mode === 'none' || !this.ui.check_updates) return;
    if (Date.now() - this.ui.last_update_check < UPDATE_CHECK_INTERVAL_MS) return;
    await this.checkUpdate(true);
    const found = this.updateCheck;
    if (found.state !== 'available' || this.notice) return;
    const version = found.info.version;
    this.notice = {
      kind: 'info',
      text: () => t('settings.noticeAvailable', { version }),
      action: { label: () => t('settings.noticeView'), run: () => this.setView('settings') },
    };
  }

  setCheckUpdates(on: boolean) {
    this.ui.check_updates = on;
    this.saveUiSoon();
  }

  /** Download and install, then the app restarts. Asks about unsaved changes first. */
  installUpdate() {
    const found = this.updateCheck;
    if (found.state !== 'available' || this.app.update_mode !== 'auto') return;
    this.guard({ kind: 'update' }, async () => {
      this.stopLearn();
      await this.flush();
      this.updateProgress = { state: 'downloading', downloaded: 0, total: null };
      try {
        await api.installUpdate();
      } catch (e) {
        // The shell also sends this as a frame; show it in the progress area either way.
        this.updateProgress = { state: 'error', detail: errorText(e) };
      }
    });
  }

  openRelease(version: string) {
    return api.openRelease(version).catch((e) => this.fail(e, 'errors.openLink'));
  }

  // -------------------------------------------------------------------------
  // Quit
  // -------------------------------------------------------------------------

  async saveAndQuit() {
    if (await this.save()) await api.quit(true).catch((e) => this.fail(e, 'errors.quit'));
  }

  discardAndQuit() {
    return api.quit(true).catch((e) => this.fail(e, 'errors.quit'));
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

  /** Switches the UI language and remembers it on this computer (not in the project). */
  async setLanguage(code: Locale) {
    this.ui.language = code;
    this.saveUiSoon();
    await setLocale(code);
  }

  async #restoreLanguage(saved: unknown) {
    if (isLocale(saved)) this.ui.language = saved;
    // setLocale falls back to English (with a console warning) when a
    // language cannot be loaded, so this never keeps the app from starting.
    await setLocale(this.ui.language ?? detectLocale());
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
    if (typeof saved.check_updates === 'boolean') this.ui.check_updates = saved.check_updates;
    const last = saved.last_update_check;
    if (typeof last === 'number' && Number.isFinite(last)) this.ui.last_update_check = last;
  }

  /**
   * Shows an error. The raw detail (English, from the engine or the OS) is
   * put into a translated sentence that says what failed.
   */
  fail(e: unknown, context: FailKey) {
    const detail = errorText(e);
    this.notice = { kind: 'error', text: () => t(context, { detail }) };
  }
}

export const app = new AppStore();
