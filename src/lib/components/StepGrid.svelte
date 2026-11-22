<script lang="ts">
  // The sequencer steps, in rows of 16. Each step is a column of native
  // controls (a toggle button, a number input, two range inputs, a toggle).
  // Kept cheap:
  // - every control reads its own step's fields from the project proxy, so an
  //   edit re-renders only the bindings of that step;
  // - the playhead is a class moved from one column to the next from the
  //   transport stream; nothing re-renders per step.
  import { onMount } from 'svelte';
  import { app } from '../app.svelte';
  import { noteName, percent } from '../format';
  import { formatNumber, t } from '../i18n/i18n.svelte';
  import { MAX_STEPS, type Transport } from '../ipc';
  import { OFFSET_MAX } from '../pattern';
  import Icon from './Icon.svelte';

  const ROW = 16;
  const uid = $props.id();

  const seq = $derived(app.project.pattern.seq);
  const length = $derived(Math.max(1, Math.min(MAX_STEPS, Math.round(seq.length) || 1)));
  const blocks = $derived(Array.from({ length: Math.ceil(length / ROW) }, (_, b) => b));

  // Columns by step index, for the playhead.
  const cols: (HTMLElement | undefined)[] = new Array(MAX_STEPS);
  let lit = -1;

  function paint(tr: Readonly<Transport>) {
    // The playhead, or while step recording the step the next key fills.
    const step = tr.playing || tr.rec === 'step' ? tr.step : -1;
    if (step === lit) return;
    cols[lit]?.classList.remove('now');
    cols[step]?.classList.add('now');
    lit = step;
  }

  onMount(() => {
    paint(app.transport);
    return app.onTransport(paint);
  });

  function offsetInput(i: number, el: HTMLInputElement) {
    // Only valid values reach the project; an invalid one stays in the field
    // and is flagged once the user leaves it (:user-invalid).
    if (el.value === '' || !el.validity.valid) return;
    seq.steps[i].offset = el.valueAsNumber + 0;
    app.touch();
  }

  function stepNote(offset: number): string {
    const n = seq.root + offset;
    return n >= 0 && n <= 127 ? noteName(n) : '–';
  }
</script>

