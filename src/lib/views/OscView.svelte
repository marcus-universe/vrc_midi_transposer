<script lang="ts">
  import { onDestroy, tick, untrack } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import { app } from '../app.svelte';
  import { DEFAULT_LISTEN_PATHS, clamp, noteName, slug, triggerLabel } from '../format';
  import { listParts, t, tp } from '../i18n/i18n.svelte';
  import { OSC_ROLES, type MidiTrigger, type OscRole, type SendAddress } from '../ipc';
  import { float, host, integer, nonEmpty, optionalFloat, oscPath, port } from '../parsers';
  import Field from '../components/Field.svelte';
  import Icon from '../components/Icon.svelte';
  import Segmented from '../components/Segmented.svelte';
  import Switch from '../components/Switch.svelte';

  const osc = $derived(app.project.osc);
  const s = $derived(app.snapshot);

  const touch = () => app.touch();
  const done = () => void app.flush();

  let listenTable = $state<HTMLTableElement>();
  let controlTable = $state<HTMLTableElement>();

  async function focusLastRow(table: HTMLTableElement | undefined, row = 'tbody tr:last-child') {
    await tick();
    const input = table?.querySelector<HTMLInputElement>(`${row} input.input`);
    input?.focus();
    input?.select();
  }

  // --- Listening addresses -------------------------------------------------
  const roleLabel = (role: OscRole) => t(`osc.roles.${role}.label` as const);
  const roleHint = (role: OscRole | null) => (role ? t(`osc.roles.${role}.hint` as const) : t('osc.ignored'));

  const missingDefaults = $derived(OSC_ROLES.filter((r) => !osc.listening_addresses.some((a) => a.role === r)));

  function addAddress() {
    let n = 1;
    while (osc.listening_addresses.some((a) => a.addr === `/address${n}`)) n++;
    osc.listening_addresses.push({ addr: `/address${n}`, role: null });
    app.touch(true);
    void focusLastRow(listenTable);
  }

  function removeAddress(i: number) {
    osc.listening_addresses.splice(i, 1);
    app.touch(true);
  }

  function restoreDefaults() {
    for (const role of missingDefaults) {
      osc.listening_addresses.push({ addr: DEFAULT_LISTEN_PATHS[role], role });
    }
    app.touch(true);
  }

  // --- Custom controls -----------------------------------------------------
  const controls = $derived(osc.sending_addresses);

  /** Slugs used by more than one control: their MQTT topics would collide. */
  const clashes = $derived.by(() => {
    const count = new Map<string, number>();
    for (const c of controls) count.set(slug(c.name), (count.get(slug(c.name)) ?? 0) + 1);
    return [...count].filter(([, n]) => n > 1).map(([name]) => name);
  });

  function addControl() {
    let n = controls.length + 1;
    while (controls.some((c) => c.name === `Control ${n}`)) n++;
    controls.push({
      name: `Control ${n}`,
      addr: `/avatar/parameters/Control${n}`,
      type: 'bool',
      default: 0,
      min: null,
      max: null,
    });
    app.touch(true);
    void focusLastRow(controlTable, 'tbody:last-of-type tr.main');
  }

  function removeControl(i: number) {
    // Indices shift: a learn in progress would land on the wrong control.
    if (app.learning !== null) app.stopLearn();
    controls.splice(i, 1);
    app.touch(true);
  }

  function setType(c: SendAddress, type: 'bool' | 'float') {
    if (c.type === type) return;
    c.type = type;
    if (type === 'bool') {
      c.default = c.default >= 0.5 ? 1 : 0;
      c.min = null;
      c.max = null;
    } else {
      c.min = 0;
      c.max = 1;
      c.default = clamp(c.default, 0, 1);
    }
    app.touch(true);
  }

  const inRange = (c: SendAddress) => (n: number) =>
    (c.min !== null && n < c.min) || (c.max !== null && n > c.max)
      ? t('osc.defaultOutOfRange')
      : null;
  const belowMax = (c: SendAddress) => (n: number) => (c.max !== null && n >= c.max ? t('errors.minBelowMax') : null);
  const aboveMin = (c: SendAddress) => (n: number) => (c.min !== null && n <= c.min ? t('errors.maxAboveMin') : null);

  /** The control's name for labels, or "control 3" while it has none. */
  const nameOf = (c: SendAddress, i: number) => c.name || t('osc.controlFallback', { n: i + 1 });

  // --- MIDI triggers ---------------------------------------------------------
  /** Controls whose trigger editor is open. */
  const expanded = new SvelteSet<SendAddress>();
  const CHANNELS = Array.from({ length: 16 }, (_, k) => k + 1);

  /** The last learn result, shown under its control until the next learn starts. */
  let lastLearned = $state<{ seq: number; index: number } | null>(null);
  $effect(() => {
    const learned = s.learned;
    untrack(() => {
      if (learned && learned.seq !== lastLearned?.seq) lastLearned = { seq: learned.seq, index: learned.index };
    });
  });
  // The result that was there when the view opened is old news.
  untrack(() => {
    if (app.snapshot.learned) lastLearned = { seq: app.snapshot.learned.seq, index: -1 };
  });

  function toggleTrigger(c: SendAddress) {
    if (expanded.has(c)) expanded.delete(c);
    else expanded.add(c);
  }

  function setKind(c: SendAddress, kind: string) {
    if (kind !== 'cc' && kind !== 'note' && kind !== 'program') {
      delete c.midi;
    } else if (!c.midi) {
      c.midi = { kind, channel: null, number: kind === 'note' ? 60 : 0, mode: 'toggle', forward: false };
    } else {
      c.midi.kind = kind;
      if (kind !== 'note') c.midi.mode = 'toggle';
    }
    app.touch(true);
  }

  function learn(c: SendAddress, i: number) {
    if (app.learning === i) {
      app.stopLearn();
      return;
    }
    expanded.add(c);
    lastLearned = lastLearned && { ...lastLearned, index: -1 };
    app.startLearn(i);
  }

  const midiNumber = integer((n) => (n < 0 || n > 127 ? t('midi.numberRange') : null));

  const numberLabel = (m: MidiTrigger) =>
    m.kind === 'cc' ? t('midi.ccNumber') : m.kind === 'note' ? t('midi.noteNumber') : t('midi.programNumber');

  // Esc stops MIDI Learn; leaving the view does too.
  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && app.learning !== null && !app.dialog) {
      e.preventDefault();
      app.stopLearn();
    }
  }
  onDestroy(() => app.stopLearn());
