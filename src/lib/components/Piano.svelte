<script lang="ts" module>
  const FIRST = 21; // A0
  const LAST = 108; // C8
  const BLACK = new Set([1, 3, 6, 8, 10]);

  type Key = { note: number; label: string; left: number };

  const whites: Key[] = [];
  const blacks: Key[] = [];
  for (let note = FIRST; note <= LAST; note++) {
    if (BLACK.has(note % 12)) blacks.push({ note, label: '', left: whites.length });
    else whites.push({ note, label: note % 12 === 0 ? `C${note / 12 - 1}` : '', left: 0 });
  }
  const WHITE_COUNT = whites.length; // 52
</script>

<script lang="ts">
  // 88 keys, A0 to C8. Pressed keys are painted by diffing the key bitmap from
  // the engine against the last one and toggling a class on only the keys that
  // changed; the velocity of each pressed key sets how strong its blue is.
  // Nothing here re-renders per note.
  import { onMount } from 'svelte';
  import { app } from '../app.svelte';
  import { t } from '../i18n/i18n.svelte';
  import Segmented from './Segmented.svelte';
  import Wheels from './Wheels.svelte';

  let root = $state<HTMLDivElement>();
  let lowEdge = $state<HTMLSpanElement>();
  let highEdge = $state<HTMLSpanElement>();

  const els: (HTMLElement | undefined)[] = new Array(128);
  const last = new Uint32Array(4);
  /** Velocity last written to each key, so unchanged keys are not touched. */
  const lastVel = new Uint8Array(128);
  let lowOn = false;
  let highOn = false;

  function popcount(n: number): number {
    n = n - ((n >>> 1) & 0x55555555);
    n = (n & 0x33333333) + ((n >>> 2) & 0x33333333);
    return (((n + (n >>> 4)) & 0x0f0f0f0f) * 0x01010101) >>> 24;
  }

  function paint(keys: readonly number[], vel: readonly number[] = []) {
    const offset = app.ui.piano_source === 'output' ? 4 : 0;
    // `vel` lists the velocities of all set bits, input words first.
    let vi = 0;
    if (offset) for (let w = 0; w < 4; w++) vi += popcount((keys[w] ?? 0) >>> 0);

    for (let w = 0; w < 4; w++) {
      const cur = (keys[offset + w] ?? 0) >>> 0;
      let diff = (cur ^ last[w]) >>> 0;
      while (diff) {
        const bit = 31 - Math.clz32(diff & -diff);
        diff = (diff & ~(1 << bit)) >>> 0;
        const on = ((cur >>> bit) & 1) === 1;
        els[w * 32 + bit]?.classList.toggle('on', on);
        if (!on) lastVel[w * 32 + bit] = 0;
      }
      // Velocity of every pressed key in this word (also re-strikes of held keys).
      let bits = cur;
      while (bits) {
        const bit = 31 - Math.clz32(bits & -bits);
        bits = (bits & ~(1 << bit)) >>> 0;
        const note = w * 32 + bit;
        const v = vel[vi++] ?? 100;
        if (v !== lastVel[note]) {
          lastVel[note] = v;
          const el = els[note];
          if (el) {
            el.style.setProperty('--v', (v / 127).toFixed(2));
            el.classList.toggle('hard', v >= 72);
          }
        }
      }
      last[w] = cur;
    }
    // Notes below A0 (0-20) and above C8 (109-127) light the edge markers.
    const low = (last[0] & 0x1fffff) !== 0;
    const high = last[3] >>> 13 !== 0;
    if (low !== lowOn) lowEdge?.classList.toggle('on', (lowOn = low));
    if (high !== highOn) highEdge?.classList.toggle('on', (highOn = high));
  }

  onMount(() => {
    for (const el of root?.querySelectorAll<HTMLElement>('[data-note]') ?? []) {
      els[Number(el.dataset.note)] = el;
    }
    last.fill(0);
    lastVel.fill(0);
    paint(app.keys, app.vel);
    return app.onKeys(paint);
  });

  // Switching between input and output repaints from the other half of the bitmap.
  $effect(() => {
    void app.ui.piano_source;
    lastVel.fill(0);
    paint(app.keys, app.vel);
  });
</script>

