<script lang="ts">
  // Arpeggiator / sequencer in one line under the transpose card: the mode,
  // the beat and tempo, and Play / Stop for the sequencer. The full settings
  // are in the rail. Kept to a single row so the dashboard does not grow taller.
  // Beat and tempo are patched from the transport stream, never re-rendered.
  import { onMount } from 'svelte';
  import { app } from '../app.svelte';
  import { bpmNumber } from '../format';
  import { t } from '../i18n/i18n.svelte';
  import type { PatternMode, Transport } from '../ipc';
  import BeatDots from './BeatDots.svelte';
  import Icon from './Icon.svelte';
  import PlayButton from './PlayButton.svelte';
  import RecordButton from './RecordButton.svelte';
  import Segmented from './Segmented.svelte';

  // The mode playing now: MIDI can switch it for the session.
  const mode = $derived(app.patternMode);
  const noClock = $derived(mode !== 'off' && app.play.clock === 'none');
  // Step recording does not follow the clock; its buttons take the room of the beat.
  const stepRec = $derived(app.play.rec === 'step');

  let bpmEl = $state<HTMLSpanElement>();
  let shownBpm = -1;

  function paint(tr: Readonly<Transport>) {
    const bpm = tr.clock === 'none' ? 0 : Math.round(tr.bpm * 10) / 10;
    if (bpm === shownBpm || !bpmEl) return;
    shownBpm = bpm;
    bpmEl.textContent = bpm ? t('pattern.bpmValue', { bpm: bpmNumber(bpm) }) : '';
  }

  onMount(() => {
    paint(app.transport);
    return app.onTransport(paint);
  });

  // The language can change while the tempo stays the same.
  $effect(() => {
    void t('pattern.bpmValue', { bpm: '' });
    shownBpm = -1;
    paint(app.transport);
  });

  function setMode(value: string) {
    app.setPatternMode(value as PatternMode);
  }
</script>

<section class="strip" aria-labelledby="pattern-strip-title">
  <!-- The mode legend and the settings link name the strip visibly enough; the heading is for screen readers. -->
  <h2 class="visually-hidden" id="pattern-strip-title">{t('pattern.short')}</h2>

  <Segmented
    small
    label={t('pattern.short')}
    options={[
      { value: 'off', label: t('pattern.modeOff') },
      { value: 'arp', label: t('pattern.modeArpShort') },
      { value: 'seq', label: t('pattern.modeSeqShort') },
    ]}
    value={mode}
    onchange={setMode}
  />

  {#if mode !== 'off' && !stepRec}
    {#if noClock}
      <span class="clock warn" title={t('pattern.noClock')}>
        <Icon name="alert" size={14} />
        <span>{t('pattern.noClockShort')}</span>
      </span>
    {:else}
      <span class="clock">
        <BeatDots />
        <span class="bpm num" bind:this={bpmEl}></span>
      </span>
    {/if}
  {/if}

  <span class="end">
    {#if mode === 'seq'}<RecordButton small /><PlayButton small />{/if}
  </span>
</section>

<style>
  .strip {
    container: pattern-strip / inline-size;
    display: flex;
    flex-wrap: wrap;
    align-items: safe center;
    gap: var(--sp-2);
    min-inline-size: 0;
    padding: 6px var(--sp-2);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface-2);
    box-shadow: var(--shadow-1);
  }

  .clock {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--fs-sm);
    color: var(--text-dim);
    white-space: nowrap;
  }

  .clock.warn {
    gap: 4px;
    color: var(--warn);
  }

  .bpm:empty {
    display: none;
  }

  /* Narrower: the tempo in words goes first, then the Play label (it stays
     for screen readers), so everything fits on one line. */
  @container pattern-strip (inline-size < 420px) {
    .bpm {
      display: none;
    }
  }

  @container pattern-strip (inline-size < 330px) {
    .end :global(.play > span) {
      position: absolute;
      inline-size: 1px;
      block-size: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }

    .end :global(.play.small) {
      min-inline-size: 32px;
      padding-inline: 6px;
    }

    .strip > :global(.seg.small span) {
      padding-inline: 5px;
    }
  }

  /* Record and Play / Stop sit at the end of the row. */
  .end {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
    margin-inline-start: auto;
  }

</style>
