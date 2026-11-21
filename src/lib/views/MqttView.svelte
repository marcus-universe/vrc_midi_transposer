<script lang="ts">
  import { app } from '../app.svelte';
  import { relativeTime, slug } from '../format';
  import { formatNumber, t, tn, tpn } from '../i18n/i18n.svelte';
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
        return { tone: 'ok', text: t('mqtt.connectedTo', { address: `${m.broker_host}:${m.broker_port}` }) };
      case 'connecting':
        return { tone: 'warn', text: t('mqtt.connectingTo', { address: `${m.broker_host}:${m.broker_port}` }) };
      case 'error':
        return { tone: 'err', text: t('status.error') };
      default:
        return { tone: '', text: t('status.off') };
    }
  });

  /** The console can switch the client on or off without touching the project. */
  const sessionNote = $derived(
    s.mqtt_enabled === m.enabled
      ? ''
      : s.mqtt_enabled
        ? t('mqtt.sessionOn')
        : t('mqtt.sessionOff'),
  );

  function describeCommand(c: LastCommand): string {
    switch (c.kind) {
      case 'transpose':
        return t('mqtt.cmd.transpose', { value: formatNumber(c.value) });
      case 'transpose_up':
        return t('mqtt.cmd.transposeUp');
      case 'transpose_down':
        return t('mqtt.cmd.transposeDown');
      case 'osc_sending':
        return c.on ? t('mqtt.cmd.oscSendingOn') : t('mqtt.cmd.oscSendingOff');
      case 'send_original':
        return c.on ? t('mqtt.cmd.sendOriginal') : t('mqtt.cmd.sendTransposed');
      case 'debug':
        return c.on ? t('mqtt.cmd.debugOn') : t('mqtt.cmd.debugOff');
      case 'custom':
        // The payload is shown as received ("1", "0.35"): it is what the client sent.
        return t('mqtt.cmd.custom', { name: c.name, value: c.value });
    }
  }

  // --- Topic reference -----------------------------------------------------
  let showTopics = $state(false);

  const topics = $derived.by(() => {
    const b = m.base_topic || 'midi_transposer';
    const rows: { what: string; command: string; state: string }[] = [
      { what: t('mqtt.rowTranspose'), command: `${b}/transpose`, state: `${b}/state/transpose` },
      { what: t('mqtt.rowTransposeUp'), command: `${b}/transposeUp`, state: `${b}/state/transpose` },
      { what: t('mqtt.rowTransposeDown'), command: `${b}/transposeDown`, state: `${b}/state/transpose` },
      { what: t('mqtt.rowOscSending'), command: `${b}/osc/sendingEnabled`, state: `${b}/state/osc/sendingEnabled` },
      { what: t('mqtt.rowSendOriginal'), command: `${b}/osc/sendOriginal`, state: `${b}/state/osc/sendOriginal` },
      { what: t('mqtt.rowDebug'), command: `${b}/debug/enabled`, state: `${b}/state/debug/enabled` },
    ];
    for (const c of app.project.osc.sending_addresses) {
      const id = slug(c.name);
      rows.push({
        what: c.name || t('common.unnamedControl'),
        command: `${b}/osc/custom/${id}/set`,
        state: `${b}/state/osc/custom/${id}`,
      });
    }
    rows.push({ what: t('mqtt.rowAvailability'), command: '', state: `${b}/availability` });
    return rows;
  });
</script>