<section class="strip" aria-label={t('piano.keyboard')}>
  <div class="bar">
    <span class="title">{t('piano.keyboard')}</span>
    <Segmented
      small
      label={t('piano.shownNotes')}
      options={[
        { value: 'input', label: t('piano.input') },
        { value: 'output', label: t('piano.output') },
      ]}
      value={app.ui.piano_source}
      onchange={(v) => app.setPianoSource(v)}
    />
    <span class="note">
      {app.ui.piano_source === 'input' ? t('piano.asPlayed') : t('piano.afterTranspose')}
    </span>
    <button
      type="button"
      class="btn btn-ghost panic"
      title={t('piano.panicTip')}
      onclick={() => app.engine({ type: 'panic' })}
    >
      {t('piano.panic')}
    </button>
  </div>

  <div class="piano">
    <Wheels />
    <div class="board" role="img" aria-label={t('piano.board')}>
    <span class="edge" bind:this={lowEdge}>‹</span>
    <div class="keys motion-safe" bind:this={root}>
      {#each whites as k (k.note)}
        <span class="w" data-note={k.note}>{k.label}</span>
      {/each}
      {#each blacks as k (k.note)}
        <span class="b" data-note={k.note} style:--i={k.left}></span>
      {/each}
    </div>
    <span class="edge" bind:this={highEdge}>›</span>
    </div>
  </div>
</section>

<style>
  .strip {
    contain: layout style paint;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding: var(--sp-2) var(--sp-4) var(--sp-3);
    background: var(--surface-1);
    border-block-start: 1px solid var(--line);
  }

  .bar {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-block-size: 28px;
  }

  .title {
    color: var(--text-dim);
    font-size: var(--fs-sm);
    font-weight: 500;
  }

  .note {
    color: var(--text-dim);
    font-size: var(--fs-sm);
  }

  .panic {
    min-block-size: 28px;
    margin-inline-start: auto;
  }

  .piano {
    display: flex;
    align-items: stretch;
    gap: var(--sp-3);
    block-size: 68px;
  }

  .board {
    display: flex;
    flex: 1;
    align-items: stretch;
    gap: var(--sp-1);
    min-inline-size: 0;
  }

  .edge {
    display: grid;
    place-items: center;
    inline-size: 10px;
    border-radius: 3px;
    background: var(--surface-3);
    color: transparent;
    font-size: var(--fs-sm);
    font-weight: 700;
    line-height: 1;
  }

  /* :global: the class is toggled from script, not from the template. */
  .edge:global(.on) {
    background: var(--accent);
    color: var(--on-accent);
  }

  .keys {
    --white-w: calc(100% / 52);
    position: relative;
    display: flex;
    flex: 1;
    min-inline-size: 0;
    border-radius: 0 0 4px 4px;
    overflow: hidden;
  }

  .w {
    display: flex;
    flex: 1;
    align-items: flex-end;
    justify-content: center;
    min-inline-size: 0;
    padding-block-end: 3px;
    border-inline-end: 1px solid var(--key-white-line);
    border-radius: 0 0 3px 3px;
    background: var(--key-white);
    color: var(--key-black);
    font-size: 9px;
    font-weight: 600;
    line-height: 1;
    overflow: hidden;
  }

  .b {
    position: absolute;
    inset-block-start: 0;
    inset-inline-start: calc(var(--i) * var(--white-w) - var(--white-w) * 0.3);
    inline-size: calc(var(--white-w) * 0.6);
    block-size: 60%;
    border-radius: 0 0 2px 2px;
    background: var(--key-black);
  }

  /*
   * Pressed keys: a blue layer whose strength follows the velocity (--v, 0-1).
   * Only opacity changes, so pressing keys never causes layout work. The layer
   * fades in fast and out a little slower, which reads as smooth without lag.
   */
  .w,
  .b {
    --v: 1;
    position: relative;
  }

  .b {
    position: absolute;
  }

  .w::after,
  .b::after {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: var(--accent);
    opacity: 0;
    pointer-events: none;
    transition: opacity 200ms ease-out;
  }

  .b::after {
    background: linear-gradient(to bottom, var(--accent-strong), var(--accent));
    box-shadow: inset 0 0 0 1px var(--accent-text);
  }

  .w:global(.on)::after,
  .b:global(.on)::after {
    opacity: calc(0.28 + 0.72 * var(--v));
    transition-duration: 45ms;
  }

  /* The octave label sits above the blue layer and turns white on strong presses. */
  .w {
    isolation: isolate;
  }

  .w::after {
    z-index: -1;
  }

  .w:global(.on.hard) {
    color: var(--on-accent);
  }

  @media (forced-colors: active) {
    .w,
    .b {
      forced-color-adjust: none;
    }
    .w:global(.on),
    .b:global(.on) {
      background: Highlight;
    }
    .w::after,
    .b::after {
      display: none;
    }
  }
</style>
