<script lang="ts">
  // Clock, arpeggiator and sequencer settings and the step grid. Shown in the
  // Arp / Sequencer view and, compact (no hints, grid folded), on the
  // dashboard. Everything here is a project edit through app.touch(); only
  // Play / Stop is a session command.
  import { app } from '../app.svelte';
  import { noteName, percent } from '../format';
  import { formatNumber, t } from '../i18n/i18n.svelte';
  import { ARP_STYLES, RATES, type PatternMode, type Rate } from '../ipc';
  import { float, integer } from '../parsers';
  import { BPM_MAX, BPM_MIN, clearSteps, randomizeSteps, rateLabel, shiftSteps } from '../pattern';
  import { NONE, SAVED, midiLine, portFromOption, portSelection } from '../ports';
  import ClockStatus from '../components/ClockStatus.svelte';
  import Field from '../components/Field.svelte';
  import Icon from '../components/Icon.svelte';
  import PlayButton from '../components/PlayButton.svelte';
  import Segmented from '../components/Segmented.svelte';
  import Slider from '../components/Slider.svelte';
  import StepGrid from '../components/StepGrid.svelte';
  import Switch from '../components/Switch.svelte';

  let { compact = false }: { compact?: boolean } = $props();
  let stepsOpen = $state(false);

  const s = $derived(app.snapshot);
  const pat = $derived(app.project.pattern);

  /** Typing: send after a pause. */
  const touch = () => app.touch();
  /** Selects, switches, buttons: send now. */
  const now = () => app.touch(true);
  /** Leaving a field: send what is pending. */
  const done = () => void app.flush();

  const CHANNELS = Array.from({ length: 16 }, (_, i) => i + 1);

  const parseBpm = float((n) => (n < BPM_MIN || n > BPM_MAX ? t('pattern.bpmRange') : null));
  const parseLength = integer((n) => (n < 1 || n > 64 ? t('pattern.lengthRange') : null));
  const parseRoot = integer((n) => (n < 0 || n > 127 ? t('midi.numberRange') : null));
  // --- Clock -----------------------------------------------------------------
  const fromDevice = $derived(pat.clock.source === 'device');
  const devSel = $derived(portSelection(pat.clock.device, s.clock_device, s.inputs));
  const devLine = $derived(midiLine(s.clock_device));

  function chooseDevice(value: string) {
    const port = portFromOption(value, s.inputs);
    if (port === undefined) return;
    pat.clock.device = port;
    now();
  }
</script>

