<script lang="ts">
  import { app } from './lib/app.svelte';
  import { api } from './lib/ipc';
  import Console from './lib/components/Console.svelte';
  import Dialogs from './lib/components/Dialogs.svelte';
  import Icon from './lib/components/Icon.svelte';
  import Piano from './lib/components/Piano.svelte';
  import Rail from './lib/components/Rail.svelte';
  import TopBar from './lib/components/TopBar.svelte';
  import Dashboard from './lib/views/Dashboard.svelte';
  import MqttView from './lib/views/MqttView.svelte';
  import OscView from './lib/views/OscView.svelte';
  import SettingsView from './lib/views/SettingsView.svelte';

  let main = $state<HTMLElement>();

  // A new view starts at the top.
  $effect(() => {
    void app.ui.view;
    if (main) main.scrollTop = 0;
  });

  function onKeydown(e: KeyboardEvent) {
    if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
    if (e.key.toLowerCase() === 's') {
      e.preventDefault();
      // Not while a dialog is asking something: its buttons are the way forward.
      if (app.dirty && !app.busy && !app.dialog) void app.save();
    } else if (e.key === '`' || e.code === 'Backquote') {
      e.preventDefault();
      app.toggleConsole();
    }
  }

  function onVisibility() {
    api.setVisible(document.visibilityState === 'visible').catch(() => {});
  }
</script>

<svelte:window onkeydown={onKeydown} />
<svelte:document onvisibilitychange={onVisibility} />

{#if app.ready}
  <div class="app">
    <Rail />
    <TopBar />

    <main class="main scroller" bind:this={main}>
      {#if app.notice}
        <div class={['notice', app.notice.kind]} role={app.notice.kind === 'error' ? 'alert' : 'status'}>
          <Icon name={app.notice.kind === 'error' ? 'alert' : 'check'} size={16} />
          <p class="selectable">
            {#if app.notice.kind === 'error'}<span class="visually-hidden">Error: </span>{/if}{app.notice.text}
          </p>
          <button type="button" class="btn btn-ghost btn-icon" aria-label="Dismiss" onclick={() => (app.notice = null)}>
            <Icon name="close" size={16} />
          </button>
        </div>
      {/if}

      {#key app.epoch}
        {#if app.ui.view === 'dashboard'}
          <Dashboard />
        {:else if app.ui.view === 'osc'}
          <OscView />
        {:else if app.ui.view === 'mqtt'}
          <MqttView />
        {:else}
          <SettingsView />
        {/if}
      {/key}
    </main>

    {#if app.ui.view === 'dashboard'}
      <div class="piano"><Piano /></div>
    {/if}

    {#if app.ui.console_open}
      <div class="drawer"><Console /></div>
    {/if}
  </div>

  <Dialogs />
{/if}

<style>
  .app {
    display: grid;
    grid-template-columns: var(--rail-w) minmax(0, 1fr);
    grid-template-rows: var(--topbar-h) minmax(0, 1fr) auto auto;
    grid-template-areas:
      'rail top'
      'rail main'
      'rail piano'
      'rail drawer';
    block-size: 100dvh;
    min-inline-size: 0;
  }

  .app > :global(.rail) {
    grid-area: rail;
  }

  .app > :global(.topbar) {
    grid-area: top;
  }

  .main {
    grid-area: main;
    container: main / inline-size;
    min-block-size: 0;
  }

  .piano {
    grid-area: piano;
    min-inline-size: 0;
  }

  .drawer {
    grid-area: drawer;
    min-inline-size: 0;
  }

  .notice {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    margin: var(--sp-4) var(--sp-4) 0;
    padding: var(--sp-1) var(--sp-1) var(--sp-1) var(--sp-3);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--surface-2);
  }

  .notice p {
    flex: 1;
    min-inline-size: 0;
    overflow-wrap: anywhere;
  }

  .notice.error {
    border-color: var(--err);
    background: var(--err-soft);
  }

  .notice.error > :global(svg) {
    color: var(--err);
  }

  .notice.info > :global(svg) {
    color: var(--ok);
  }

  /* Shared frame of the form views. */
  .main :global(.view) {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    max-inline-size: 1240px;
    padding: var(--sp-4);
  }

  .main :global(.view-head) {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding-block: var(--sp-1);
  }

  .main :global(.view-head h1) {
    font-size: var(--fs-lg);
    font-weight: 600;
    line-height: 1.3;
  }

  .main :global(.view-head p) {
    color: var(--text-dim);
    font-size: var(--fs-sm);
  }
</style>
