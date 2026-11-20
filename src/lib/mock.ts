// In-memory stand-in for the Rust shell, used when the UI runs in a plain
// browser (`bun run dev:web`). Implements every command in ipc.ts closely
// enough to develop and review the UI. Never loaded inside Tauri.
//
// Query parameters for reviewing a state without clicking:
//   ?view=osc|mqtt|settings   start on a view
//   ?console=1&h=260          console drawer open, with a height
//   ?close=1                  simulate closing the window with unsaved changes
//   ?demo=0                   do not play the piano demo

import { clamp, defaultListenAddresses, slug } from './format';
import type {
  AppInfo,
  EngineCmd,
  Frame,
  Init,
  LogLevel,
  LogLine,
  MidiStatus,
  PortInfo,
  PortRef,
  Project,
  ProjectInfo,
  ProjectState,
  Snapshot,
} from './ipc';

const BANNER = String.raw` _  _  ____   ___
╱ )( ╲(  _ ╲ ╱ __)     ▓▓╦▓▓▓╦▓▓▓╦▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓
╲ ╲╱ ╱ )   ╱( (__      ▒▒╬▒▒▒╬▒▒▒╬▒▒▒▒o▒▒o▒▒o▒▒o▒▒o▒▒o▒▒o▒
 ╲__╱ (__╲_) ╲___)     ░░╩░░░╩░░░╩░░░░░░░░░░░░░░░░░░░░░░░░
 _  _  __  ____  __    ║░█░█░║░█░█░█░║░█░█░║░█░█░█░║░█░█░║
( ╲╱ )(  )(    ╲(  )   ║░█░█░║░█░█░█░║░█░█░║░█░█░█░║░█░█░║
╱ ╲╱ ╲ )(  ) D ( )(    ║░║░║░║░║░║░║░║░║░║░║░║░║░║░║░║░║░║
╲_)(_╱(__)(____╱(__)   ╚═╩═╩═╩═╩═╩═╩═╩═╩═╩═╩═╩═╩═╩═╩═╩═╩═╝
 ____  ____   __   __ _  ____  ____   __   ____  ____  ____
(_  _)(  _ ╲ ╱ _╲ (  ( ╲╱ ___)(  _ ╲ ╱  ╲ ╱ ___)(  __)(  _ ╲
  )(   )   ╱╱    ╲╱    ╱╲___ ╲ ) __╱(  O )╲___ ╲ ) _)  )   ╱
 (__) (__╲_)╲_╱╲_╱╲_)__)(____╱(__)   ╲__╱ (____╱(____)(__╲_)`;

const APP: AppInfo = {
  name: 'VRC MIDI Transposer',
  version: '2.0.0',
  owner: 'Marcus Universe',
  email: 'contact@marcus-universe.de',
  license: 'MIT License',
  repository: 'https://github.com/marcus-universe/vrc_midi_transposer',
  data_dir: 'C:\\Users\\you\\AppData\\Roaming\\de.marcus-universe.vrc-midi-transposer',
};

const INPUTS: PortInfo[] = [
  { id: 'in-0', name: 'Arturia KeyLab 61' },
  { id: 'in-1', name: 'MRCC' },
  { id: 'in-2', name: 'loopMIDI Port' },
];

const OUTPUTS: PortInfo[] = [
  { id: 'out-0', name: 'loopMIDI Port' },
  { id: 'out-1', name: 'Microsoft GS Wavetable Synth' },
  { id: 'out-2', name: 'MRCC' },
];

const UI_KEY = 'vrc-midi-transposer.mock-ui';
const params = new URLSearchParams(typeof location === 'undefined' ? '' : location.search);

function clone<T>(v: T): T {
  return JSON.parse(JSON.stringify(v)) as T;
}

