<script lang="ts">
  // Sequencer Play / Stop. The state comes from the engine: Play arms the
  // sequencer, which starts on the next bar line of the clock; pressing again
  // while armed cancels. The label changes with the state, so the button has
  // no aria-pressed; a status line announces the change.
  import { app } from '../app.svelte';
  import { t } from '../i18n/i18n.svelte';
  import Icon from './Icon.svelte';

  let { small = false }: { small?: boolean } = $props();

  const uid = $props.id();
  const st = $derived(app.play);
  const label = $derived(st.playing ? t('pattern.stop') : st.armed ? t('pattern.armed') : t('pattern.play'));
  const spoken = $derived(
    st.playing ? t('pattern.statePlaying') : st.armed ? t('pattern.stateArmed') : t('pattern.stateStopped'),
  );
</script>

<button
  type="button"
  class={['btn', 'play', !st.playing && !st.armed && 'btn-primary', st.armed && 'armed', small && 'small']}
  aria-describedby={st.armed ? `${uid}-hint` : undefined}
  onclick={() => app.seqPlay(!(st.playing || st.armed))}
>
  <Icon name={st.playing ? 'stop' : st.armed ? 'clock' : 'play'} size={small ? 14 : 18} />
  <span>{label}</span>
</button>
<span class="visually-hidden" id="{uid}-hint">{t('pattern.armedHint')}</span>
<span class="visually-hidden" role="status">{spoken}</span>

<style>
  .play {
    min-block-size: 44px;
    padding-inline: var(--sp-4);
    font-size: var(--fs-md);
  }

  .play.small {
    min-block-size: 28px;
    padding-inline: var(--sp-2) var(--sp-3);
    font-size: var(--fs-sm);
    gap: 6px;
  }

  /* Waiting for the bar line: a dashed accent frame and a clock icon, no pulsing. */
  .armed {
    border: 1px dashed var(--accent-text);
    background: var(--accent-soft);
    color: var(--text);
  }

  .armed:hover {
    background: var(--accent-soft);
    border-style: solid;
  }

  span {
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
