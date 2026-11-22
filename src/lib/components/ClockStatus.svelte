<script lang="ts">
  // "● MIDI input · 120.0 BPM ○●○○": which clock plays, its tempo and the beat.
  // The source changes rarely and is reactive; the BPM and the beat are
  // patched from the transport stream.
  import { onMount, untrack } from 'svelte';
  import { app } from '../app.svelte';
  import { bpmNumber } from '../format';
  import { i18n, t } from '../i18n/i18n.svelte';
  import type { Transport } from '../ipc';
  import BeatDots from './BeatDots.svelte';

  let { id }: { id?: string } = $props();

  const clock = $derived(app.play.clock);
  const source = $derived(
    clock === 'internal'
      ? t('pattern.sourceInternal')
      : app.project.pattern.clock.source === 'device'
        ? t('pattern.sourceDevice')
        : t('pattern.sourceInput'),
  );

  let bpmEl = $state<HTMLSpanElement>();
  let shownBpm = NaN;

  function paintBpm(tr: Readonly<Transport>) {
    if (!bpmEl || tr.bpm === shownBpm) return;
    shownBpm = tr.bpm;
    bpmEl.textContent = t('pattern.bpmValue', { bpm: bpmNumber(tr.bpm) });
  }

  // Again when the element appears or the language changes.
  $effect(() => {
    void i18n.locale;
    if (!bpmEl) return;
    untrack(() => {
      shownBpm = NaN;
      paintBpm(app.transport);
    });
  });

  onMount(() => app.onTransport(paintBpm));
</script>

<p class="clock-status" {id}>
  {#if clock === 'none'}
    <span class="dot warn"></span>
    <span class="text">{t('pattern.noClock')}</span>
  {:else}
    <span class="dot ok"></span>
    <span class="text">{source}</span>
    <span class="sep" aria-hidden="true">·</span>
    <span class="num bpm" bind:this={bpmEl}></span>
    <BeatDots />
  {/if}
</p>

<style>
  .clock-status {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-1) var(--sp-2);
    min-block-size: 1.45em;
    color: var(--text-dim);
    font-size: var(--fs-sm);
  }

  .text {
    color: var(--text);
  }

  .bpm {
    /* "120.0 BPM" and "92.5 BPM" take the same room, so the dots do not move. */
    min-inline-size: 8.5ch;
    color: var(--text);
  }
</style>
