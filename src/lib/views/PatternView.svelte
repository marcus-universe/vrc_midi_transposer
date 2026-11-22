<script lang="ts">
  // Arpeggiator and step sequencer. The mode lives here; the settings and the
  // step grid are PatternPanels, which the dashboard shows as well.
  import { app } from '../app.svelte';
  import { t } from '../i18n/i18n.svelte';
  import type { PatternMode } from '../ipc';
  import PatternPanels from '../components/PatternPanels.svelte';
  import PatternTriggers from '../components/PatternTriggers.svelte';
  import Segmented from '../components/Segmented.svelte';

  // The mode playing now: MIDI can switch it for the session.
  const mode = $derived(app.patternMode);

  const modes = $derived<{ value: PatternMode; label: string }[]>([
    { value: 'off', label: t('pattern.modeOff') },
    { value: 'arp', label: t('pattern.modeArp') },
    { value: 'seq', label: t('pattern.modeSeq') },
  ]);
  const explain = $derived(
    mode === 'arp' ? t('pattern.explainArp') : mode === 'seq' ? t('pattern.explainSeq') : t('pattern.explainOff'),
  );
</script>

<div class="view">
  <header class="view-head">
    <h1>{t('pattern.title')}</h1>
    <p>{t('pattern.intro')}</p>
  </header>

  <section class="card" aria-labelledby="pat-mode-title">
    <h2 class="visually-hidden" id="pat-mode-title">{t('pattern.mode')}</h2>
    <div class="mode">
      <Segmented
        label={t('pattern.mode')}
        options={modes}
        value={mode}
        onchange={(v) => app.setPatternMode(v)}
      />
    </div>
    <p class="explain">{explain}</p>
  </section>

  <PatternPanels />

  <PatternTriggers />
</div>

<style>
  .mode {
    max-inline-size: 520px;
  }

  .explain {
    color: var(--text-dim);
    font-size: var(--fs-sm);
  }
</style>