<div class="view">
  <header class="view-head">
    <h1>MQTT</h1>
    <p>{t('mqtt.intro')}</p>
  </header>

  <div class="duo">
    <section class="card" aria-labelledby="mqtt-conn-title">
      <div class="card-head">
        <h2 class="card-title" id="mqtt-conn-title">{t('mqtt.broker')}</h2>
      </div>

      <Switch
        label={t('mqtt.connect')}
        hint={t('mqtt.connectHint')}
        checked={m.enabled}
        onchange={(on) => {
          m.enabled = on;
          app.touch(true);
        }}
      />

      <hr class="divider" />

      <div class="pair">
        <Field
          label={t('mqtt.brokerHost')}
          value={m.broker_host}
          parse={host}
          oncommit={(v) => ((m.broker_host = v), touch())}
          ondone={done}
          mono
          required
        />
        <Field
          label={t('common.port')}
          value={m.broker_port}
          parse={port}
          oncommit={(v) => ((m.broker_port = v), touch())}
          ondone={done}
          numeric
          required
        />
      </div>

      <Field
        label={t('mqtt.baseTopic')}
        hint={t('mqtt.baseTopicHint')}
        value={m.base_topic}
        parse={mqttTopic}
        oncommit={(v) => ((m.base_topic = v), touch())}
        ondone={done}
        mono
        required
      />

      <div class="halves">
        <Field
          label={t('mqtt.username')}
          value={m.username}
          parse={anyText}
          oncommit={(v) => ((m.username = v), touch())}
          ondone={done}
          autocomplete="username"
          placeholder={t('common.none')}
        />

        <div class="field">
          <label class="label" for="mqtt-password">{t('mqtt.password')}</label>
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
              placeholder={m.password_set ? t('mqtt.passwordReplace') : t('common.none')}
              aria-describedby="mqtt-password-note"
              bind:value={password}
              onkeydown={(e) => {
                if (e.key === 'Enter' && !e.isComposing) commitPassword();
              }}
            />
            <button
              type="button"
              class={['btn btn-ghost btn-icon', reveal && 'pressed']}
              aria-label={t('mqtt.showPassword')}
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
          <span class="set"><Icon name="check" size={14} /> {t('mqtt.passwordSet')}</span>
          {t('mqtt.passwordSetNote')}
          <button type="button" class="link" onclick={clearPassword}>{t('mqtt.clearPassword')}</button>
        {:else}
          {t('mqtt.passwordNone')}
        {/if}
      </p>
    </section>

    <section class="card" aria-labelledby="mqtt-status-title">
      <div class="card-head">
        <h2 class="card-title" id="mqtt-status-title">{t('common.status')}</h2>
      </div>

      <dl class="status">
        <dt>{t('mqtt.broker')}</dt>
        <dd>
          <span class="line"><span class={['dot', broker.tone]}></span><span>{broker.text}</span></span>
          {#if s.mqtt.state === 'error' && s.mqtt.detail}<span class="detail err">{s.mqtt.detail}</span>{/if}
          {#if sessionNote}<span class="detail">{sessionNote}</span>{/if}
        </dd>

        <dt>{t('mqtt.discovery')}</dt>
        <dd>
          <span class="line num">{tn('mqtt.entities', s.mqtt.entities)}</span>
        </dd>

        <dt>{t('mqtt.lastCommand')}</dt>
        <dd>
          {#if s.mqtt.last_command}
            <span class="line mono selectable">{describeCommand(s.mqtt.last_command)}</span>
            <span class="detail num">{relativeTime(s.mqtt.last_command_ts, app.now)}</span>
          {:else}
            <span class="line">{t('mqtt.noCommand')}</span>
          {/if}
        </dd>
      </dl>
    </section>
  </div>

  <section class="card" aria-labelledby="mqtt-topics-title">
    <div class="card-head">
      <h2 class="card-title" id="mqtt-topics-title">{t('mqtt.topicReference')}</h2>
      <button
        type="button"
        class="btn btn-ghost"
        aria-expanded={showTopics}
        aria-controls="mqtt-topics"
        onclick={() => (showTopics = !showTopics)}
      >
        {showTopics ? t('mqtt.hideTopics') : t('mqtt.showTopics')}
        <span class={['chev', showTopics && 'up']}><Icon name="chevron" size={16} /></span>
      </button>
    </div>

    <div id="mqtt-topics" hidden={!showTopics}>
      <table class="table topics" aria-labelledby="mqtt-topics-title">
        <thead>
          <tr>
            <th scope="col" class="col-what">{t('mqtt.colControl')}</th>
            <th scope="col">{t('mqtt.colCommand')}</th>
            <th scope="col">{t('mqtt.colState')}</th>
          </tr>
        </thead>
        <tbody>
          {#each topics as row (row.what + row.command)}
            <tr>
              <th scope="row">{row.what}</th>
              <td class="mono selectable">{row.command || '–'}</td>
              <td class="mono selectable">{row.state}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      <p class="hint">{t('mqtt.topicsHint')}</p>
    </div>
    {#if !showTopics}
      <p class="hint">
        {#each tpn('mqtt.topicsSummary', app.project.osc.sending_addresses.length, { topic: m.base_topic }) as part, i (i)}{#if part.param === 'topic'}<span
              class="mono">{part.text}</span
            >{:else}{part.text}{/if}{/each}
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
