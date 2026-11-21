<script lang="ts">
  // Slim icon rail. Each button is named by aria-label; the tooltip repeats
  // that name for sighted users on hover and keyboard focus.
  import { app, type View } from '../app.svelte';
  import { t } from '../i18n/i18n.svelte';
  import Icon, { type IconName } from './Icon.svelte';

  const items: { view: View; label: () => string; icon: IconName }[] = [
    { view: 'dashboard', label: () => t('rail.dashboard'), icon: 'dashboard' },
    { view: 'osc', label: () => 'OSC', icon: 'osc' },
    { view: 'mqtt', label: () => 'MQTT', icon: 'mqtt' },
    {
      view: 'settings',
      label: () => (app.updateAvailable ? t('rail.settingsUpdate') : t('rail.settings')),
      icon: 'settings',
    },
  ];
</script>

<nav class="rail" aria-label={t('rail.views')}>
  <div class="logo" aria-hidden="true">
    <Icon name="piano" size={20} />
  </div>
  <ul>
    {#each items as item (item.view)}
      <li>
        <button
          type="button"
          class="item"
          aria-label={item.label()}
          aria-current={app.ui.view === item.view ? 'page' : undefined}
          data-tip={item.label()}
          onclick={() => app.setView(item.view)}
        >
          <Icon name={item.icon} size={20} />
          {#if item.view === 'settings' && app.updateAvailable}<span class="badge-dot" aria-hidden="true"></span>{/if}
        </button>
      </li>
    {/each}
  </ul>
  <button
    type="button"
    class="item"
    aria-label={t('rail.about')}
    aria-haspopup="dialog"
    data-tip={t('rail.about')}
    onclick={() => (app.dialog = { kind: 'about' })}
  >
    <Icon name="info" size={20} />
  </button>
</nav>

<style>
  .rail {
    position: relative;
    z-index: 20;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-2) 0 var(--sp-3);
    background: var(--surface-1);
    border-inline-end: 1px solid var(--line);
  }

  .logo {
    display: grid;
    place-items: center;
    inline-size: 32px;
    block-size: 32px;
    margin-block-end: var(--sp-2);
    border-radius: var(--r-sm);
    background: var(--accent);
    color: var(--on-accent);
  }

  ul {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--sp-1);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .item {
    position: relative;
    display: grid;
    place-items: center;
    inline-size: 40px;
    block-size: 40px;
    padding: 0;
    border: 0;
    border-radius: var(--r-sm);
    background: none;
    color: var(--text-dim);
    cursor: pointer;
    transition:
      background-color var(--dur) var(--ease),
      color var(--dur) var(--ease);
  }

  .item:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  .item:active {
    background: var(--surface-3-hover);
  }

  .item[aria-current='page'] {
    background: var(--accent-soft);
    color: var(--text);
  }

  /* The current view is marked by a bar as well as by colour. */
  .item[aria-current='page']::before {
    content: '';
    position: absolute;
    inset-block: 8px;
    inset-inline-start: -8px;
    inline-size: 3px;
    border-radius: 0 3px 3px 0;
    background: var(--accent-text);
  }

  .badge-dot {
    position: absolute;
    inset-block-start: 7px;
    inset-inline-end: 7px;
    inline-size: 9px;
    block-size: 9px;
    border: 2px solid var(--surface-1);
    border-radius: 50%;
    background: var(--accent-text);
  }

  @media (forced-colors: active) {
    .badge-dot {
      forced-color-adjust: none;
      background: CanvasText;
    }
  }

  .item::after {
    content: attr(data-tip);
    position: absolute;
    inset-inline-start: calc(100% + 12px);
    inset-block-start: 50%;
    translate: -4px -50%;
    padding: 5px var(--sp-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--surface-3);
    color: var(--text);
    font-size: var(--fs-sm);
    font-weight: 500;
    line-height: 1.2;
    white-space: nowrap;
    box-shadow: var(--shadow-1);
    opacity: 0;
    pointer-events: none;
    transition:
      opacity var(--dur) var(--ease),
      translate var(--dur) var(--ease);
  }

  .item:hover::after,
  .item:focus-visible::after {
    opacity: 1;
    translate: 0 -50%;
  }
</style>
