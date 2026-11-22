<script lang="ts">
  // Arpeggiator / sequencer at a glance on the dashboard: the mode, the beat,
  // Play / Stop for the sequencer. The full view is in the rail.
  import { app } from '../app.svelte';
  import { t } from '../i18n/i18n.svelte';
  import BeatDots from './BeatDots.svelte';
  import PlayButton from './PlayButton.svelte';

  const mode = $derived(app.project.pattern.mode);
  const modeLabel = $derived(
    mode === 'arp' ? t('pattern.modeArp') : mode === 'seq' ? t('pattern.modeSeq') : t('pattern.modeOff'),
  );
  const noClock = $derived(mode !== 'off' && app.play.clock === 'none');
</script>

<div class="pstat">
  <span class="what">
    <span class={['dot', mode !== 'off' && (noClock ? 'warn' : 'ok')]}></span>
    <span>{modeLabel}</span>
    {#if mode !== 'off' && !noClock}<BeatDots />{/if}
  </span>
  <span class="grow"></span>
  {#if mode === 'seq'}<PlayButton small />{/if}
</div>
{#if noClock}<p class="hint warn-line">{t('pattern.noClock')}</p>{/if}

<style>
  .pstat {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-1) var(--sp-2);
    min-inline-size: 0;
  }

  /* The mode and its beat dots stay on one line. */
  .what {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    white-space: nowrap;
  }

  .grow {
    flex: 1;
  }

  .warn-line {
    margin-block-start: 2px;
    font-size: var(--fs-xs);
  }
</style>
