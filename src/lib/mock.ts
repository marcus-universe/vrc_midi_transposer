// In-memory stand-in for the Rust shell, used when the UI runs in a plain
// browser (`bun run dev:web`). Implements every command in ipc.ts closely
// enough to develop and review the UI. Never loaded inside Tauri.
//
// Query parameters for reviewing a state without clicking:
//   ?view=osc|mqtt|settings   start on a view
//   ?console=1&h=260          console drawer open, with a height
//   ?close=1                  simulate closing the window with unsaved changes
//   ?demo=0                   do not play the piano demo
//   ?update=none|notify|error update mode (default auto, offering 2.1.1); `error` fails the download
//   ?dialog=cancel            the native Save As and Open dialogs are cancelled
//   ?pending=1                the system asks to open a file 2 s after start (double-click on a .vrcmt)
//   ?fresh=1                  start with a new, never saved project
//
// From the browser console: `mockOpenFile('C:\\path\\file.vrcmt')` simulates a
// double-click on a project file while the app runs.

import { clamp, defaultListenAddresses, fileName } from './format';
import type {
  AppInfo,
  EngineCmd,
  Frame,
  Init,
  LastCommand,
  Learned,
  LogLevel,
  LogLine,
  MidiStatus,
  MidiTrigger,
  PortInfo,
  PortRef,
  Project,
  ProjectState,
  RecentFile,
  Snapshot,
  UpdateInfo,
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

const params = new URLSearchParams(typeof location === 'undefined' ? '' : location.search);

const UPDATE_PARAM = params.get('update');

const APP: AppInfo = {
  name: 'VRC MIDI Transposer',
  version: '2.1.0',
  owner: 'Marcus Universe',
  email: 'contact@marcus-universe.de',
  license: 'MIT License',
  repository: 'https://github.com/marcus-universe/vrc_midi_transposer',
  data_dir: 'C:\\Users\\you\\AppData\\Roaming\\de.marcus-universe.vrc-midi-transposer',
  documents_dir: 'C:\\Users\\you\\Documents\\VRC MIDI Transposer',
  update_mode: UPDATE_PARAM === 'none' ? 'none' : UPDATE_PARAM === 'notify' ? 'notify' : 'auto',
};

const UPDATE: UpdateInfo = {
  version: '2.1.1',
  current: APP.version,
  date: '2026-11-18T09:30:00Z',
  notes:
    'Fixes\n- MIDI Learn ignores Note Off messages.\n- The recent list no longer shows a file twice.\n\nImprovements\n- Faster start with many recent projects.\n<b>Release notes are plain text</b>',
};

const DOCS = APP.documents_dir;

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
    {
      name: 'Sunglasses Toggle',
      addr: '/avatar/parameters/Sunglasses',
      type: 'bool',
      default: 0,
      min: null,
      max: null,
      midi: { kind: 'note', channel: null, number: 36, mode: 'toggle', forward: false },
    },
    { name: 'Sign Toggle', addr: '/avatar/parameters/Sign', type: 'bool', default: 1, min: null, max: null },
    {
      name: 'Hue Shift',
      addr: '/avatar/parameters/HUE',
      type: 'float',
      default: 0.35,
      min: 0,
      max: 1,
      midi: { kind: 'cc', channel: 1, number: 20, mode: 'toggle', forward: false },
    },
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

/** Project files on the pretend disk, by path. */
const files = new Map<string, Project>();
/** Recently used files, newest first. Some may be missing from `files`. */
let recentPaths: string[] = [];
let working: Project;
/** The last saved version of the working copy (the default project while it has no file). */
let baseline: Project;
let workingPath: string | null = null;
/** The MQTT password lives on the computer, never in a file. */
let secret = '';
/** A file the system asked to open that the UI has not taken yet. */
let pendingOpen: string | null = null;
let seeded = false;

const session = {
  transpose: 0,
  debug: false,
  osc_sending: false,
  send_original: true,
  mqtt_enabled: false,
  mqtt_connected: false,
  custom_values: [] as number[],
  last_command: null as LastCommand | null,
  last_command_ts: 0,
  learning: null as number | null,
  learned: null as Learned | null,
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
let learnTimer: ReturnType<typeof setTimeout> | undefined;
let nextId = 1;

const STAGE_PATH = `${DOCS}\\Stage piano.vrcmt`;
const PRACTICE_PATH = 'C:\\Users\\you\\Music\\VRChat sets\\2026\\Practice room.vrcmt';
const MISSING_PATH = 'D:\\Backup\\Old laptop\\Live set.vrcmt';

function seed() {
  seeded = true;
  const stage = demoProject();
  const practice = makeProject('p-practice', 'Practice room');
  practice.midi.input = { id: 'in-0', name: 'Arturia KeyLab 61' };
  practice.midi.output = { id: 'out-1', name: 'Microsoft GS Wavetable Synth' };
  files.set(STAGE_PATH, clone(stage));
  files.set(PRACTICE_PATH, clone(practice));
  recentPaths = [STAGE_PATH, PRACTICE_PATH, MISSING_PATH];
  secret = 'hunter2';
  const now = Date.now();
  if (params.get('fresh') === '1') {
    working = makeProject(`p-${nextId++}`, 'Untitled');
    baseline = clone(working);
    workingPath = null;
  } else {
    working = clone(stage);
    baseline = clone(stage);
    workingPath = STAGE_PATH;
  }
  applyOpenValues();
  session.transpose = 3;
  session.mqtt_connected = true;
  session.last_command = { kind: 'transpose', value: 3 };
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
  queue(visual());
}

/** Velocity per held input note (the demo plays with varying strength). */
const noteVel = new Map<number, number>();
/** [pitch bend 0..16383, mod wheel 0..127] */
let wheels: [number, number] = [8192, 0];

function keyWords(): number[] {
  const words = [0, 0, 0, 0, 0, 0, 0, 0];
  for (const n of inputNotes) {
    words[n >> 5] = (words[n >> 5] | (1 << (n & 31))) >>> 0;
    const out = clamp(n + session.transpose, 0, 127);
    words[4 + (out >> 5)] = (words[4 + (out >> 5)] | (1 << (out & 31))) >>> 0;
  }
  return words;
}

/** Same shape as the engine: keys, velocities in bit order, wheels. */
function visual(): { keys: number[]; vel: number[]; wheels: [number, number] } {
  const keys = keyWords();
  const velOf = new Map<number, number>();
  for (const n of inputNotes) {
    const v = noteVel.get(n) ?? 100;
    velOf.set(n, v);
    const out = clamp(n + session.transpose, 0, 127);
    velOf.set(128 + out, Math.max(v, velOf.get(128 + out) ?? 0));
  }
  const vel: number[] = [];
  for (let w = 0; w < 8; w++) {
    for (let bit = 0; bit < 32; bit++) {
      if ((keys[w] >>> bit) & 1) vel.push(velOf.get((w >> 2) * 128 + (w & 3) * 32 + bit) ?? 100);
    }
  }
  return { keys, vel, wheels: [...wheels] as [number, number] };
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
    learning: session.learning,
    learned: session.learned ? clone(session.learned) : null,
  };
}

// ---------------------------------------------------------------------------
// Projects
// ---------------------------------------------------------------------------

/** Like the shell: ids do not count, a missing trigger and a null one are the same. */
function comparable(p: Project): string {
  return JSON.stringify({
    ...p,
    id: '',
    mqtt: { ...p.mqtt, password: '' },
    osc: { ...p.osc, sending_addresses: p.osc.sending_addresses.map((c) => ({ ...c, midi: c.midi ?? null })) },
  });
}

function recentList(): RecentFile[] {
  return recentPaths.map((path) => {
    const file = files.get(path);
    return { path, name: file?.name ?? fileName(path).replace(/\.vrcmt$/i, ''), exists: !!file };
  });
}

function remember(path: string) {
  recentPaths = [path, ...recentPaths.filter((p) => p !== path)].slice(0, 10);
}

function state(): ProjectState {
  const out = clone(working);
  out.mqtt.password = '';
  return {
    project: out,
    dirty: comparable(baseline) !== comparable(working),
    path: workingPath,
    recent: recentList(),
  };
}

function writeFile(path: string) {
  files.set(path, clone(working));
  baseline = clone(working);
  workingPath = path;
  remember(path);
  log('success', `Project saved to ${path}`);
}

function open(project: Project, path: string | null) {
  working = clone(project);
  // A file opens clean; an imported config.json or a new project starts from the default.
  baseline = path ? clone(project) : makeProject(project.id, 'Untitled');
  workingPath = path;
  if (path) remember(path);
  session.learning = null;
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
    secret = incoming.mqtt.password;
    incoming.mqtt.password_set = true;
  } else if (!incoming.mqtt.password_set) {
    secret = '';
  }
  incoming.mqtt.password = '';
  // The shell skips a missing trigger and accepts null; store both as absent.
  for (const c of incoming.osc.sending_addresses) if (c.midi == null) delete c.midi;
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
      wheels = [8192, 0];
      log('warn', 'Panic: all notes released');
      pushKeys();
      break;
    case 'midi_learn':
      clearTimeout(learnTimer);
      if (cmd.index === null || !working.osc.sending_addresses[cmd.index]) {
        if (session.learning !== null) log('info', 'MIDI Learn cancelled');
        session.learning = null;
        break;
      }
      session.learning = cmd.index;
      log('info', `MIDI Learn: waiting for a message for "${working.osc.sending_addresses[cmd.index].name}"`);
      // Pretend the user touches the keyboard after a moment.
      learnTimer = setTimeout(() => {
        const index = session.learning;
        if (index === null) return;
        const trigger = learnedTrigger(index);
        session.learning = null;
        session.learned = { seq: (session.learned?.seq ?? 0) + 1, index, trigger };
        log('success', `MIDI Learn: ${trigger.kind} ${trigger.number} on channel ${trigger.channel ?? 'any'}`);
        pushSnapshot();
      }, 1500);
      break;
  }
  pushSnapshot();
}

