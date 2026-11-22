<script lang="ts">
  // Sequencer recording, a toggle (aria-pressed). Stopped: step recording,
  // every key fills the next step. Playing: one loop is recorded live from
  // the start of the next loop. While step recording, Rest leaves a step
  // empty. One status line announces what recording does now.
  import { app } from '../app.svelte';
  import { t } from '../i18n/i18n.svelte';
  import Icon from './Icon.svelte';

  let { small = false }: { small?: boolean } = $props();

  const rec = $derived(app.play.rec);
  const on = $derived(rec !== 'off');

  // "Recording finished" is said once when recording ends, not on load.
  let ended = $state(false);
  let was: typeof rec = 'off';
  $effect(() => {
    if (rec === 'off' && was !== 'off') ended = true;
    else if (rec !== 'off') ended = false;
    was = rec;
  });

  const spoken = $derived(
    rec === 'step'
      ? t('pattern.recStep')
      : rec === 'waiting'
        ? t('pattern.recWaiting')
        : rec === 'live'
          ? t('pattern.recLive')
          : ended
            ? t('pattern.recDone')
            : '',
  );
</script>

<span class="rec-group">
  <button
    type="button"
    class={['btn', 'rec', on && 'on', rec === 'waiting' && 'waiting', small && 'small']}
    aria-pressed={on}
    aria-label={small ? t('pattern.record') : undefined}
    title={t('pattern.recordTip')}
    onclick={() => app.seqRecord(!on)}
  >
    <Icon name="record" size={small ? 14 : 18} />
    {#if !small}<span>{t('pattern.record')}</span>{/if}
  </button>
  {#if rec === 'step'}
    <button
      type="button"
      class={['btn', 'rest', small && 'small']}
      aria-label={small ? t('pattern.rest') : undefined}
      title={t('pattern.restTip')}
      onclick={() => app.seqRest()}
    >
      <Icon name="rest" size={small ? 14 : 18} />
      {#if !small}<span>{t('pattern.rest')}</span>{/if}
    </button>
  {/if}
</span>
<span class="visually-hidden" role="status">{spoken}</span>

<style>
  .rec-group {
    display: inline-flex;
    gap: var(--sp-1);
    min-inline-size: 0;
  }

  .btn {
    min-block-size: 44px;
    padding-inline: var(--sp-3) var(--sp-4);
    font-size: var(--fs-md);
  }

  .btn.small {
    min-block-size: 28px;
    min-inline-size: 32px;
    padding-inline: 6px;
    font-size: var(--fs-sm);
  }

  /* The record mark is red only while recording: that is its status. */
  .rec :global(svg) {
    color: var(--text-dim);
  }

  .rec.on {
    border-color: var(--err);
    background: var(--err-soft);
    color: var(--text);
  }

  .rec.on :global(svg) {
    color: var(--err);
    fill: currentColor;
  }

  /* Waiting for the loop start: a dashed frame, as Play waits for the bar. No pulsing. */
  .rec.waiting {
    border-style: dashed;
  }

  .rec.waiting :global(svg) {
    fill: none;
  }

  span:not(.rec-group) {
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
