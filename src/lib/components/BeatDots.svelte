<script lang="ts">
  // Four dots, one per beat of the bar; the current beat is lit. Painted from
  // the transport stream by moving one class, never by re-rendering. The BPM
  // and clock source next to it say the same in words, so the dots are hidden
  // from screen readers.
  import { onMount } from 'svelte';
  import { app } from '../app.svelte';
  import type { Transport } from '../ipc';

  let root = $state<HTMLSpanElement>();
  let lit = -1;

  function paint(tr: Readonly<Transport>) {
    const beat = tr.clock === 'none' ? -1 : tr.beat;
    if (beat === lit || !root) return;
    root.children[lit]?.classList.remove('on');
    root.children[beat]?.classList.add('on');
    lit = beat;
  }

  onMount(() => {
    paint(app.transport);
    return app.onTransport(paint);
  });
</script>

<span class="beats" bind:this={root} aria-hidden="true"><span></span><span></span><span></span><span></span></span>

<style>
  .beats {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 5px;
  }

  .beats > span {
    inline-size: 8px;
    block-size: 8px;
    border: 1px solid var(--line-strong);
    border-radius: 50%;
  }

  /* The downbeat is larger, so the bar can be read without colour. */
  .beats > span:first-child {
    inline-size: 10px;
    block-size: 10px;
  }

  .beats > :global(span.on) {
    border-color: var(--accent-text);
    background: var(--accent-text);
  }

  @media (forced-colors: active) {
    .beats > :global(span.on) {
      forced-color-adjust: none;
      background: Highlight;
      border-color: Highlight;
    }
  }
</style>
