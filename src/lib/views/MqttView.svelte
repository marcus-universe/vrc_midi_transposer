<script lang="ts">
  import { app } from '../app.svelte';
  import { relativeTime, slug } from '../format';
  import { anyText, host, mqttTopic, port } from '../parsers';
  import type { LastCommand } from '../ipc';
  import Field from '../components/Field.svelte';
  import Icon from '../components/Icon.svelte';
  import Switch from '../components/Switch.svelte';

  const m = $derived(app.project.mqtt);
  const s = $derived(app.snapshot);

  const touch = () => app.touch();
  const done = () => void app.flush();

  // --- Password (write-only) ----------------------------------------------
  let password = $state('');
  let reveal = $state(false);

  function commitPassword() {
    if (password === '') return;
    m.password = password;
    password = '';
    reveal = false;
    app.touch(true);
  }

  function clearPassword() {
    password = '';
    m.password = '';
    m.password_set = false;
    app.touch(true);
  }

  // --- Status --------------------------------------------------------------
  const broker = $derived.by(() => {
    switch (s.mqtt.state) {
      case 'connected':
        return { tone: 'ok', text: `Connected to ${m.broker_host}:${m.broker_port}` };
      case 'connecting':
        return { tone: 'warn', text: `Connecting to ${m.broker_host}:${m.broker_port}…` };
      case 'error':
        return { tone: 'err', text: 'Error' };
      default:
        return { tone: '', text: 'Off' };
    }
  });

  /** The console can switch the client on or off without touching the project. */
  const sessionNote = $derived(
    s.mqtt_enabled === m.enabled
      ? ''
      : s.mqtt_enabled
        ? 'Turned on for this session from the console. The project still has MQTT off.'
        : 'Turned off for this session from the console. The project still has MQTT on.',
  );

  function describeCommand(c: LastCommand): string {
    switch (c.kind) {
      case 'transpose':
        return `Transpose = ${c.value}`;
      case 'transpose_up':
        return 'Transpose up';
      case 'transpose_down':
        return 'Transpose down';
      case 'osc_sending':
        return `OSC sending ${c.on ? 'on' : 'off'}`;
      case 'send_original':
        return c.on ? 'Send original notes' : 'Send transposed notes';
      case 'debug':
        return `Debug ${c.on ? 'on' : 'off'}`;
      case 'custom':
        return `${c.name} = ${c.value}`;
    }
  }

  // --- Topic reference -----------------------------------------------------
  let showTopics = $state(false);

  const topics = $derived.by(() => {
    const b = m.base_topic || 'midi_transposer';
    const rows: { what: string; command: string; state: string }[] = [
      { what: 'Transpose', command: `${b}/transpose`, state: `${b}/state/transpose` },
      { what: 'Transpose up', command: `${b}/transposeUp`, state: `${b}/state/transpose` },
      { what: 'Transpose down', command: `${b}/transposeDown`, state: `${b}/state/transpose` },
      { what: 'OSC sending', command: `${b}/osc/sendingEnabled`, state: `${b}/state/osc/sendingEnabled` },
      { what: 'Send original notes', command: `${b}/osc/sendOriginal`, state: `${b}/state/osc/sendOriginal` },
      { what: 'Debug', command: `${b}/debug/enabled`, state: `${b}/state/debug/enabled` },
    ];
    for (const c of app.project.osc.sending_addresses) {
      const id = slug(c.name);
      rows.push({
        what: c.name || 'Unnamed control',
        command: `${b}/osc/custom/${id}/set`,
        state: `${b}/state/osc/custom/${id}`,
      });
    }
    rows.push({ what: 'Availability', command: '', state: `${b}/availability` });
    return rows;
  });
</script>

