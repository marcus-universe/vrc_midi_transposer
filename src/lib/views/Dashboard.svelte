<script lang="ts">
  import { app } from '../app.svelte';
  import { shortNumber, signed, triggerLabel } from '../format';
  import { t, tn } from '../i18n/i18n.svelte';
  import type { MidiStatus, PortInfo } from '../ipc';
  import { NONE, SAVED, midiLine, portFromOption, portSelection } from '../ports';
  import Icon from '../components/Icon.svelte';
  import PatternPanels from '../components/PatternPanels.svelte';
  import PatternStrip from '../components/PatternStrip.svelte';
  import RollingNumber from '../components/RollingNumber.svelte';
  import Segmented from '../components/Segmented.svelte';
  import Slider from '../components/Slider.svelte';
  import Switch from '../components/Switch.svelte';

  const s = $derived(app.snapshot);
  const p = $derived(app.project);

  // --- Transpose -----------------------------------------------------------
  let shown = $state(0);
  const zeroAt = $derived(
    s.transpose_min < 0 && s.transpose_max > 0 ? -s.transpose_min / (s.transpose_max - s.transpose_min) : null,
  );

  function nudge(delta: number) {
    const next = s.transpose + delta;
    if (next < s.transpose_min || next > s.transpose_max) return;
    void app.engine({ type: 'nudge', delta });
  }

  // --- MIDI ports ----------------------------------------------------------
  const inSel = $derived(portSelection(p.midi.input, s.midi_in, s.inputs));
  const outSel = $derived(portSelection(p.midi.output, s.midi_out, s.outputs));

  function choose(which: 'input' | 'output', value: string) {
    const port = portFromOption(value, which === 'input' ? s.inputs : s.outputs);
    if (port === undefined) return;
    p.midi[which] = port;
    app.touch(true);
  }

  // --- Status --------------------------------------------------------------
  const listener = $derived.by(() => {
    const l = s.osc_listener;
    if (l.state === 'running') return { tone: 'ok', text: t('dashboard.listeningOn', { address: l.bound }) };
    if (l.state === 'error') {
      return { tone: 'err', text: l.detail ? t('common.errorDetail', { detail: l.detail }) : t('status.error') };
    }
    return { tone: '', text: t('status.stopped') };
  });

  const mqtt = $derived.by(() => {
    switch (s.mqtt.state) {
      case 'connected':
        return {
          tone: 'ok',
          text: t('dashboard.connectedTo', { address: `${p.mqtt.broker_host}:${p.mqtt.broker_port}` }),
        };
      case 'connecting':
        return { tone: 'warn', text: t('dashboard.connectingEllipsis') };
      case 'error':
        return {
          tone: 'err',
          text: s.mqtt.detail ? t('common.errorDetail', { detail: s.mqtt.detail }) : t('status.error'),
        };
      default:
        return { tone: '', text: t('status.off') };
    }
  });

  // --- Custom controls -----------------------------------------------------
  const controls = $derived(p.osc.sending_addresses);
</script>