</script>

<svelte:window onkeydown={onKeydown} />

<div class="view">
  <header class="view-head">
    <h1>OSC</h1>
    <p>{t('osc.intro')}</p>
  </header>

  <div class="trio">
    <section class="card" aria-labelledby="osc-listener-title">
      <div class="card-head">
        <h2 class="card-title" id="osc-listener-title">{t('osc.listener')}</h2>
        <span class="state">
          <span class={['dot', s.osc_listener.state === 'running' ? 'ok' : s.osc_listener.state === 'error' ? 'err' : '']}></span>
          {s.osc_listener.state === 'running'
            ? t('status.running')
            : s.osc_listener.state === 'error'
              ? t('status.error')
              : t('status.stopped')}
        </span>
      </div>
      <div class="pair">
        <Field
          label={t('common.host')}
          value={osc.listen_host}
          parse={host}
          oncommit={(v) => ((osc.listen_host = v), touch())}
          ondone={done}
          mono
          required
        />
        <Field
          label={t('common.port')}
          value={osc.listen_port}
          parse={port}
          oncommit={(v) => ((osc.listen_port = v), touch())}
          ondone={done}
          numeric
          required
        />
      </div>
      {#if s.osc_listener.state === 'error' && s.osc_listener.detail}
        <p class="problem"><Icon name="alert" size={14} /><span>{s.osc_listener.detail}</span></p>
      {:else}
        <p class="hint">{t('osc.listenerHint')}</p>
      {/if}
    </section>

    <section class="card" aria-labelledby="osc-target-title">
      <div class="card-head">
        <h2 class="card-title" id="osc-target-title">{t('osc.target')}</h2>
      </div>
      <div class="pair">
        <Field
          label={t('common.host')}
          value={osc.target_host}
          parse={host}
          oncommit={(v) => ((osc.target_host = v), touch())}
          ondone={done}
          mono
          required
        />
        <Field
          label={t('common.port')}
          value={osc.target_port}
          parse={port}
          oncommit={(v) => ((osc.target_port = v), touch())}
          ondone={done}
          numeric
          required
        />
      </div>
      <p class="hint">{t('osc.targetHint')}</p>
    </section>

    <section class="card" aria-labelledby="osc-startup-title">
      <div class="card-head">
        <h2 class="card-title" id="osc-startup-title">{t('osc.startup')}</h2>
      </div>
      <Switch
        label={t('osc.startSending')}
        checked={osc.sending_enabled}
        onchange={(on) => {
          osc.sending_enabled = on;
          app.touch(true);
        }}
      />
      <Segmented
        showLabel
        label={t('osc.startBy')}
        options={[
          { value: 'original', label: t('osc.originalNotes') },
          { value: 'transposed', label: t('osc.transposedNotes') },
        ]}
        value={osc.send_original ? 'original' : 'transposed'}
        onchange={(v) => {
          osc.send_original = v === 'original';
          app.touch(true);
        }}
      />
    </section>
  </div>

  <section class="card" aria-labelledby="osc-listen-title">
    <div class="card-head">
      <h2 class="card-title" id="osc-listen-title">{t('osc.listening')}</h2>
      <div class="card-actions">
        <button
          type="button"
          class="btn btn-ghost"
          disabled={!missingDefaults.length}
          title={missingDefaults.length ? t('osc.restoreTip') : t('osc.restoreTipNone')}
          onclick={restoreDefaults}
        >
          {t('osc.restoreDefaults')}
        </button>
      </div>
    </div>

    {#if osc.listening_addresses.length}
      <table class="table" bind:this={listenTable} aria-labelledby="osc-listen-title">
        <thead>
          <tr>
            <th scope="col" class="col-role">{t('osc.colAction')}</th>
            <th scope="col">{t('osc.colAddress')}</th>
            <th scope="col" class="col-hint">{t('osc.colExpects')}</th>
            <th scope="col" class="col-x"><span class="visually-hidden">{t('osc.remove')}</span></th>
          </tr>
        </thead>
        <tbody>
          {#each osc.listening_addresses as a, i (a)}
            <tr>
              <td>
                <select
                  class="select"
                  name="listen-role-{i}"
                  aria-label={t('osc.actionFor', { n: i + 1 })}
                  value={a.role ?? ''}
                  onchange={(e) => {
                    a.role = (e.currentTarget.value || null) as OscRole | null;
                    app.touch(true);
                  }}
                >
                  {#each OSC_ROLES as r (r)}
                    <option value={r}>{roleLabel(r)}</option>
                  {/each}
                  <option value="">{t('osc.noAction')}</option>
                </select>
              </td>
              <td>
                <Field
                  bare
                  label={t('osc.addressN', { n: i + 1 })}
                  value={a.addr}
                  parse={oscPath}
                  oncommit={(v) => ((a.addr = v), touch())}
                  ondone={done}
                  mono
                  required
                  placeholder="/address"
                />
              </td>
              <td><span class="cell-text">{roleHint(a.role)}</span></td>
              <td>
                <button
                  type="button"
                  class="btn btn-ghost btn-icon"
                  aria-label={t('osc.removeAddress', { address: a.addr || i + 1 })}
                  onclick={() => removeAddress(i)}
                >
                  <Icon name="trash" size={16} />
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <p class="empty">{t('osc.listenEmpty')}</p>
    {/if}

    <div class="under">
      <button type="button" class="btn" onclick={addAddress}>
        <Icon name="plus" size={16} />
        {t('osc.addAddress')}
      </button>
      <span class="hint">{t('osc.listenHint')}</span>
    </div>
  </section>

  <section class="card" aria-labelledby="osc-controls-title">
    <div class="card-head">
      <h2 class="card-title" id="osc-controls-title">{t('common.customControls')}</h2>
    </div>

    {#if controls.length}
      <table class="table" bind:this={controlTable} aria-labelledby="osc-controls-title">
        <thead>
          <tr>
            <th scope="col" class="col-name">{t('osc.colName')}</th>
            <th scope="col">{t('osc.colAddress')}</th>
            <th scope="col" class="col-type">{t('osc.colType')}</th>
            <th scope="col" class="col-default">{t('osc.colDefault')}</th>
            <th scope="col" class="col-num">{t('osc.colMin')}</th>
            <th scope="col" class="col-num">{t('osc.colMax')}</th>
            <th scope="col" class="col-x"><span class="visually-hidden">{t('osc.remove')}</span></th>
          </tr>
        </thead>
        {#each controls as c, i (c)}
          {@const listening = app.learning === i}
          {@const open = listening || expanded.has(c)}
          <!-- One body per control: its settings, then its MIDI trigger. -->
          <tbody class="group">
            <tr class="main">
              <td>
                <Field
                  bare
                  label={t('osc.nameOf', { n: i + 1 })}
                  value={c.name}
                  parse={nonEmpty('errors.controlName')}
                  oncommit={(v) => ((c.name = v), touch())}
                  ondone={done}
                  required
                />
              </td>
              <td>
                <Field
                  bare
                  label={t('osc.addressOf', { name: nameOf(c, i) })}
                  value={c.addr}
                  parse={oscPath}
                  oncommit={(v) => ((c.addr = v), touch())}
                  ondone={done}
                  mono
                  required
                  placeholder="/avatar/parameters/Name"
                />
              </td>
              <td>
                <select
                  class="select"
                  name="control-type-{i}"
                  aria-label={t('osc.typeOf', { name: nameOf(c, i) })}
                  value={c.type}
                  onchange={(e) => setType(c, e.currentTarget.value as 'bool' | 'float')}
                >
                  <option value="bool">{t('osc.typeSwitch')}</option>
                  <option value="float">{t('osc.typeSlider')}</option>
                </select>
              </td>
              {#if c.type === 'bool'}
                <td>
                  <select
                    class="select"
                    name="control-default-{i}"
                    aria-label={t('osc.defaultOf', { name: nameOf(c, i) })}
                    value={c.default >= 0.5 ? '1' : '0'}
                    onchange={(e) => {
                      c.default = Number(e.currentTarget.value);
                      app.touch(true);
                    }}
                  >
                    <option value="0">{t('common.off')}</option>
                    <option value="1">{t('common.on')}</option>
                  </select>
                </td>
                {#each [t('osc.minUnused'), t('osc.maxUnused')] as what (what)}
                  <td>
                    <span class="cell-text">
                      <span aria-hidden="true">–</span>
                      <span class="visually-hidden">{what}</span>
                    </span>
                  </td>
                {/each}
              {:else}
                <td>
                  <Field
                    bare
                    label={t('osc.defaultOf', { name: nameOf(c, i) })}
                    value={c.default}
                    parse={float(inRange(c))}
                    oncommit={(v) => ((c.default = v), touch())}
                    ondone={done}
                    numeric
                    required
                  />
                </td>
                <td>
                  <Field
                    bare
                    label={t('osc.minOf', { name: nameOf(c, i) })}
                    value={c.min}
                    parse={optionalFloat(belowMax(c))}
                    oncommit={(v) => ((c.min = v), touch())}
                    ondone={done}
                    numeric
                    placeholder="0"
                  />
                </td>
                <td>
                  <Field
                    bare
                    label={t('osc.maxOf', { name: nameOf(c, i) })}
                    value={c.max}
                    parse={optionalFloat(aboveMin(c))}
                    oncommit={(v) => ((c.max = v), touch())}
                    ondone={done}
                    numeric
                    placeholder="1"
                  />
                </td>
              {/if}
              <td>
                <button
                  type="button"
                  class="btn btn-ghost btn-icon"
                  aria-label={t('osc.removeControl', { name: c.name || i + 1 })}
                  onclick={() => removeControl(i)}
                >
                  <Icon name="trash" size={16} />
                </button>
              </td>
            </tr>
            <tr class="trigger-row">
              <td colspan="7">
                <div class="trigger-bar">
                  <button
                    type="button"
                    class="disclosure"
                    aria-expanded={open}
                    aria-controls="trigger-{i}"
                    onclick={() => toggleTrigger(c)}
                  >
                    <Icon name="chevron" size={16} />
                    <span class="visually-hidden">{t('midi.triggerOf', { name: nameOf(c, i) })}:</span>
                    <span aria-hidden="true">{t('midi.trigger')}</span>
                    {#if c.midi}
                      <span class="badge">{triggerLabel(c.midi)}</span>
                    {:else}
                      <span class="none">{t('midi.none')}</span>
                    {/if}
                  </button>
                  <p class="learn-status" role="status">
                    {#if listening}
                      <span class="listening"><span class="dot accent" aria-hidden="true"></span>{t('midi.learning')}</span>
                    {:else if lastLearned?.index === i && c.midi}
                      <span class="learned"><Icon name="check" size={14} />{t('midi.learned', { trigger: triggerLabel(c.midi) })}</span>
                    {/if}
                  </p>
                </div>

                <div class="trigger" id="trigger-{i}" hidden={!open}>
                  <div class="field kind">
                    <label class="label" for="trigger-{i}-kind">{t('midi.kind')}</label>
                    <select
                      class="select"
                      id="trigger-{i}-kind"
                      name="trigger-{i}-kind"
                      value={c.midi?.kind ?? ''}
                      onchange={(e) => setKind(c, e.currentTarget.value)}
                    >
                      <option value="">{t('midi.kindNone')}</option>
                      <option value="cc">{t('midi.kindCc')}</option>
                      <option value="note">{t('midi.kindNote')}</option>
                      <option value="program">{t('midi.kindProgram')}</option>
                    </select>
                  </div>

                  <div class="field learn">
                    <!-- Keeps the button on the line of the inputs. -->
                    <span class="label" aria-hidden="true">&nbsp;</span>
                    <button
                      type="button"
                      class={['btn', listening && 'btn-primary']}
                      aria-label={listening ? undefined : t('midi.learnFor', { name: nameOf(c, i) })}
                      onclick={() => learn(c, i)}
                    >
                      <Icon name="learn" size={16} />
                      {listening ? t('midi.learnStop') : t('midi.learn')}
                    </button>
                  </div>

                  {#if c.midi}
                    {@const m = c.midi}
                    <div class="field channel">
                      <label class="label" for="trigger-{i}-channel">{t('midi.channel')}</label>
                      <select
                        class="select"
                        id="trigger-{i}-channel"
                        name="trigger-{i}-channel"
                        value={m.channel === null ? '' : String(m.channel)}
                        onchange={(e) => {
                          const v = e.currentTarget.value;
                          m.channel = v === '' ? null : Number(v);
                          app.touch(true);
                        }}
                      >
                        <option value="">{t('midi.anyChannel')}</option>
                        {#each CHANNELS as ch (ch)}
                          <option value={String(ch)}>{ch}</option>
                        {/each}
                      </select>
                    </div>

                    <div class="number">
                      <Field
                        label={numberLabel(m)}
                        hint={m.kind === 'note' ? t('midi.noteValue', { number: m.number, note: noteName(m.number) }) : '0–127'}
                        value={m.number}
                        parse={midiNumber}
                        oncommit={(v) => ((m.number = v), touch())}
                        ondone={done}
                        numeric
                        required
                      />
                    </div>

                    {#if m.kind === 'note'}
                      <div class="field mode">
                        <label class="label" for="trigger-{i}-mode">{t('midi.mode')}</label>
                        <select
                          class="select"
                          id="trigger-{i}-mode"
                          name="trigger-{i}-mode"
                          value={m.mode}
                          onchange={(e) => {
                            m.mode = e.currentTarget.value === 'momentary' ? 'momentary' : 'toggle';
                            app.touch(true);
                          }}
                        >
                          <option value="toggle">{t('midi.toggle')}</option>
                          <option value="momentary">{t('midi.momentary')}</option>
                        </select>
                      </div>
                    {/if}

                    <div class="forward">
                      <Switch
                        label={t('midi.forward')}
                        hint={t('midi.forwardHint')}
                        checked={m.forward}
                        onchange={(on) => {
                          m.forward = on;
                          app.touch(true);
                        }}
                      />
                    </div>
                  {/if}

                  <p class="hint trigger-hint">{t('osc.triggerHint')}</p>
                </div>
              </td>
            </tr>
          </tbody>
        {/each}
      </table>
    {:else}
      <p class="empty">{t('osc.controlsEmpty')}</p>
    {/if}

    {#if clashes.length}
      <p class="warning" role="status">
        <Icon name="alert" size={14} />
        <!-- The names go where the language puts {names}, each set in code. -->
        <span>
          {#each tp('osc.clash', { names: '' }) as part, i (i)}{#if part.param}{#each listParts(clashes) as item, j (j)}{#if item.item}<code
                  >{item.text}</code
                >{:else}{item.text}{/if}{/each}{:else}{part.text}{/if}{/each}
        </span>
      </p>
    {/if}

    <div class="under">
      <button type="button" class="btn" onclick={addControl}>
        <Icon name="plus" size={16} />
        {t('osc.addControl')}
      </button>
      <span class="hint">{t('osc.controlsHint')}</span>
    </div>
  </section>
</div>

<style>
  .trio {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--sp-4);
  }

  @container main (min-width: 600px) {
    .trio {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .trio > :last-child {
      grid-column: 1 / -1;
    }
  }

  @container main (min-width: 840px) {
    .trio {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
    .trio > :last-child {
      grid-column: auto;
    }
  }

  .pair {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 92px;
    gap: var(--sp-3);
    align-items: start;
  }

  .state {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-dim);
    font-size: var(--fs-sm);
  }

  .problem,
  .warning {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    font-size: var(--fs-sm);
    line-height: 1.4;
  }

  .problem {
    color: var(--err);
  }

  .warning {
    padding: var(--sp-2) var(--sp-3);
    border: 1px solid var(--warn);
    border-radius: var(--r-sm);
    background: var(--warn-soft);
    color: var(--text);
  }

  .problem :global(svg),
  .warning :global(svg) {
    margin-block-start: 2px;
  }

  .warning :global(svg) {
    color: var(--warn);
  }

  code {
    font-family: var(--font-mono);
    font-size: var(--fs-sm);
  }

  .col-role {
    inline-size: 230px;
  }

  .col-hint {
    inline-size: 170px;
  }

  .col-x {
    inline-size: 40px;
  }

  .col-name {
    inline-size: 23%;
  }

  .col-type {
    inline-size: 140px;
  }

  .col-num {
    inline-size: 84px;
  }

  /* Wider than Min and Max: holds the Off/On select and a one-word header (de 'Standardwert'). */
  .col-default {
    inline-size: 104px;
  }

  /* --- One body per custom control ----------------------------------------- */

  .group + .group {
    border-block-start: 1px solid var(--line);
  }

  .group .main td {
    padding-block-start: var(--sp-2);
  }

  .trigger-row td {
    padding-block: 0 var(--sp-2);
  }

  .trigger-bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-1) var(--sp-3);
    min-block-size: 32px;
  }

  .disclosure {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    min-block-size: 32px;
    padding: 0 var(--sp-2) 0 var(--sp-1);
    border: 0;
    border-radius: var(--r-sm);
    background: none;
    color: var(--text-dim);
    font: inherit;
    font-size: var(--fs-sm);
    cursor: pointer;
    transition:
      background-color var(--dur) var(--ease),
      color var(--dur) var(--ease);
  }

  .disclosure:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  /* Points to the inline end while closed, down while open. */
  .disclosure :global(svg) {
    rotate: -90deg;
    transition: rotate var(--dur) var(--ease);
  }

  .disclosure:dir(rtl) :global(svg) {
    rotate: 90deg;
  }

  .disclosure[aria-expanded='true'] :global(svg) {
    rotate: 0deg;
  }

  .disclosure .none {
    color: var(--text-faint);
  }

  .learn-status {
    font-size: var(--fs-sm);
  }

  .learn-status:empty {
    display: none;
  }

  .listening,
  .learned {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .listening {
    color: var(--accent-text);
  }

  .listening .dot {
    background: var(--accent-text);
  }

  .learned {
    color: var(--text-dim);
  }

  .learned :global(svg) {
    color: var(--ok);
  }

  .trigger {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--sp-2) var(--sp-4);
    margin-block: var(--sp-1) var(--sp-2);
    padding: var(--sp-3);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--surface-1);
  }

  .trigger[hidden] {
    display: none;
  }

  .trigger .kind {
    inline-size: 180px;
  }

  .trigger .channel {
    inline-size: 130px;
  }

  .trigger .number {
    inline-size: 120px;
  }

  .trigger .mode {
    inline-size: 180px;
  }

  .trigger .forward {
    inline-size: 240px;
    padding-block-start: 2px;
  }

  /* The note name goes under the input, so every input in the row lines up. */
  .number :global(.hint) {
    order: 3;
    font-variant-numeric: tabular-nums;
  }

  .number :global(.error-msg) {
    order: 4;
  }

  .trigger-hint {
    flex-basis: 100%;
  }

  .under {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-4);
  }
</style>
