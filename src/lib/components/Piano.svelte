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
  // changed; nothing here re-renders per note.
  import { onMount } from 'svelte';
  import { app } from '../app.svelte';
  import Segmented from './Segmented.svelte';

  let root = $state<HTMLDivElement>();
  let lowEdge = $state<HTMLSpanElement>();
  let highEdge = $state<HTMLSpanElement>();

  const els: (HTMLElement | undefined)[] = new Array(128);
  const last = new Uint32Array(4);
  let lowOn = false;
  let highOn = false;

  function paint(keys: readonly number[]) {
    const offset = app.ui.piano_source === 'output' ? 4 : 0;
    for (let w = 0; w < 4; w++) {
      const cur = (keys[offset + w] ?? 0) >>> 0;
      let diff = (cur ^ last[w]) >>> 0;
      while (diff) {
        const bit = 31 - Math.clz32(diff & -diff);
        diff = (diff & ~(1 << bit)) >>> 0;
        els[w * 32 + bit]?.classList.toggle('on', ((cur >>> bit) & 1) === 1);
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
    paint(app.keys);
    return app.onKeys(paint);
  });

  // Switching between input and output repaints from the other half of the bitmap.
  $effect(() => {
    void app.ui.piano_source;
    paint(app.keys);
  });
</script>

<section class="strip" aria-label="Keyboard">
  <div class="bar">
    <span class="title">Keyboard</span>
    <Segmented
      small
      label="Notes shown on the keyboard"
      options={[
        { value: 'input', label: 'Input' },
        { value: 'output', label: 'Output' },
      ]}
      value={app.ui.piano_source}
      onchange={(v) => app.setPianoSource(v)}
    />
    <span class="note">
      {app.ui.piano_source === 'input' ? 'Keys as played' : 'Keys after transposing'}
    </span>
    <button
      type="button"
      class="btn btn-ghost panic"
      title="Release every held note on the output and over OSC"
      onclick={() => app.engine({ type: 'panic' })}
    >
      Panic
    </button>
  </div>

  <div class="piano" role="img" aria-label="Piano with 88 keys. Pressed keys are highlighted.">
    <span class="edge" bind:this={lowEdge}>‹</span>
    <div class="keys" bind:this={root}>
      {#each whites as k (k.note)}
        <span class="w" data-note={k.note}>{k.label}</span>
      {/each}
      {#each blacks as k (k.note)}
        <span class="b" data-note={k.note} style:--i={k.left}></span>
      {/each}
    </div>
    <span class="edge" bind:this={highEdge}>›</span>
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
    gap: var(--sp-1);
    block-size: 68px;
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

  /* Instant: no transition on key colour. */
  .w:global(.on) {
    background: var(--accent);
    color: var(--on-accent);
  }

  .b:global(.on) {
    background: var(--accent-strong);
    box-shadow: inset 0 0 0 1px var(--accent-text);
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
  }
</style>
