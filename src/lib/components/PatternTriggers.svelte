<script lang="ts">
  // MIDI messages that switch the pattern mode, start the sequencer or record.
  // One row per action: its trigger (set by MIDI Learn), whether the message
  // also goes to the output, and Learn / Remove. The triggers are part of the
  // project; a mode switched by MIDI is session state (see app.patternMode).
  import { onDestroy } from 'svelte';
  import { app } from '../app.svelte';
  import { triggerLabel } from '../format';
  import { t } from '../i18n/i18n.svelte';
  import { PATTERN_ACTIONS, type PatternAction } from '../ipc';
  import Icon from './Icon.svelte';

  const midi = $derived(app.project.pattern.midi);
  const learning = $derived(app.patternLearning);
  const nameOf = (a: PatternAction) => t(`pattern.actions.${a}`);

  function learn(a: PatternAction) {
    if (learning === a) app.stopLearn();
    else app.startPatternLearn(a);
  }

  function remove(a: PatternAction) {
    delete midi[a];
    app.touch(true);
  }

  function setForward(a: PatternAction, on: boolean) {
    const trigger = midi[a];
    if (!trigger) return;
    trigger.forward = on;
    app.touch(true);
  }

  // Esc stops MIDI Learn; leaving the view does too.
  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && learning !== null && !app.dialog) {
      e.preventDefault();
      app.stopLearn();
    }
  }
  onDestroy(() => {
    if (app.patternLearning !== null) app.stopLearn();
  });
</script>

<svelte:window onkeydown={onKey} />

<section class="card" aria-labelledby="pat-midi-title" aria-describedby="pat-midi-hint">
  <div class="card-head">
    <h2 class="card-title" id="pat-midi-title">{t('pattern.triggers')}</h2>
  </div>
  <p class="hint" id="pat-midi-hint">{t('pattern.triggersHint')}</p>

  <div class="scroll">
    <table>
      <thead>
        <tr>
          <th scope="col">{t('pattern.triggerAction')}</th>
          <th scope="col">{t('midi.trigger')}</th>
          <th scope="col">{t('pattern.forwardShort')}</th>
          <th scope="col"><span class="visually-hidden">{t('midi.learn')}</span></th>
        </tr>
      </thead>
      <tbody>
        {#each PATTERN_ACTIONS as a (a)}
          {@const trigger = midi[a]}
          {@const listening = learning === a}
          <tr class={[listening && 'listening']}>
            <th scope="row">{nameOf(a)}</th>
            <td>
              {#if listening}
                <span class="wait"><span class="dot accent" aria-hidden="true"></span>{t('midi.learning')}</span>
              {:else if trigger}
                <span class="badge">{triggerLabel(trigger)}</span>
              {:else}
                <span class="none">{t('midi.none')}</span>
              {/if}
            </td>
            <td>
              <input
                type="checkbox"
                class="check"
                aria-label={`${t('midi.forward')}: ${nameOf(a)}`}
                disabled={!trigger}
                checked={trigger?.forward ?? false}
                onchange={(e) => setForward(a, e.currentTarget.checked)}
              />
            </td>
            <td class="actions">
              <button
                type="button"
                class={['btn', listening ? 'btn-primary' : 'btn-ghost']}
                aria-label={listening ? undefined : t('midi.learnFor', { name: nameOf(a) })}
                onclick={() => learn(a)}
              >
                <Icon name="learn" size={16} />
                {listening ? t('midi.learnStop') : t('midi.learn')}
              </button>
              <button
                type="button"
                class="btn btn-ghost btn-icon"
                aria-label={t('pattern.removeTrigger', { action: nameOf(a) })}
                title={t('pattern.removeTrigger', { action: nameOf(a) })}
                disabled={!trigger}
                onclick={() => remove(a)}
              >
                <Icon name="trash" size={16} />
              </button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</section>

<style>
  .scroll {
    overflow-x: auto;
  }

  table {
    inline-size: 100%;
    border-collapse: collapse;
    font-size: var(--fs-sm);
  }

  th,
  td {
    padding: 6px var(--sp-2);
    text-align: start;
    vertical-align: middle;
    border-block-end: 1px solid var(--line);
  }

  thead th {
    color: var(--text-dim);
    font-weight: 500;
    font-size: var(--fs-xs);
  }

  tbody th {
    font-weight: 500;
    color: var(--text);
    white-space: nowrap;
  }

  tbody tr:last-child > * {
    border-block-end: 0;
  }

  tr.listening {
    background: var(--accent-soft);
  }

  .none {
    color: var(--text-dim);
  }

  .wait {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--accent-text);
  }

  .wait .dot {
    background: var(--accent-text);
  }

  .check {
    inline-size: 16px;
    block-size: 16px;
    accent-color: var(--accent);
  }

  .actions {
    white-space: nowrap;
    text-align: end;
  }

  .actions .btn {
    gap: 6px;
  }
</style>