function makeProject(id: string, name: string): Project {
  return {
    schema: 1,
    id,
    name,
    midi: { input: null, output: null },
    osc: {
      listen_host: '127.0.0.1',
      listen_port: 9069,
      target_host: '127.0.0.1',
      target_port: 9000,
      sending_enabled: false,
      send_original: true,
      listening_addresses: defaultListenAddresses(),
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
}

function demoProject(): Project {
  const p = makeProject('p-stage', 'Stage piano');
  p.midi.input = { id: 'in-1', name: 'MRCC' };
  p.midi.output = { id: 'out-0', name: 'loopMIDI Port' };
  p.osc.sending_enabled = true;
  p.osc.sending_addresses = [
    { name: 'Sunglasses Toggle', addr: '/avatar/parameters/Sunglasses', type: 'bool', default: 0, min: null, max: null },
    { name: 'Sign Toggle', addr: '/avatar/parameters/Sign', type: 'bool', default: 1, min: null, max: null },
    { name: 'Hue Shift', addr: '/avatar/parameters/HUE', type: 'float', default: 0.35, min: 0, max: 1 },
  ];
  p.mqtt.enabled = true;
  p.mqtt.broker_host = '192.168.1.20';
  p.mqtt.username = 'transposer';
  p.mqtt.password_set = true;
  return p;
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/** Saved files, by project id. */
const saved = new Map<string, { project: Project; modified: number }>();
let working: Project;
const secrets = new Map<string, string>();

const session = {
  transpose: 0,
  debug: false,
  osc_sending: false,
  send_original: true,
  mqtt_enabled: false,
  mqtt_connected: false,
  custom_values: [] as number[],
  last_command: '',
  last_command_ts: 0,
};

let logs: LogLine[] = [];
let logId = 0;
const inputNotes = new Set<number>();

let listener: ((frame: Frame) => void) | null = null;
let visible = true;
let pending: Frame | null = null;
let frameTimer: ReturnType<typeof setTimeout> | undefined;
let demoTimer: ReturnType<typeof setInterval> | undefined;
let mqttTimer: ReturnType<typeof setTimeout> | undefined;
let nextId = 1;

function seed() {
  const stage = demoProject();
  const practice = makeProject('p-practice', 'Practice room');
  practice.midi.input = { id: 'in-0', name: 'Arturia KeyLab 61' };
  practice.midi.output = { id: 'out-1', name: 'Microsoft GS Wavetable Synth' };
  const now = Date.now();
  saved.set(stage.id, { project: clone(stage), modified: now - 3 * 3600_000 });
  saved.set(practice.id, { project: clone(practice), modified: now - 26 * 3600_000 });
  secrets.set(stage.id, 'hunter2');
  working = clone(stage);
  applyOpenValues();
  session.transpose = 3;
  session.mqtt_connected = true;
  session.last_command = 'transpose = 3';
  session.last_command_ts = now - 42_000;

  log('accent', `${APP.name} ${APP.version}`);
  log('info', `Project "${working.name}" loaded`);
  log('success', 'MIDI input connected: MRCC');
  log('success', 'MIDI output connected: loopMIDI Port');
  log('info', 'OSC listener on 127.0.0.1:9069, sending to 127.0.0.1:9000');
  log('success', 'MQTT connected to 192.168.1.20:1883, 9 Home Assistant entities published');
  log('info', 'MQTT: transpose = 3');
  log('warn', 'OSC: /transposeUp received 0.5, ignored (expects 1 or true)');
  log('info', 'Type "help" for a list of commands.');
}

/** `osc.sending_enabled`, `osc.send_original` and `debug` apply when a project is opened. */
function applyOpenValues() {
  session.osc_sending = working.osc.sending_enabled;
  session.send_original = working.osc.send_original;
  session.debug = working.debug;
  session.mqtt_enabled = working.mqtt.enabled;
  session.mqtt_connected = working.mqtt.enabled;
  session.transpose = clamp(session.transpose, working.transpose.min, working.transpose.max);
  session.custom_values = working.osc.sending_addresses.map((c) => c.default);
}

// ---------------------------------------------------------------------------
// Frames
// ---------------------------------------------------------------------------

function queue(part: Frame) {
  pending = { ...(pending ?? {}), ...part, logs: [...(pending?.logs ?? []), ...(part.logs ?? [])] };
  if (frameTimer !== undefined || !visible) return;
  frameTimer = setTimeout(deliver, 16);
}

function deliver() {
  frameTimer = undefined;
  if (!pending || !listener || !visible) return;
  const frame = pending;
  pending = null;
  if (!frame.logs?.length) delete frame.logs;
  listener(frame);
}

function log(level: LogLevel, text: string) {
  const line: LogLine = { id: ++logId, ts: Date.now(), level, text };
  logs.push(line);
  if (logs.length > 2000) logs = logs.slice(-2000);
  queue({ logs: [line] });
}

function pushSnapshot() {
  queue({ snap: snapshot() });
}

function pushKeys() {
  queue({ keys: keyWords() });
}

function keyWords(): number[] {
  const words = [0, 0, 0, 0, 0, 0, 0, 0];
  for (const n of inputNotes) {
    words[n >> 5] = (words[n >> 5] | (1 << (n & 31))) >>> 0;
    const out = clamp(n + session.transpose, 0, 127);
    words[4 + (out >> 5)] = (words[4 + (out >> 5)] | (1 << (out & 31))) >>> 0;
  }
  return words;
}

/** Same order as the engine: id, then exact name, then name as a substring. */
function resolvePort(ref: PortRef | null, ports: PortInfo[]): PortInfo | undefined {
  if (!ref) return undefined;
  return (
    ports.find((p) => ref.id !== '' && p.id === ref.id) ??
    ports.find((p) => p.name === ref.name) ??
    (ref.name ? ports.find((p) => p.name.includes(ref.name)) : undefined)
  );
}

function midiStatus(ref: PortRef | null, ports: PortInfo[]): MidiStatus {
  if (!ref) return { state: 'none', id: '', name: '', detail: '' };
  const port = resolvePort(ref, ports);
  if (!port) return { state: 'missing', id: '', name: ref.name, detail: '' };
  return { state: 'connected', id: port.id, name: port.name, detail: '' };
}

function snapshot(): Snapshot {
  const o = working.osc;
  const m = working.mqtt;
  const portClash = o.listen_port === o.target_port && o.listen_host === o.target_host;
  const brokerBad = !m.broker_host.trim() || m.broker_host.includes('bad');
  const mqttState: Snapshot['mqtt']['state'] = !session.mqtt_enabled
    ? 'disabled'
    : brokerBad
      ? 'error'
      : session.mqtt_connected
        ? 'connected'
        : 'connecting';
  return {
    transpose: session.transpose,
    transpose_min: working.transpose.min,
    transpose_max: working.transpose.max,
    debug: session.debug,
    osc_sending: session.osc_sending,
    send_original: session.send_original,
    mqtt_enabled: session.mqtt_enabled,
    custom_values: [...session.custom_values],
    inputs: clone(INPUTS),
    outputs: clone(OUTPUTS),
    midi_in: midiStatus(working.midi.input, INPUTS),
    midi_out: midiStatus(working.midi.output, OUTPUTS),
    osc_listener: portClash
      ? {
          state: 'error',
          detail: `Port ${o.listen_port} is already in use. Close the other program or choose another port.`,
          bound: '',
        }
      : { state: 'running', detail: '', bound: `${o.listen_host}:${o.listen_port}` },
    osc_target: `${o.target_host}:${o.target_port}`,
    mqtt: {
      state: mqttState,
      detail: mqttState === 'error' ? `Could not reach ${m.broker_host || 'the broker'}:${m.broker_port}. Connection refused.` : '',
      entities: mqttState === 'connected' ? 6 + working.osc.sending_addresses.length : 0,
      last_command: session.last_command,
      last_command_ts: session.last_command_ts,
    },
  };
}

// ---------------------------------------------------------------------------
// Projects
// ---------------------------------------------------------------------------

function comparable(p: Project): string {
  return JSON.stringify({ ...p, mqtt: { ...p.mqtt, password: '' } });
}

function projectList(): ProjectInfo[] {
  return [...saved.values()]
    .map(({ project, modified }) => ({ id: project.id, name: project.name, modified }))
    .sort((a, b) => a.name.localeCompare(b.name));
}

function state(): ProjectState {
  const file = saved.get(working.id);
  const out = clone(working);
  out.mqtt.password = '';
  return {
    project: out,
    dirty: !file || comparable(file.project) !== comparable(working),
    projects: projectList(),
  };
}

function open(project: Project) {
  working = clone(project);
  applyOpenValues();
  connectMqttSoon();
  log('info', `Project "${working.name}" loaded`);
  pushSnapshot();
  pushKeys();
}

function connectMqttSoon() {
  clearTimeout(mqttTimer);
  if (!session.mqtt_enabled) return;
  session.mqtt_connected = false;
  mqttTimer = setTimeout(() => {
    session.mqtt_connected = true;
    pushSnapshot();
  }, 700);
}

function updateProject(next: Project): ProjectState {
  const before = working;
  const incoming = clone(next);
  if (incoming.mqtt.password !== '') {
    secrets.set(incoming.id, incoming.mqtt.password);
    incoming.mqtt.password_set = true;
  } else if (!incoming.mqtt.password_set) {
    secrets.delete(incoming.id);
  }
  incoming.mqtt.password = '';
  working = incoming;

  session.transpose = clamp(session.transpose, working.transpose.min, working.transpose.max);
  const controls = working.osc.sending_addresses;
  session.custom_values = controls.map((c, i) => {
    const v = session.custom_values[i] ?? c.default;
    return c.type === 'bool' ? (v ? 1 : 0) : clamp(v, c.min ?? -Infinity, c.max ?? Infinity);
  });
  if (before.mqtt.enabled !== working.mqtt.enabled) session.mqtt_enabled = working.mqtt.enabled;
  const brokerChanged =
    before.mqtt.broker_host !== working.mqtt.broker_host ||
    before.mqtt.broker_port !== working.mqtt.broker_port ||
    before.mqtt.enabled !== working.mqtt.enabled;
  if (brokerChanged) connectMqttSoon();
  pushSnapshot();
  pushKeys();
  return state();
}

// ---------------------------------------------------------------------------
// Engine and console commands
// ---------------------------------------------------------------------------

function setTranspose(value: number, source = '') {
  const v = clamp(Math.round(value), working.transpose.min, working.transpose.max);
  if (v === session.transpose) return;
  session.transpose = v;
  log('info', `${source}Transpose set to ${v}`);
  pushKeys();
}

function engine(cmd: EngineCmd) {
  switch (cmd.type) {
    case 'set_transpose':
      setTranspose(cmd.value);
      break;
    case 'nudge':
      setTranspose(session.transpose + cmd.delta);
      break;
    case 'set_osc_sending':
      session.osc_sending = cmd.on;
      log('info', `OSC sending ${cmd.on ? 'enabled' : 'disabled'}`);
      break;
    case 'set_send_original':
      session.send_original = cmd.on;
      log('info', `OSC sends ${cmd.on ? 'original' : 'transposed'} notes`);
      break;
    case 'set_debug':
      session.debug = cmd.on;
      log('info', `Debug logging ${cmd.on ? 'on' : 'off'}`);
      break;
    case 'custom_osc': {
      const control = working.osc.sending_addresses[cmd.index];
      if (!control) break;
      const v =
        control.type === 'bool' ? (cmd.value ? 1 : 0) : clamp(cmd.value, control.min ?? -Infinity, control.max ?? Infinity);
      session.custom_values[cmd.index] = v;
      if (session.debug) log('debug', `OSC out ${control.addr} ${control.type === 'bool' ? v : v.toFixed(2)}`);
      break;
    }
    case 'refresh_ports':
      log('info', `MIDI ports refreshed: ${INPUTS.length} inputs, ${OUTPUTS.length} outputs`);
      break;
    case 'panic':
      inputNotes.clear();
      log('warn', 'Panic: all notes released');
      pushKeys();
      break;
  }
  pushSnapshot();
}

function listMidi() {
  log('accent', 'MIDI inputs');
  INPUTS.forEach((p, i) => log('info', `  ${i}: ${p.name}`));
  log('accent', 'MIDI outputs');
  OUTPUTS.forEach((p, i) => log('info', `  ${i}: ${p.name}`));
}

const HELP = [
  '<number>              set transpose',
  'osc on | off          OSC sending',
  'osc original          send original (input) notes',
  'osc transposed        send transposed (output) notes',
  'midi list             list MIDI devices',
  'mqtt on | off         MQTT client for this session',
  'debug on | off        verbose logging',
  'help                  this list',
  'exit                  quit',
];

function consoleLine(raw: string) {
  const line = raw.trim();
  if (!line) return;
  log('accent', `> ${line}`);
  const cmd = line.toLowerCase().replace(/\s+/g, ' ');
  const on = (word: string) => ['1', 'on', 'enable', 'true'].includes(word);
  const off = (word: string) => ['0', 'off', 'disable', 'false'].includes(word);

  if (/^[+-]?\d+$/.test(cmd)) {
    const before = session.transpose;
    setTranspose(Number(cmd));
    if (before === session.transpose) log('info', `Transpose is ${session.transpose}`);
  } else if (cmd === 'osc on' || cmd === 'osc enable') engine({ type: 'set_osc_sending', on: true });
  else if (cmd === 'osc off' || cmd === 'osc disable') engine({ type: 'set_osc_sending', on: false });
  else if (['osc original', 'osc input', 'osc_original'].includes(cmd)) engine({ type: 'set_send_original', on: true });
  else if (['osc transposed', 'osc output', 'osc_transposed'].includes(cmd)) engine({ type: 'set_send_original', on: false });
  else if (/^osc_original[ :]\w+$/.test(cmd)) {
    const word = cmd.split(/[ :]/)[1];
    if (on(word)) engine({ type: 'set_send_original', on: true });
    else if (off(word)) engine({ type: 'set_send_original', on: false });
    else log('error', `Unknown value "${word}". Use on or off.`);
  } else if (['midi list', 'midi devices', 'list midi'].includes(cmd)) listMidi();
  else if (cmd === 'mqtt on' || cmd === 'mqtt enable') {
    session.mqtt_enabled = true;
    connectMqttSoon();
    log('info', 'MQTT client started for this session');
  } else if (cmd === 'mqtt off' || cmd === 'mqtt disable') {
    session.mqtt_enabled = false;
    log('info', 'MQTT client stopped for this session');
  } else if (cmd === 'debug on' || cmd === 'debug enable') engine({ type: 'set_debug', on: true });
  else if (cmd === 'debug off' || cmd === 'debug disable') engine({ type: 'set_debug', on: false });
  else if (cmd === 'help' || cmd === 'h') HELP.forEach((l) => log('info', l));
  else if (['exit', 'quit', 'q'].includes(cmd)) quit(false);
  else log('error', `Unknown command "${line}". Type "help" for a list of commands.`);
  pushSnapshot();
}

function quit(force: boolean) {
  if (!force && state().dirty) {
    queue({ close_requested: true });
    return;
  }
  log('warn', 'Quit requested. In the app the window would close now (browser mock).');
}

// ---------------------------------------------------------------------------
// Piano demo
// ---------------------------------------------------------------------------

const NOTE_NAMES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];
const noteName = (n: number) => `${NOTE_NAMES[n % 12]}${Math.floor(n / 12) - 1}`;

/** A slow progression: each step is the set of held notes. */
const DEMO_STEPS: number[][] = [
  [48, 60, 64, 67],
  [48, 60, 64, 67, 72],
  [45, 57, 60, 64],
  [45, 57, 60, 64, 69],
  [41, 53, 57, 60],
  [41, 53, 57, 60, 65],
  [43, 55, 59, 62],
  [43, 55, 59, 62, 67, 74],
  [],
];
let demoStep = 0;

function demoTick() {
  if (!visible) return;
  const next = new Set(DEMO_STEPS[demoStep % DEMO_STEPS.length]);
  demoStep++;
  for (const n of inputNotes) {
    if (!next.has(n)) {
      inputNotes.delete(n);
      if (session.debug) log('debug', `Note Off ${noteName(n)} (${n})`);
    }
  }
  for (const n of next) {
    if (!inputNotes.has(n)) {
      inputNotes.add(n);
      if (session.debug) {
        const out = clamp(n + session.transpose, 0, 127);
        log('debug', `Note On  ${noteName(n)} (${n}) -> ${noteName(out)} (${out})`);
      }
    }
  }
  pushKeys();
}

// ---------------------------------------------------------------------------
// Entry points used by ipc.ts
// ---------------------------------------------------------------------------

function loadUi(): Record<string, unknown> {
  let ui: Record<string, unknown> = {};
  try {
    ui = JSON.parse(localStorage.getItem(UI_KEY) ?? '{}') as Record<string, unknown>;
  } catch {
    ui = {};
  }
  if (params.has('view')) ui.view = params.get('view');
  if (params.has('console')) ui.console_open = params.get('console') === '1';
  if (params.has('h')) ui.console_height = Number(params.get('h'));
  return ui;
}

export async function mockSubscribe(onFrame: (frame: Frame) => void): Promise<Init> {
  if (!saved.size) seed();
  listener = onFrame;
  pending = null;

  clearInterval(demoTimer);
  if (params.get('demo') !== '0') {
    for (const n of DEMO_STEPS[1]) inputNotes.add(n);
    demoStep = 2;
    demoTimer = setInterval(demoTick, 900);
  }
  if (params.get('close') === '1') {
    working.osc.target_port = 9001;
    setTimeout(() => queue({ snap: snapshot(), close_requested: true }), 150);
  }

  return {
    ...state(),
    app: APP,
    banner: BANNER,
    snapshot: snapshot(),
    ui: loadUi(),
    logs: [...logs],
    keys: keyWords(),
  };
}

export async function mockInvoke(cmd: string, args: Record<string, unknown> = {}): Promise<unknown> {
  switch (cmd) {
    case 'set_visible':
      visible = Boolean(args.visible);
      if (visible) queue({ snap: snapshot(), keys: keyWords() });
      return;
    case 'engine_cmd':
      engine(args.cmd as EngineCmd);
      return;
    case 'console_line':
      consoleLine(String(args.line ?? ''));
      return;
    case 'clear_log':
      logs = [];
      pending = { ...(pending ?? {}), logs: [] };
      queue({ cleared: true });
      return;

    case 'update_project':
      return updateProject(args.project as Project);
    case 'save_project':
      saved.set(working.id, { project: clone(working), modified: Date.now() });
      log('success', `Project "${working.name}" saved`);
      return state();
    case 'revert_project': {
      const file = saved.get(working.id);
      if (file) open(file.project);
      return state();
    }
    case 'open_project': {
      const file = saved.get(String(args.id));
      if (!file) throw `Project ${String(args.id)} was not found. It may have been deleted.`;
      open(file.project);
      return state();
    }
    case 'new_project': {
      const p = makeProject(`p-${nextId++}-${Date.now().toString(36)}`, String(args.name));
      saved.set(p.id, { project: clone(p), modified: Date.now() });
      open(p);
      return state();
    }
    case 'duplicate_project': {
      const p = clone(working);
      const secret = secrets.get(working.id);
      p.id = `p-${nextId++}-${Date.now().toString(36)}`;
      p.name = String(args.name);
      if (secret) secrets.set(p.id, secret);
      saved.set(p.id, { project: clone(p), modified: Date.now() });
      open(p);
      return state();
    }
    case 'delete_project': {
      const id = String(args.id);
      const name = saved.get(id)?.project.name ?? id;
      saved.delete(id);
      secrets.delete(id);
      log('warn', `Project "${name}" deleted`);
      if (working.id === id) {
        if (!saved.size) {
          const p = makeProject(`p-${nextId++}-${Date.now().toString(36)}`, 'Default');
          saved.set(p.id, { project: clone(p), modified: Date.now() });
        }
        open(projectList().map((info) => saved.get(info.id)!.project)[0]);
      }
      return state();
    }
    case 'import_project': {
      // What importing a v1 config.json produces: ports are name patterns without an id.
      const p = makeProject(`p-${nextId++}-${Date.now().toString(36)}`, 'Imported config');
      p.midi.input = { id: '', name: 'MRCC' };
      p.midi.output = { id: '', name: 'loopMIDI' };
      p.osc.sending_addresses = [
        { name: 'Hue Shift', addr: '/avatar/parameters/HUE', type: 'float', default: 0, min: 0, max: 1 },
      ];
      saved.set(p.id, { project: clone(p), modified: Date.now() });
      open(p);
      log('success', 'Imported config.json (browser mock: no file dialog)');
      return state();
    }
    case 'export_project': {
      const file = `${slug(working.name) || 'project'}.json`;
      return `C:\\Users\\you\\Documents\\${file}`;
    }

    case 'save_ui':
      try {
        localStorage.setItem(UI_KEY, JSON.stringify(args.ui ?? {}));
      } catch {
        // Private windows and file:// pages may refuse storage; the mock can live without it.
      }
      return;
    case 'open_repository':
      window.open(APP.repository, '_blank', 'noopener');
      return;
    case 'quit':
      quit(Boolean(args.force));
      return;
    default:
      throw `Unknown command "${cmd}" (browser mock)`;
  }
}