/** What "the user played" during MIDI Learn: a different message per control. */
function learnedTrigger(index: number): MidiTrigger {
  switch (index % 3) {
    case 0:
      return { kind: 'note', channel: 1, number: 48 + index, mode: 'toggle', forward: false };
    case 1:
      return { kind: 'cc', channel: 2, number: 21 + index, mode: 'toggle', forward: false };
    default:
      return { kind: 'program', channel: 1, number: 4 + index, mode: 'toggle', forward: false };
  }
}

/** Now and then a control with a trigger reacts, as if its key or knob was used. */
function triggerTick() {
  const controls = working.osc.sending_addresses;
  const withTrigger = controls.map((c, i) => [c, i] as const).filter(([c]) => c.midi);
  if (!withTrigger.length) return;
  const [c, i] = withTrigger[Math.floor(Math.random() * withTrigger.length)];
  if (c.type === 'bool') session.custom_values[i] = session.custom_values[i] >= 0.5 ? 0 : 1;
  else {
    const min = c.min ?? 0;
    const max = c.max ?? 1;
    session.custom_values[i] = Number((min + Math.random() * (max - min)).toFixed(2));
  }
  if (session.debug) log('debug', `MIDI trigger ${c.name}: ${session.custom_values[i]}`);
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
  if (demoStep % 4 === 0) triggerTick();
  for (const n of inputNotes) {
    if (!next.has(n)) {
      inputNotes.delete(n);
      if (session.debug) log('debug', `Note Off ${noteName(n)} (${n})`);
    }
  }
  for (const n of next) {
    if (!inputNotes.has(n)) {
      inputNotes.add(n);
      // Bass soft, melody loud, with some variation.
      noteVel.set(n, clamp(Math.round(25 + (n - 40) * 2.2 + Math.random() * 25), 10, 127));
      if (session.debug) {
        const out = clamp(n + session.transpose, 0, 127);
        log('debug', `Note On  ${noteName(n)} (${n}) -> ${noteName(out)} (${out})`);
      }
    }
  }
  pushKeys();
}