{#snippet column(i: number)}
  {@const st = seq.steps[i]}
  {@const n = i + 1}
  <!-- Its column in the row: after the labels, skipping the narrow gap column after each beat. -->
  <div
    class={['step', i % 4 === 0 && 'beat', !st.on && 'off']}
    style:grid-column={2 + (i % ROW) + Math.floor((i % ROW) / 4)}
    bind:this={cols[i]}
  >
    <span class="h-idx idx num" aria-hidden="true">{formatNumber(n)}</span>
    <button
      type="button"
      class="h-pad pad"
      aria-pressed={st.on}
      aria-label={t('pattern.stepOn', { n })}
      onclick={() => {
        st.on = !st.on;
        app.touch(true);
      }}
    ></button>
    <input
      class="h-offset offset num"
      type="number"
      inputmode="numeric"
      min={-OFFSET_MAX}
      max={OFFSET_MAX}
      step="1"
      required
      value={st.offset}
      aria-label={t('pattern.stepOffset', { n })}
      aria-invalid="false"
      data-error="{uid}-err"
      oninput={(e) => offsetInput(i, e.currentTarget)}
      onchange={() => void app.flush()}
    />
    <span class="h-note note num" aria-hidden="true">{stepNote(st.offset)}</span>
    <input
      class="h-bar range bar"
      type="range"
      min="1"
      max="127"
      value={st.velocity}
      style:--control-value={st.velocity}
      style:--attr-min="1"
      style:--attr-max="127"
      aria-label={t('pattern.stepVelocity', { n })}
      oninput={(e) => {
        st.velocity = e.currentTarget.valueAsNumber;
        app.touch();
      }}
    />
    <input
      class="h-bar range bar"
      type="range"
      min="5"
      max="100"
      value={st.gate}
      style:--control-value={st.gate}
      style:--attr-min="5"
      style:--attr-max="100"
      aria-label={t('pattern.stepGate', { n })}
      aria-valuetext={percent(st.gate)}
      oninput={(e) => {
        st.gate = e.currentTarget.valueAsNumber;
        app.touch();
      }}
    />
    <button
      type="button"
      class="h-tie tie"
      aria-pressed={st.tie}
      aria-label={t('pattern.stepTie', { n })}
      onclick={() => {
        st.tie = !st.tie;
        app.touch(true);
      }}
    >
      <Icon name="tie" size={16} />
    </button>
  </div>
{/snippet}

<div class="grid">
  {#each blocks as b (b)}
    {@const from = b * ROW}
    {@const to = Math.min(length, from + ROW)}
    <div
      class="block scroller"
      role="group"
      aria-label={t('pattern.stepsRange', { from: formatNumber(from + 1), to: formatNumber(to) })}
    >
      <div class="labels" aria-hidden="true">
        <span class="h-idx">{t('pattern.rowStep')}</span>
        <span class="h-pad">{t('pattern.rowOn')}</span>
        <span class="h-offset">{t('pattern.rowOffset')}</span>
        <span class="h-note">{t('pattern.rowNote')}</span>
        <span class="h-bar">{t('pattern.rowVelocity')}</span>
        <span class="h-bar">{t('pattern.rowGate')}</span>
        <span class="h-tie">{t('pattern.rowTie')}</span>
      </div>
      {#each Array.from({ length: to - from }, (_, k) => from + k) as i (i)}
        {@render column(i)}
      {/each}
    </div>
  {/each}
</div>
<div class="error-msg grid-error" id="{uid}-err">
  <span role="img" aria-label={t('common.error')}><Icon name="alert" size={14} /></span>
  <span>{t('pattern.offsetRange')}</span>
</div>

<style>
  .grid {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-inline-size: 0;
  }

  /* One row of up to 16 steps. Scrolls sideways when the card is too narrow. */
  .block {
    --h-idx: 18px;
    --h-pad: 30px;
    --h-offset: 28px;
    --h-note: 16px;
    --h-bar: 20px;
    --h-tie: 26px;
    display: grid;
    /* Labels, then four beats of four steps with a narrow gap column after each, so every row lines up. */
    --col: minmax(40px, 1fr);
    grid-template-columns: max-content repeat(4, var(--col) var(--col) var(--col) var(--col) 3px);
    column-gap: 3px;
    padding-block-end: var(--sp-1);
    overflow-x: auto;
    overflow-y: hidden;
  }

  .labels,
  .step {
    display: flex;
    flex-direction: column;
    gap: 4px;
    grid-row: 1;
  }

  .labels {
    grid-column: 1;
    position: sticky;
    inset-inline-start: 0;
    z-index: 1;
    padding-inline-end: var(--sp-2);
    background: var(--surface-2);
    color: var(--text-dim);
    font-size: var(--fs-xs);
    font-weight: 500;
  }

  .labels > span {
    display: flex;
    align-items: center;
    white-space: nowrap;
  }

  .step {
    min-inline-size: 0;
    padding: 2px;
    border: 1px solid transparent;
    border-radius: 6px;
  }

  .h-idx {
    block-size: var(--h-idx);
  }
  .h-pad {
    block-size: var(--h-pad);
  }
  .h-offset {
    block-size: var(--h-offset);
  }
  .h-note {
    block-size: var(--h-note);
  }
  .h-bar {
    block-size: var(--h-bar);
  }
  .h-tie {
    block-size: var(--h-tie);
  }

  .step .idx {
    color: var(--text-faint);
    font-size: var(--fs-xs);
    line-height: var(--h-idx);
    text-align: center;
  }

  .step.beat .idx {
    color: var(--text-dim);
    font-weight: 600;
  }

  /* Playhead: a frame and a tint around the column, and a bold step number. */
  .step:global(.now) {
    border-color: var(--accent-text);
    background: var(--accent-soft);
  }

  .step:global(.now) .idx {
    color: var(--text);
    font-weight: 700;
  }

  .pad,
  .tie {
    display: grid;
    place-items: center;
    inline-size: 100%;
    padding: 0;
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    background: var(--surface-3);
    color: var(--text-dim);
    cursor: pointer;
  }

  .pad:hover,
  .tie:hover {
    background: var(--surface-3-hover);
  }

  /* On: filled, with an inner mark, so it does not depend on colour. */
  .pad[aria-pressed='true'] {
    border-color: var(--accent);
    background: var(--accent);
  }

  .pad[aria-pressed='true']::after {
    content: '';
    inline-size: 8px;
    block-size: 8px;
    border-radius: 2px;
    background: var(--on-accent);
  }

  .tie[aria-pressed='true'] {
    border-color: var(--accent-text);
    background: var(--accent-soft);
    color: var(--text);
  }

  .tie[aria-pressed='false'] :global(svg) {
    opacity: 0.55;
  }

  .offset {
    inline-size: 100%;
    min-inline-size: 0;
    padding: 0 2px;
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    background: var(--surface-3);
    color: var(--text);
    font: inherit;
    font-size: var(--fs-sm);
    text-align: center;
    appearance: textfield;
    -moz-appearance: textfield;
  }

  .offset::-webkit-inner-spin-button,
  .offset::-webkit-outer-spin-button {
    -webkit-appearance: none;
    appearance: none;
    margin: 0;
  }

  .offset:hover {
    border-color: var(--text-dim);
  }

  /* Two rules: an engine without :user-invalid drops a whole selector list (validation.ts adds the class there). */
  .offset:user-invalid {
    border-color: var(--err);
    background: var(--err-soft);
  }

  .offset:global(.user-invalid-fallback) {
    border-color: var(--err);
    background: var(--err-soft);
  }

  .grid:has(.offset:user-invalid) + .grid-error {
    display: flex;
  }

  .grid:has(.offset:global(.user-invalid-fallback)) + .grid-error {
    display: flex;
  }

  .step .note {
    color: var(--text-dim);
    font-size: var(--fs-xs);
    line-height: var(--h-note);
    text-align: center;
    white-space: nowrap;
  }

  /* Velocity and gate read as bars: a thick track filled to the value, with a slim handle. */
  input[type='range'].bar {
    --slider-track-size: 10px;
    --slider-thumb-size: 16px;
    --slider-thumb-color: var(--text);
    inline-size: 100%;
    min-inline-size: 0;
  }

  input[type='range'].bar::-webkit-slider-runnable-track {
    border-radius: 3px;
  }

  input[type='range'].bar::-webkit-slider-thumb {
    inline-size: 4px;
    border: 0;
    border-radius: 2px;
  }

  input[type='range'].bar::-moz-range-track {
    border-radius: 3px;
  }

  input[type='range'].bar::-moz-range-progress {
    border-radius: 3px;
  }

  input[type='range'].bar::-moz-range-thumb {
    inline-size: 4px;
    border: 0;
    border-radius: 2px;
  }

  /* A step that is off keeps its values, shown dimmed. */
  .step.off :is(.offset, .note, .bar) {
    opacity: 0.5;
  }

  @media (forced-colors: active) {
    .pad[aria-pressed='true'],
    .tie[aria-pressed='true'] {
      forced-color-adjust: none;
      background: Highlight;
      color: HighlightText;
    }
    .step:global(.now) {
      border-color: Highlight;
    }
  }
</style>