{#snippet rateSelect(id: string, value: Rate, set: (r: Rate) => void)}
  <div class="field">
    <label class="label" for={id}>{t('pattern.rate')}</label>
    <select
      class="select"
      {id}
      {value}
      onchange={(e) => {
        set(e.currentTarget.value as Rate);
        now();
      }}
    >
      {#each RATES as r (r)}
        <option value={r}>{rateLabel(r)}</option>
      {/each}
    </select>
  </div>
{/snippet}

{#snippet range(
  id: string,
  label: string,
  hint: string,
  value: number,
  min: number,
  max: number,
  text: string,
  set: (v: number) => void,
)}
  <div class="range-field">
    <div class="range-head">
      <label class="label" for={id}>{label}</label>
      <span class="num readout" aria-hidden="true">{text}</span>
    </div>
    {#if hint}<span class="hint" id="{id}-hint">{hint}</span>{/if}
    <Slider
      {id}
      {value}
      {min}
      {max}
      valueText={text}
      aria-describedby={hint ? `${id}-hint` : undefined}
      oninput={(v) => {
        set(v);
        touch();
      }}
    />
  </div>
{/snippet}

  <div class={["cols", compact && "compact"]}>
    <!-- Clock -->
    <section class="card" aria-labelledby="pat-clock-title">
      <div class="card-head">
        <h2 class="card-title" id="pat-clock-title">{t('pattern.clock')}</h2>
      </div>

      <Switch
        label={t('pattern.fromDevice')}
        hint={compact ? undefined : t('pattern.fromDeviceHint')}
        checked={fromDevice}
        onchange={(on) => {
          pat.clock.source = on ? 'device' : 'input';
          now();
        }}
      />

      {#if fromDevice}
        <div class="field">
          <label class="label" for="pat-clock-device">{t('pattern.clockDevice')}</label>
          <select
            class="select"
            id="pat-clock-device"
            value={devSel.value}
            aria-describedby="pat-clock-device-state"
            onchange={(e) => chooseDevice(e.currentTarget.value)}
          >
            <option value={NONE}>{t('common.none')}</option>
            {#each s.inputs as port (port.id)}
              <option value={port.id}>{port.name}</option>
            {/each}
            {#if devSel.extra}
              <option value={SAVED}>{t('dashboard.notConnected', { name: devSel.extra.name || t('dashboard.savedPort') })}</option>
            {/if}
          </select>
          <p class="state" id="pat-clock-device-state">
            <span class={['dot', devLine.tone]}></span>
            <span>{devLine.text}</span>
          </p>
        </div>
      {/if}

      <hr class="divider" />

      <Switch
        label={t('pattern.fallback')}
        hint={compact ? undefined : t('pattern.fallbackHint')}
        checked={pat.clock.internal_fallback}
        onchange={(on) => {
          pat.clock.internal_fallback = on;
          now();
        }}
      />
      <div class="bpm">
        <Field
          label={t('pattern.bpm')}
          numeric
          value={pat.clock.bpm}
          parse={parseBpm}
          oncommit={(v) => {
            pat.clock.bpm = v;
            touch();
          }}
          ondone={done}
        />
      </div>

      {#if !compact}
        <!-- On the dashboard the strip above already shows the beat and tempo. -->
        <hr class="divider" />

        <div class="field">
          <span class="label" id="pat-clock-status">{t('pattern.clockStatus')}</span>
          <ClockStatus />
        </div>
      {/if}
    </section>

    {#if pat.mode === 'arp'}
      <!-- Arpeggiator -->
      <section class="card" aria-labelledby="pat-arp-title">
        <div class="card-head">
          <h2 class="card-title" id="pat-arp-title">{t('pattern.arp')}</h2>
        </div>

        <div class="pair">
          <div class="field">
            <label class="label" for="pat-arp-style">{t('pattern.style')}</label>
            <select
              class="select"
              id="pat-arp-style"
              value={pat.arp.style}
              onchange={(e) => {
                pat.arp.style = e.currentTarget.value as typeof pat.arp.style;
                now();
              }}
            >
              {#each ARP_STYLES as st (st)}
                <option value={st}>{t(`pattern.styles.${st}`)}</option>
              {/each}
            </select>
          </div>
          {@render rateSelect('pat-arp-rate', pat.arp.rate, (r) => (pat.arp.rate = r))}
        </div>

        <Segmented
          showLabel
          label={t('pattern.octaves')}
          options={['1', '2', '3', '4'].map((v) => ({ value: v, label: formatNumber(Number(v)) }))}
          value={String(pat.arp.octaves)}
          onchange={(v) => {
            pat.arp.octaves = Number(v);
            now();
          }}
        />

        <div class="pair">
          {@render range('pat-arp-gate', t('pattern.gate'), compact ? '' : t('pattern.gateHint'), pat.arp.gate, 5, 100, percent(pat.arp.gate), (v) => (pat.arp.gate = v))}
          {@render range('pat-arp-swing', t('pattern.swing'), compact ? '' : t('pattern.swingHint'), pat.arp.swing, 0, 75, percent(pat.arp.swing), (v) => (pat.arp.swing = v))}
        </div>

        <hr class="divider" />

        <Switch
          label={t('pattern.latch')}
          hint={compact ? undefined : t('pattern.latchHint')}
          checked={pat.arp.latch}
          onchange={(on) => {
            pat.arp.latch = on;
            now();
          }}
        />
        <Switch
          label={t('pattern.velocityAsPlayed')}
          hint={compact ? undefined : t('pattern.velocityAsPlayedHint')}
          checked={pat.arp.velocity === null}
          onchange={(on) => {
            pat.arp.velocity = on ? null : 100;
            now();
          }}
        />
        {#if pat.arp.velocity !== null}
          {@const vel = pat.arp.velocity}
          {@render range('pat-arp-vel', t('pattern.fixedVelocity'), '', vel, 1, 127, formatNumber(vel), (v) => (pat.arp.velocity = v))}
        {/if}
      </section>
    {:else if pat.mode === 'seq'}
      <!-- Sequencer -->
      <section class="card" aria-labelledby="pat-seq-title">
        <div class="card-head">
          <h2 class="card-title" id="pat-seq-title">{t('pattern.seq')}</h2>
        </div>

        {#if !compact}<div class="play-row"><PlayButton /></div>{/if}

        <div class="pair">
          {@render rateSelect('pat-seq-rate', pat.seq.rate, (r) => (pat.seq.rate = r))}
          <div class="field">
            <label class="label" for="pat-seq-channel">{t('pattern.channel')}</label>
            {#if !compact}<span class="hint" id="pat-seq-channel-hint">{t('pattern.channelHint')}</span>{/if}
            <select
              class="select"
              id="pat-seq-channel"
              value={pat.seq.channel}
              aria-describedby={compact ? undefined : 'pat-seq-channel-hint'}
              onchange={(e) => {
                pat.seq.channel = Number(e.currentTarget.value);
                now();
              }}
            >
              {#each CHANNELS as ch (ch)}
                <option value={ch}>{t('pattern.channelN', { n: formatNumber(ch) })}</option>
              {/each}
            </select>
          </div>
        </div>

        <div class="pair">
          <Field
            label={t('pattern.length')}
            hint={compact ? undefined : t('pattern.lengthHint')}
            numeric
            value={pat.seq.length}
            parse={parseLength}
            oncommit={(v) => {
              pat.seq.length = v;
              touch();
            }}
            ondone={done}
          />
          <Field
            label={t('pattern.root')}
            hint={compact ? undefined : t('pattern.rootHint', { note: noteName(pat.seq.root) })}
            numeric
            value={pat.seq.root}
            parse={parseRoot}
            oncommit={(v) => {
              pat.seq.root = v;
              touch();
            }}
            ondone={done}
          />
        </div>

        {@render range('pat-seq-swing', t('pattern.swing'), compact ? '' : t('pattern.swingHint'), pat.seq.swing, 0, 75, percent(pat.seq.swing), (v) => (pat.seq.swing = v))}

        <hr class="divider" />

        <Switch
          label={t('pattern.follow')}
          hint={compact ? undefined : t('pattern.followHint')}
          checked={pat.seq.follow_transport}
          onchange={(on) => {
            pat.seq.follow_transport = on;
            now();
          }}
        />
      </section>
    {/if}
  </div>

  {#if pat.mode === 'seq'}
    {#if compact}
      <!-- On the dashboard the grid is folded away until wanted. -->
      <details class="card steps-fold" bind:open={stepsOpen}>
        <summary>
          <span class="card-title">{t('pattern.steps')}</span>
          <span class="count num">{formatNumber(pat.seq.length)}</span>
        </summary>
        <div class="fold-body">
          <div class="fold-actions">
          <button
            type="button"
            class="btn btn-ghost btn-icon"
            aria-label={t('pattern.shiftLeft')}
            title={t('pattern.shiftLeft')}
            onclick={() => {
              shiftSteps(pat.seq.steps, pat.seq.length, -1);
              now();
            }}
          >
            <Icon name="left" size={16} />
          </button>
          <button
            type="button"
            class="btn btn-ghost btn-icon"
            aria-label={t('pattern.shiftRight')}
            title={t('pattern.shiftRight')}
            onclick={() => {
              shiftSteps(pat.seq.steps, pat.seq.length, 1);
              now();
            }}
          >
            <Icon name="right" size={16} />
          </button>
          <button
            type="button"
            class="btn btn-ghost"
            title={t('pattern.randomizeTip')}
            onclick={() => {
              randomizeSteps(pat.seq.steps, pat.seq.length);
              now();
            }}
          >
            <Icon name="shuffle" size={16} />{t('pattern.randomize')}
          </button>
          <button
            type="button"
            class="btn btn-ghost"
            title={t('pattern.clearTip')}
            onclick={() => {
              clearSteps(pat.seq.steps, pat.seq.length);
              now();
            }}
          >
            <Icon name="eraser" size={16} />{t('pattern.clear')}
          </button>
          </div>
          <StepGrid />
        </div>
      </details>
    {:else}
    <section class="card" aria-labelledby="pat-steps-title" aria-describedby="pat-steps-hint">
      <div class="card-head">
        <h2 class="card-title" id="pat-steps-title">{t('pattern.steps')}</h2>
        <div class="card-actions">
          <button
            type="button"
            class="btn btn-ghost btn-icon"
            aria-label={t('pattern.shiftLeft')}
            title={t('pattern.shiftLeft')}
            onclick={() => {
              shiftSteps(pat.seq.steps, pat.seq.length, -1);
              now();
            }}
          >
            <Icon name="left" size={16} />
          </button>
          <button
            type="button"
            class="btn btn-ghost btn-icon"
            aria-label={t('pattern.shiftRight')}
            title={t('pattern.shiftRight')}
            onclick={() => {
              shiftSteps(pat.seq.steps, pat.seq.length, 1);
              now();
            }}
          >
            <Icon name="right" size={16} />
          </button>
          <button
            type="button"
            class="btn btn-ghost"
            title={t('pattern.randomizeTip')}
            onclick={() => {
              randomizeSteps(pat.seq.steps, pat.seq.length);
              now();
            }}
          >
            <Icon name="shuffle" size={16} />{t('pattern.randomize')}
          </button>
          <button
            type="button"
            class="btn btn-ghost"
            title={t('pattern.clearTip')}
            onclick={() => {
              clearSteps(pat.seq.steps, pat.seq.length);
              now();
            }}
          >
            <Icon name="eraser" size={16} />{t('pattern.clear')}
          </button>
        </div>
      </div>
      <p class="hint" id="pat-steps-hint">{t('pattern.stepsHint')}</p>
      <StepGrid />
    </section>
    {/if}
  {/if}

<style>
  .cols {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--sp-4);
    align-items: start;
  }

  @container main (min-width: 720px) {
    .cols {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  .pair {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
    gap: var(--sp-3);
    /* Hints sit above the controls, so aligning the bottoms lines the controls up. */
    align-items: end;
  }

  .bpm {
    max-inline-size: 180px;
  }

  .state {
    display: flex;
    align-items: baseline;
    gap: var(--sp-2);
    color: var(--text-dim);
    font-size: var(--fs-sm);
  }

  .state .dot {
    position: relative;
    inset-block-start: -1px;
    align-self: center;
  }

  .range-field {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-inline-size: 0;
  }

  .range-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--sp-2);
  }

  .readout {
    color: var(--text);
    font-size: var(--fs-sm);
  }

  .range-field :global(.range) {
    display: block;
  }

  .play-row {
    display: flex;
  }

  .play-row :global(.play) {
    flex: 1;
  }

  .card-actions {
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  /* Dashboard: two columns earlier, tighter spacing. */
  .cols.compact {
    gap: var(--sp-3);
  }

  .steps-fold {
    padding-block: var(--sp-3);
  }

  .steps-fold > summary {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    list-style: none;
    cursor: pointer;
  }

  .steps-fold > summary::-webkit-details-marker {
    display: none;
  }

  /* Chevron right before the label; points down when open. */
  .steps-fold > summary::before {
    content: '';
    inline-size: 12px;
    block-size: 12px;
    flex: none;
    background-color: currentColor;
    mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E") no-repeat center / contain;
    rotate: -90deg;
    color: var(--text-dim);
  }

  .steps-fold > summary:dir(rtl)::before {
    rotate: 90deg;
  }

  .steps-fold[open] > summary::before {
    rotate: 0deg;
  }

  .count {
    padding: 1px 8px;
    border-radius: var(--r-pill);
    background: var(--surface-3);
    color: var(--text-dim);
    font-size: var(--fs-xs);
  }

  .fold-body {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding-block-start: var(--sp-3);
  }

  .fold-actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-1);
  }
</style>