<div class="dash">
  <h1 class="visually-hidden">{t('dashboard.title')}</h1>

  <!-- Transpose, with the arpeggiator / sequencer strip directly under it -->
  <div class="lead">
  <section class="hero" aria-labelledby="hero-title">
    <div class="rings" aria-hidden="true"></div>
    <div class="hero-head">
      <h2 id="hero-title">{t('dashboard.transpose')}</h2>
      <span class="unit">{t('dashboard.semitones')}</span>
    </div>

    <div class="hero-main">
      <button
        type="button"
        class="round"
        aria-label={t('dashboard.down')}
        aria-disabled={s.transpose <= s.transpose_min}
        onclick={() => nudge(-1)}
      >
        <Icon name="minus" size={20} />
      </button>
      <span class="value num"><RollingNumber value={shown} format={signed} /></span>
      <button
        type="button"
        class="round"
        aria-label={t('dashboard.up')}
        aria-disabled={s.transpose >= s.transpose_max}
        onclick={() => nudge(1)}
      >
        <Icon name="plus" size={20} />
      </button>
    </div>

    <div class="hero-range">
      <Slider
        aria-label={t('dashboard.transpose')}
        value={s.transpose}
        min={s.transpose_min}
        max={s.transpose_max}
        valueText={tn('dashboard.semitoneValue', Math.abs(shown), { value: signed(shown) })}
        oninput={(value) => app.engineCoalesced('transpose', { type: 'set_transpose', value })}
        bind:shown
      />
      {#if zeroAt !== null}<span class="zero" style:--p={zeroAt} aria-hidden="true"></span>{/if}
    </div>

    <div class="hero-foot">
      <span class="num">{signed(s.transpose_min)}</span>
      <button
        type="button"
        class="reset"
        aria-disabled={s.transpose === 0}
        onclick={() => s.transpose !== 0 && app.engine({ type: 'set_transpose', value: 0 })}
      >
        {t('dashboard.reset')}
      </button>
      <span class="num">{signed(s.transpose_max)}</span>
    </div>
  </section>
  <PatternStrip />
  </div>

  <!-- MIDI -->
  <section class="card" aria-labelledby="midi-title">
    <div class="card-head">
      <h2 class="card-title" id="midi-title">MIDI</h2>
      <button
        type="button"
        class="btn btn-ghost btn-icon"
        aria-label={t('dashboard.refreshPorts')}
        title={t('dashboard.refreshPorts')}
        onclick={() => app.engine({ type: 'refresh_ports' })}
      >
        <Icon name="refresh" size={16} />
      </button>
    </div>

    {#snippet portField(which: 'input' | 'output', label: string, sel: typeof inSel, ports: PortInfo[], st: MidiStatus)}
      {@const line = midiLine(st)}
      <div class="field">
        <label class="label" for="midi-{which}">{label}</label>
        <select
          class="select"
          id="midi-{which}"
          name="midi-{which}"
          value={sel.value}
          aria-describedby="midi-{which}-state"
          onchange={(e) => choose(which, e.currentTarget.value)}
        >
          <option value={NONE}>{t('common.none')}</option>
          {#each ports as port (port.id)}
            <option value={port.id}>{port.name}</option>
          {/each}
          {#if sel.extra}
            <option value={SAVED}>{t('dashboard.notConnected', { name: sel.extra.name || t('dashboard.savedPort') })}</option>
          {/if}
        </select>
        <p class="state" id="midi-{which}-state">
          <span class={['dot', line.tone]}></span>
          <span>{line.text}</span>
        </p>
      </div>
    {/snippet}

    {@render portField('input', t('dashboard.input'), inSel, s.inputs, s.midi_in)}
    {@render portField('output', t('dashboard.output'), outSel, s.outputs, s.midi_out)}
  </section>

  <!-- Status -->
  <section class="card" aria-labelledby="status-title">
    <div class="card-head">
      <h2 class="card-title" id="status-title">{t('common.status')}</h2>
    </div>

    <Switch
      label={t('dashboard.oscSending')}
      hint={s.osc_target
        ? t('dashboard.oscSendingHint', { target: s.osc_target })
        : t('dashboard.oscSendingHintNoTarget')}
      checked={s.osc_sending}
      onchange={(on) => app.engine({ type: 'set_osc_sending', on })}
    />

    <Segmented
      showLabel
      label={t('dashboard.notesToSend')}
      options={[
        { value: 'original', label: t('dashboard.original') },
        { value: 'transposed', label: t('dashboard.transposed') },
      ]}
      value={s.send_original ? 'original' : 'transposed'}
      onchange={(v) => app.engine({ type: 'set_send_original', on: v === 'original' })}
    />

    <hr class="divider" />

    <dl class="states">
      <dt>{t('dashboard.oscListener')}</dt>
      <dd><span class={['dot', listener.tone]}></span><span>{listener.text}</span></dd>
      <dt>MQTT</dt>
      <dd><span class={['dot', mqtt.tone]}></span><span>{mqtt.text}</span></dd>
    </dl>
  </section>

  <!-- Arp / Sequencer settings while a mode is on (compact; full view in the rail) -->
  {#if p.pattern.mode !== 'off'}
    <div class="pattern" role="group" aria-label={t('pattern.title')}>
      <PatternPanels compact />
    </div>
  {/if}

  <!-- Custom controls -->
  <section class="card custom" aria-labelledby="custom-title">
    <div class="card-head">
      <h2 class="card-title" id="custom-title">{t('common.customControls')}</h2>
      {#if controls.length}
        <button type="button" class="btn btn-ghost edit" onclick={() => app.setView('osc')}>{t('dashboard.edit')}</button>
      {/if}
    </div>

    {#if controls.length}
      <ul class="controls">
        {#each controls as c, i (i)}
          {@const v = s.custom_values[i] ?? c.default}
          <li class="control">
            {#if c.type === 'bool'}
              <Switch
                label={c.name || t('common.unnamedControl')}
                hint={c.addr}
                badge={c.midi ? triggerLabel(c.midi) : undefined}
                badgePrefix={t('midi.badgePrefix')}
                checked={v >= 0.5}
                onchange={(on) => app.engine({ type: 'custom_osc', index: i, value: on ? 1 : 0 })}
              />
            {:else}
              {@const min = c.min ?? 0}
              {@const max = c.max ?? 1}
              <div class="float">
                <div class="float-head">
                  <span class="float-name">
                    <label for="custom-{i}">{c.name || t('common.unnamedControl')}</label>
                    {#if c.midi}
                      <span class="badge" id="custom-{i}-trigger">
                        <span class="visually-hidden">{`${t('midi.badgePrefix')} `}</span>{triggerLabel(c.midi)}
                      </span>
                    {/if}
                  </span>
                  <span class="num readout">{shortNumber(v)}</span>
                </div>
                <Slider
                  id="custom-{i}"
                  aria-describedby={c.midi ? `custom-${i}-trigger` : undefined}
                  value={v}
                  {min}
                  max={max > min ? max : min + 1}
                  step={0.01}
                  valueText={shortNumber(v)}
                  oninput={(value) => app.engineCoalesced(`custom-${i}`, { type: 'custom_osc', index: i, value })}
                />
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    {:else}
      <div class="empty">
        <p>{t('dashboard.customEmpty')}</p>
        <button type="button" class="btn" onclick={() => app.setView('osc')}>{t('dashboard.openOsc')}</button>
      </div>
    {/if}
  </section>
</div>

<style>
  .dash {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--sp-4);
    padding: var(--sp-4);
  }

  @container main (min-width: 600px) {
    .dash {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .lead,
    .pattern,
    .custom {
      grid-column: 1 / -1;
    }
  }

  @container main (min-width: 840px) {
    .dash {
      grid-template-columns: minmax(0, 1.05fr) minmax(0, 1fr) minmax(0, 1fr);
    }
    .lead {
      grid-column: auto;
    }
  }

  /* Full width from 840 px too, below the top row. */
  .pattern {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    grid-column: 1 / -1;
    min-inline-size: 0;
  }

  /* --- Hero -------------------------------------------------------------- */

  /* Transpose card and the Arp / Sequencer strip share the first column. */
  .lead {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-inline-size: 0;
  }

  .lead > .hero {
    flex: 1;
  }

  .hero {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-inline-size: 0;
    padding: var(--sp-3) var(--sp-5);
    border-radius: var(--r-lg);
    background: linear-gradient(145deg, var(--accent) 0%, var(--accent-strong) 100%);
    color: var(--on-accent);
    box-shadow: var(--shadow-1);
    overflow: hidden;
    --focus: var(--on-accent);
  }

  /* Decoration only: two rings in the corner, like a wallet card. */
  .rings {
    position: absolute;
    inset-block-start: -70px;
    inset-inline-end: -60px;
    inline-size: 200px;
    block-size: 200px;
    border: 1px solid var(--on-accent-soft);
    border-radius: 50%;
    pointer-events: none;
  }

  .rings::after {
    content: '';
    position: absolute;
    inset: 36px;
    border: 1px solid var(--on-accent-soft);
    border-radius: 50%;
  }

  .hero-head {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-block-size: 24px;
  }

  .hero h2 {
    font-size: var(--fs-sm);
    font-weight: 500;
    letter-spacing: 0.01em;
  }

  .unit {
    padding: 3px 10px;
    border-radius: var(--r-pill);
    background: var(--on-accent-soft);
    font-size: var(--fs-xs);
    font-weight: 500;
  }

  .hero-main {
    position: relative;
    display: flex;
    flex: 1;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2);
  }

  .value {
    min-inline-size: 3ch;
    font-size: var(--fs-hero);
    font-weight: 650;
    line-height: 1;
    letter-spacing: -0.02em;
    text-align: center;
  }

  .round {
    display: grid;
    flex: none;
    place-items: center;
    inline-size: 36px;
    block-size: 36px;
    padding: 0;
    border: 1px solid var(--on-accent-line);
    border-radius: 50%;
    background: var(--on-accent-soft);
    color: var(--on-accent);
    cursor: pointer;
    transition: background-color var(--dur) var(--ease);
  }

  .round:hover,
  .reset:hover {
    background: var(--on-accent-soft-hover);
  }

  .round:active,
  .reset:active {
    background: var(--on-accent-line);
  }

  .round[aria-disabled='true'],
  .reset[aria-disabled='true'] {
    opacity: 0.45;
    cursor: default;
  }

  .round[aria-disabled='true']:hover,
  .reset[aria-disabled='true']:hover {
    background: var(--on-accent-soft);
  }

  .hero-range {
    position: relative;
    padding-block-end: 6px;
  }

  .hero-range :global(.range) {
    --slider-track-size: 4px;
    --slider-thumb-size: 20px;
    --slider-track-color: var(--on-accent-track);
    --slider-fill-color: var(--on-accent-track);
    --slider-thumb-color: var(--on-accent);
    --slider-thumb-ring: var(--on-accent-ink);
    display: block;
  }

  /* Marks 0 under the track. The thumb travels the width minus its own size. */
  .zero {
    position: absolute;
    inset-block-end: 0;
    inset-inline-start: calc(10px + var(--p) * (100% - 20px) - 1px);
    inline-size: 2px;
    block-size: 6px;
    border-radius: 1px;
    background: var(--on-accent-track);
  }

  .hero-foot {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: var(--fs-sm);
    font-weight: 500;
  }

  .reset {
    min-block-size: 28px;
    padding: 0 var(--sp-3);
    border: 1px solid var(--on-accent-line);
    border-radius: var(--r-pill);
    background: var(--on-accent-soft);
    color: var(--on-accent);
    font: inherit;
    cursor: pointer;
    transition: background-color var(--dur) var(--ease);
  }

  /* --- MIDI and status --------------------------------------------------- */

  .state {
    display: flex;
    align-items: baseline;
    gap: var(--sp-2);
    min-block-size: 1.45em;
    color: var(--text-dim);
    font-size: var(--fs-sm);
  }

  .state .dot,
  .states .dot {
    position: relative;
    inset-block-start: -1px;
    align-self: center;
  }

  .states {
    display: grid;
    /* A long label (the arpeggiator row in some languages) wraps instead of squeezing the values. */
    grid-template-columns: fit-content(40%) minmax(0, 1fr);
    gap: var(--sp-2) var(--sp-4);
    font-size: var(--fs-sm);
  }

  .states dt {
    color: var(--text-dim);
  }

  .states dd {
    display: flex;
    align-items: baseline;
    gap: var(--sp-2);
    min-inline-size: 0;
    overflow-wrap: anywhere;
  }

  .states dd .dot {
    align-self: flex-start;
    inset-block-start: 6px;
  }


  /* --- Custom controls --------------------------------------------------- */

  .edit {
    min-block-size: 28px;
    font-size: var(--fs-sm);
  }

  .controls {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: var(--sp-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .control {
    display: flex;
    flex-direction: column;
    justify-content: center;
    min-block-size: 64px;
    min-inline-size: 0;
    padding: var(--sp-2) var(--sp-3);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
  }

  .control :global(.hint) {
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .float-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--sp-3);
  }

  .float-name {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px var(--sp-2);
    min-inline-size: 0;
  }

  .readout {
    min-inline-size: 4ch;
    color: var(--text-dim);
    font-size: var(--fs-sm);
    text-align: end;
  }

  .float :global(.range) {
    display: block;
    block-size: 22px;
  }
</style>