/** Moves the wheels now and then so they can be seen working. */
let wheelTimer: ReturnType<typeof setInterval> | undefined;
let wheelPhase = 0;
function wheelTick() {
  if (!visible) return;
  wheelPhase++;
  const cycle = wheelPhase % 160;
  // A bend up and back during the first 40 ticks of each cycle, the mod wheel rising and falling.
  const pitch = cycle < 40 ? Math.round(8192 + Math.sin((cycle / 40) * Math.PI) * 6000) : 8192;
  const modw = Math.round(64 + Math.sin(wheelPhase / 30) * 63);
  if (pitch !== wheels[0] || modw !== wheels[1]) {
    wheels = [pitch, modw];
    pushKeys();
  }
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

/** Simulates a double-click on a project file while the app runs. */
function systemOpen(path: string) {
  pendingOpen = path;
  queue({ open_pending: true });
}

if (typeof window !== 'undefined') {
  (window as unknown as { mockOpenFile: (path: string) => void }).mockOpenFile = systemOpen;
}

let updateTimer: ReturnType<typeof setInterval> | undefined;

/** Fake download: 18 MB in about three seconds, then "installing". */
function fakeInstall() {
  const total = 18_400_000;
  let downloaded = 0;
  clearInterval(updateTimer);
  queue({ update: { state: 'downloading', downloaded, total: null } });
  updateTimer = setInterval(() => {
    downloaded = Math.min(total, downloaded + 1_150_000);
    if (UPDATE_PARAM === 'error' && downloaded > total / 2) {
      clearInterval(updateTimer);
      queue({ update: { state: 'error', detail: 'Download failed: connection reset by peer' } });
      return;
    }
    queue({ update: { state: 'downloading', downloaded, total } });
    if (downloaded >= total) {
      clearInterval(updateTimer);
      queue({ update: { state: 'installing' } });
      log('warn', 'Installing update 2.1.1. In the app it would restart now (browser mock).');
    }
  }, 200);
}

export async function mockSubscribe(onFrame: (frame: Frame) => void): Promise<Init> {
  if (!seeded) seed();
  listener = onFrame;
  pending = null;

  clearInterval(demoTimer);
  clearInterval(wheelTimer);
  if (params.get('demo') !== '0') {
    for (const n of DEMO_STEPS[1]) {
      inputNotes.add(n);
      noteVel.set(n, 40 + ((n * 7) % 80));
    }
    demoStep = 2;
    demoTimer = setInterval(demoTick, 900);
    wheelTimer = setInterval(wheelTick, 50);
  }
  if (params.get('close') === '1') {
    working.osc.target_port = 9001;
    setTimeout(() => queue({ snap: snapshot(), close_requested: true }), 150);
  }
  if (params.get('pending') === '1') {
    setTimeout(() => systemOpen(PRACTICE_PATH), 2000);
  }

  return {
    ...state(),
    app: APP,
    banner: BANNER,
    snapshot: snapshot(),
    ui: loadUi(),
    logs: [...logs],
    ...visual(),
  };
}

export async function mockInvoke(cmd: string, args: Record<string, unknown> = {}): Promise<unknown> {
  switch (cmd) {
    case 'set_visible':
      visible = Boolean(args.visible);
      if (visible) queue({ snap: snapshot(), ...visual() });
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
      if (!workingPath) throw 'NO_PATH';
      writeFile(workingPath);
      return state();
    case 'save_project_as': {
      // Browser mock: no dialog. The file is named after the project, in the
      // folder of the current file or the documents folder.
      if (params.get('dialog') === 'cancel') return null;
      const dir = workingPath ? workingPath.slice(0, Math.max(workingPath.lastIndexOf('\\'), 0)) : DOCS;
      const base = (working.name || 'Untitled').replace(/[\\/:*?"<>|]/g, '_');
      writeFile(`${dir}\\${base}.vrcmt`);
      return state();
    }
    case 'revert_project':
      working = clone(baseline);
      pushSnapshot();
      return state();
    case 'new_project':
      open(makeProject(`p-${nextId++}`, 'Untitled'), null);
      return state();
    case 'open_project_dialog': {
      if (params.get('dialog') === 'cancel') return null;
      // Browser mock: no dialog. Alternates between a project file and a v1 config.json.
      if (workingPath !== PRACTICE_PATH && files.has(PRACTICE_PATH)) {
        open(files.get(PRACTICE_PATH)!, PRACTICE_PATH);
        return state();
      }
      // What importing a v1 config.json produces: ports are name patterns without an id.
      const p = makeProject(`p-${nextId++}`, 'config');
      p.midi.input = { id: '', name: 'MRCC' };
      p.midi.output = { id: '', name: 'loopMIDI' };
      p.osc.sending_addresses = [
        { name: 'Hue Shift', addr: '/avatar/parameters/HUE', type: 'float', default: 0, min: 0, max: 1 },
      ];
      open(p, null);
      log('success', 'Imported C:\\Users\\you\\Downloads\\config.json (browser mock: no file dialog)');
      return state();
    }
    case 'open_file': {
      const path = String(args.path);
      const file = files.get(path);
      if (!file) throw `Cannot read ${path}: The system cannot find the file specified. (os error 2)`;
      open(file, path);
      return state();
    }
    case 'remove_recent':
      recentPaths = recentPaths.filter((p) => p !== String(args.path));
      return state();
    case 'take_pending_open': {
      const path = pendingOpen;
      pendingOpen = null;
      return path;
    }

    case 'check_update':
      await new Promise((r) => setTimeout(r, 600));
      return clone(UPDATE);
    case 'install_update':
      if (APP.update_mode !== 'auto') {
        throw 'This installation cannot update itself. Download the new version from the release page.';
      }
      fakeInstall();
      return;
    case 'open_release':
      window.open(`${APP.repository}/releases/tag/v${String(args.version)}`, '_blank', 'noopener');
      return;

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