<div class="view">
  <header class="view-head">
    <h1>MQTT</h1>
    <p>Control the transposer from Home Assistant or any MQTT client. Entities are announced automatically.</p>
  </header>

  <div class="duo">
    <section class="card" aria-labelledby="mqtt-conn-title">
      <div class="card-head">
        <h2 class="card-title" id="mqtt-conn-title">Broker</h2>
      </div>

      <Switch
        label="Connect to an MQTT broker"
        hint="Saved with the project and applied when it opens."
        checked={m.enabled}
        onchange={(on) => {
          m.enabled = on;
          app.touch(true);
        }}
      />

      <hr class="divider" />

      <div class="pair">
        <Field
          label="Broker host"
          value={m.broker_host}
          parse={host}
          oncommit={(v) => ((m.broker_host = v), touch())}
          ondone={done}
          mono
          required
        />
        <Field
          label="Port"
          value={m.broker_port}
          parse={port}
          oncommit={(v) => ((m.broker_port = v), touch())}
          ondone={done}
          numeric
          required
        />
      </div>

      <Field
        label="Base topic"
        hint="Prefix of every topic this app uses."
        value={m.base_topic}
        parse={mqttTopic}
        oncommit={(v) => ((m.base_topic = v), touch())}
        ondone={done}
        mono
        required
      />

      <div class="halves">
        <Field
          label="Username"
          value={m.username}
          parse={anyText}
          oncommit={(v) => ((m.username = v), touch())}
          ondone={done}
          autocomplete="username"
          placeholder="None"
        />

        <div class="field">
          <label class="label" for="mqtt-password">Password</label>
          <!-- The password is handed over when focus leaves the field and its Show button. -->
          <div
            class="secret"
            onfocusout={(e) => {
              if (!e.currentTarget.contains(e.relatedTarget as Node | null)) commitPassword();
            }}
          >
            <input
              class="input"
              id="mqtt-password"
              name="mqtt-password"
              type={reveal ? 'text' : 'password'}
              autocomplete="current-password"
              autocapitalize="none"
              spellcheck="false"
              placeholder={m.password_set ? 'Type to replace' : 'None'}
              aria-describedby="mqtt-password-note"
              bind:value={password}
              onkeydown={(e) => {
                if (e.key === 'Enter' && !e.isComposing) commitPassword();
              }}
            />
            <button
              type="button"
              class={['btn btn-ghost btn-icon', reveal && 'pressed']}
              aria-label="Show password"
              aria-pressed={reveal}
              disabled={password === ''}
              onpointerdown={(e) => e.preventDefault()}
              onclick={() => (reveal = !reveal)}
            >
              <Icon name="eye" size={16} />
            </button>
          </div>
        </div>
      </div>

      <p class="hint note" id="mqtt-password-note">
        {#if m.password_set}
          <span class="set"><Icon name="check" size={14} /> Password is set.</span>
          It is stored on this computer and never shown or exported.
          <button type="button" class="link" onclick={clearPassword}>Clear password</button>
        {:else}
          No password is set. A password you enter is stored on this computer and never shown or exported.
        {/if}
      </p>
    </section>

    <section class="card" aria-labelledby="mqtt-status-title">
      <div class="card-head">
        <h2 class="card-title" id="mqtt-status-title">Status</h2>
      </div>

      <dl class="status">
        <dt>Broker</dt>
        <dd>
          <span class="line"><span class={['dot', broker.tone]}></span><span>{broker.text}</span></span>
          {#if s.mqtt.state === 'error' && s.mqtt.detail}<span class="detail err">{s.mqtt.detail}</span>{/if}
          {#if sessionNote}<span class="detail">{sessionNote}</span>{/if}
        </dd>

        <dt>Discovery</dt>
        <dd>
          <span class="line num">
            {s.mqtt.entities}
            {s.mqtt.entities === 1 ? 'entity' : 'entities'} published
          </span>
        </dd>

        <dt>Last command</dt>
        <dd>
          {#if s.mqtt.last_command}
            <span class="line mono selectable">{describeCommand(s.mqtt.last_command)}</span>
            <span class="detail num">{relativeTime(s.mqtt.last_command_ts, app.now)}</span>
          {:else}
            <span class="line">None received yet</span>
          {/if}
        </dd>
      </dl>
    </section>
  </div>

  <section class="card" aria-labelledby="mqtt-topics-title">
    <div class="card-head">
      <h2 class="card-title" id="mqtt-topics-title">Topic reference</h2>
      <button
        type="button"
        class="btn btn-ghost"
        aria-expanded={showTopics}
        aria-controls="mqtt-topics"
        onclick={() => (showTopics = !showTopics)}
      >
        {showTopics ? 'Hide topics' : 'Show topics'}
        <span class={['chev', showTopics && 'up']}><Icon name="chevron" size={16} /></span>
      </button>
    </div>

    <div id="mqtt-topics" hidden={!showTopics}>
      <table class="table topics" aria-labelledby="mqtt-topics-title">
        <thead>
          <tr>
            <th scope="col" class="col-what">Control</th>
            <th scope="col">Command topic (send to)</th>
            <th scope="col">State topic (read from)</th>
          </tr>
        </thead>
        <tbody>
          {#each topics as t (t.what + t.command)}
            <tr>
              <th scope="row">{t.what}</th>
              <td class="mono selectable">{t.command || '–'}</td>
              <td class="mono selectable">{t.state}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      <p class="hint">
        Send 1, true or on to switch something on; anything else switches it off. Transpose takes a number. State
        topics are retained.
      </p>
    </div>
    {#if !showTopics}
      <p class="hint">
        Command and state topics for the base topic <span class="mono">{m.base_topic}</span> and your
        {app.project.osc.sending_addresses.length} custom
        {app.project.osc.sending_addresses.length === 1 ? 'control' : 'controls'}.
      </p>
    {/if}
  </section>
</div>

<style>
  .duo {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--sp-4);
    align-items: start;
  }

  @container main (min-width: 760px) {
    .duo {
      grid-template-columns: minmax(0, 1.15fr) minmax(0, 1fr);
    }
  }

  .pair {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 92px;
    gap: var(--sp-3);
    align-items: start;
  }

  .halves {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-3);
    align-items: start;
  }

  .secret {
    display: flex;
    gap: var(--sp-1);
  }

  .secret .btn {
    flex: none;
    block-size: var(--control-h);
  }

  .pressed {
    background: var(--accent-soft);
    color: var(--text);
  }

  .note {
    line-height: 1.5;
  }

  .set {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--text);
    font-weight: 500;
    vertical-align: bottom;
  }

  .set :global(svg) {
    color: var(--ok);
  }

  .status {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: var(--sp-3) var(--sp-5);
  }

  .status dt {
    color: var(--text-dim);
    font-size: var(--fs-sm);
    line-height: 1.6;
  }

  .status dd {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-inline-size: 0;
  }

  .line {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    min-inline-size: 0;
    overflow-wrap: anywhere;
  }

  .detail {
    color: var(--text-dim);
    font-size: var(--fs-sm);
  }

  .detail.err {
    color: var(--err);
  }

  .chev {
    display: flex;
    transition: rotate var(--dur) var(--ease);
  }

  .chev.up {
    rotate: 180deg;
  }

  #mqtt-topics:not([hidden]) {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }

  .topics th[scope='row'] {
    padding: 6px var(--sp-2) 6px 0;
    color: var(--text);
    font-size: var(--fs-md);
    font-weight: 400;
    vertical-align: top;
  }

  .topics td {
    padding-block: 6px;
    color: var(--text-dim);
    overflow-wrap: anywhere;
  }

  .topics tbody tr {
    border-block-start: 1px solid var(--line);
  }

  .col-what {
    inline-size: 200px;
  }
</style>
