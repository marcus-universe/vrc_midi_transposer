<script lang="ts">
  import { app } from '../app.svelte';
  import Icon from './Icon.svelte';
  import ProjectMenu from './ProjectMenu.svelte';

  type Tone = '' | 'ok' | 'warn' | 'err';
  type Pill = { name: string; tone: Tone; word: string };

  const s = $derived(app.snapshot);

  const midi = $derived.by((): Pill => {
    const a = s.midi_in.state;
    const b = s.midi_out.state;
    if (a === 'error' || b === 'error') return { name: 'MIDI', tone: 'err', word: 'Error' };
    if (a === 'missing' || b === 'missing') return { name: 'MIDI', tone: 'warn', word: 'Device missing' };
    if (a === 'connected' && b === 'connected') return { name: 'MIDI', tone: 'ok', word: 'Connected' };
    if (a === 'connected') return { name: 'MIDI', tone: 'warn', word: 'No output' };
    if (b === 'connected') return { name: 'MIDI', tone: 'warn', word: 'No input' };
    return { name: 'MIDI', tone: '', word: 'Not set' };
  });

  const osc = $derived.by((): Pill => {
    if (s.osc_listener.state === 'error') return { name: 'OSC', tone: 'err', word: 'Error' };
    if (s.osc_sending) return { name: 'OSC', tone: 'ok', word: 'Sending' };
    if (s.osc_listener.state === 'running') return { name: 'OSC', tone: 'ok', word: 'Listening' };
    return { name: 'OSC', tone: '', word: 'Stopped' };
  });

  const mqtt = $derived.by((): Pill => {
    switch (s.mqtt.state) {
      case 'connected':
        return { name: 'MQTT', tone: 'ok', word: 'Connected' };
      case 'connecting':
        return { name: 'MQTT', tone: 'warn', word: 'Connecting' };
      case 'error':
        return { name: 'MQTT', tone: 'err', word: 'Error' };
      default:
        return { name: 'MQTT', tone: '', word: 'Off' };
    }
  });

  const pills = $derived([midi, osc, mqtt]);
</script>

<header class="topbar">
  <ProjectMenu />

  <button type="button" class="btn btn-primary" disabled={!app.dirty || app.busy} onclick={() => app.save()}>
    Save
  </button>
  <button
    type="button"
    class="btn btn-ghost"
    disabled={!app.dirty || app.busy}
    onclick={() => (app.dialog = { kind: 'revert' })}
  >
    Revert
  </button>

  <ul class="pills" aria-label="Connection status">
    {#each pills as p (p.name)}
      <li class="pill">
        <span class="pill-name">{p.name}</span>
        <span class={['dot', p.tone]}></span>
        <span class="pill-word">{p.word}</span>
      </li>
    {/each}
  </ul>

  <button
    type="button"
    class={['btn btn-ghost btn-icon', app.ui.console_open && 'pressed']}
    aria-label="Console"
    aria-pressed={app.ui.console_open}
    title="Console (Ctrl+`)"
    onclick={() => app.toggleConsole()}
  >
    <Icon name="terminal" />
  </button>
</header>

<style>
  .topbar {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    min-inline-size: 0;
    padding: 0 var(--sp-3) 0 var(--sp-4);
    background: var(--surface-1);
    border-block-end: 1px solid var(--line);
  }

  .pills {
    display: flex;
    gap: var(--sp-2);
    margin: 0 0 0 auto;
    padding: 0;
    list-style: none;
  }

  .pill {
    display: flex;
    align-items: center;
    gap: 6px;
    block-size: 26px;
    padding: 0 10px;
    border: 1px solid var(--line);
    border-radius: var(--r-pill);
    background: var(--surface-2);
    font-size: var(--fs-sm);
    white-space: nowrap;
  }

  .pill-name {
    color: var(--text-dim);
    font-weight: 500;
  }

  .pill-word {
    color: var(--text);
  }

  .pressed {
    background: var(--accent-soft);
    color: var(--text);
  }
</style>
