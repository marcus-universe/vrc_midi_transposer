<script lang="ts">
  // Project switcher: a menu button listing the projects and the project
  // commands. Uses the Popover API (top layer, light dismiss, Esc) where it
  // exists and a plain positioned element with the same behaviour where not.
  import { tick } from 'svelte';
  import { app } from '../app.svelte';
  import { shortPath } from '../format';
  import { t } from '../i18n/i18n.svelte';
  import type { RecentFile } from '../ipc';
  import Icon from './Icon.svelte';

  const hasPopover = 'popover' in HTMLElement.prototype;
  const hasFocusgroup = 'focusGroup' in HTMLElement.prototype;
  const hasAnchor = CSS.supports('position-area', 'block-end');

  const uid = $props.id();
  let open = $state(false);
  let trigger = $state<HTMLButtonElement>();
  let menu = $state<HTMLDivElement>();
  let group = $state<HTMLDivElement>();
  let enterOnOpen = false;
  let openAtPress = false;

  // Attributes newer than the DOM typings.
  const popoverAttr = (hasPopover ? { popover: 'auto' } : {}) as Record<string, string>;
  const focusgroupAttr = { focusgroup: 'menu nomemory' } as Record<string, string>;
  const itemRole = hasFocusgroup ? undefined : 'menuitem';

  function items(): HTMLElement[] {
    return group ? [...group.querySelectorAll<HTMLElement>('button:not(:disabled)')] : [];
  }

  function show(enter: boolean) {
    enterOnOpen = enter;
    if (hasPopover) menu?.showPopover();
    else {
      open = true;
      void tick().then(opened);
    }
  }

  /** `restore` puts focus back on the trigger; without a ring after a pointer activation. */
  function hide(restore: boolean, fromPointer = false) {
    if (restore) trigger?.focus({ focusVisible: !fromPointer } as FocusOptions);
    if (hasPopover) {
      if (menu?.matches(':popover-open')) menu.hidePopover();
    } else open = false;
  }

  function opened() {
    if (!hasAnchor && menu && trigger) {
      const t = trigger.getBoundingClientRect();
      const m = menu.getBoundingClientRect();
      const below = t.bottom + 6;
      const top = below + m.height > innerHeight ? Math.max(8, t.top - 6 - m.height) : below;
      const left = Math.max(8, Math.min(t.left, innerWidth - m.width - 8));
      menu.style.left = `${left}px`;
      menu.style.top = `${top}px`;
    }
    // Only a keyboard open enters the menu; after a pointer open the trigger keeps focus.
    if (enterOnOpen) items()[0]?.focus();
  }

  function onToggle(e: ToggleEvent) {
    open = e.newState === 'open';
    if (open) opened();
  }

  function onTriggerClick(e: MouseEvent) {
    const fromPointer = e.detail !== 0;
    // A press on the trigger of an open menu already light-dismissed it; do not reopen.
    if (fromPointer && openAtPress) {
      openAtPress = false;
      if (open) hide(false);
      return;
    }
    if (open) hide(true, fromPointer);
    else show(!fromPointer);
  }

  function onTriggerKey(e: KeyboardEvent) {
    if (e.key !== 'ArrowDown') return;
    e.preventDefault();
    if (open) items()[0]?.focus();
    else show(true);
  }

  function onMenuKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && !hasPopover) {
      e.preventDefault();
      hide(true);
      return;
    }
    if (e.key === 'Tab') {
      // Tab leaves the menu system: close, and let focus move on from the trigger.
      hide(true);
      return;
    }
    const list = items();
    const current = list.indexOf(document.activeElement as HTMLElement);
    let next = -1;
    if (e.key === 'Home') next = 0;
    else if (e.key === 'End') next = list.length - 1;
    else if (!hasFocusgroup && (e.key === 'ArrowDown' || e.key === 'ArrowUp') && current >= 0) {
      next = (current + (e.key === 'ArrowUp' ? -1 : 1) + list.length) % list.length;
    }
    if (next < 0) return;
    e.preventDefault();
    list[next]?.focus({ preventScroll: true });
  }

  /** Hover and focus are one item, as in a native menu. */
  function onPointerMove(e: PointerEvent) {
    const item = (e.target as HTMLElement).closest<HTMLButtonElement>('button');
    if (item && !item.disabled && document.activeElement !== item) {
      item.focus({ focusVisible: false, preventScroll: true } as FocusOptions);
    }
  }

  function run(e: MouseEvent, action: () => void) {
    hide(true, e.detail !== 0);
    action();
  }

  const isMac = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
  /** Shortcut as shown in the menu; the item's aria-keyshortcuts says it to screen readers. */
  const keys = (key: string, shift = false) => (isMac ? `${shift ? '⇧' : ''}⌘${key}` : `Ctrl+${shift ? 'Shift+' : ''}${key}`);

  function openRecent(e: MouseEvent, r: RecentFile) {
    if (!r.exists) return;
    run(e, () => app.openFile(r.path, r.name));
  }

  /** Removes an entry and keeps focus in the list (on the entry that took its place). */
  async function removeRecent(r: RecentFile, i: number) {
    if (!(await app.removeRecent(r.path))) return;
    await tick();
    const left = group ? [...group.querySelectorAll<HTMLElement>('.recent')] : [];
    (left[Math.min(i, left.length - 1)] ?? items()[0])?.focus({ preventScroll: true });
  }

  function onRecentKey(e: KeyboardEvent, r: RecentFile, i: number) {
    if (e.key !== 'Delete') return;
    e.preventDefault();
    void removeRecent(r, i);
  }

  // Without the Popover API: light dismiss by hand.
  $effect(() => {
    if (hasPopover || !open) return;
    const onDown = (e: PointerEvent) => {
      const t = e.target as Node;
      if (!menu?.contains(t) && !trigger?.contains(t)) hide(false);
    };
    const onFocus = (e: FocusEvent) => {
      const t = e.target as Node;
      if (!menu?.contains(t) && !trigger?.contains(t)) hide(false);
    };
    document.addEventListener('pointerdown', onDown, true);
    document.addEventListener('focusin', onFocus);
    return () => {
      document.removeEventListener('pointerdown', onDown, true);
      document.removeEventListener('focusin', onFocus);
    };
  });
</script>

<div class="switcher">
  <button
    bind:this={trigger}
    id="{uid}-trigger"
    type="button"
    class="trigger"
    aria-haspopup="menu"
    aria-expanded={open}
    aria-controls="{uid}-menu"
    onpointerdown={() => (openAtPress = open)}
    onclick={onTriggerClick}
    onkeydown={onTriggerKey}
    title={app.path ?? t('project.notSaved')}
    aria-describedby="{uid}-file"
  >
    <span class="name">{app.project.name || t('project.untitled')}</span>
    <span class={['unsaved', app.dirty && 'on']} aria-hidden="true"></span>
    {#if app.dirty}<span class="visually-hidden">{t('project.unsavedSuffix')}</span>{/if}
    <Icon name="chevron" size={16} />
  </button>
  <span class="visually-hidden" id="{uid}-file">{app.path ?? t('project.notSaved')}</span>

  <div
    bind:this={menu}
    id="{uid}-menu"
    class={['menu', !hasPopover && open && 'is-open']}
    {...popoverAttr}
    ontoggle={onToggle}
  >
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      bind:this={group}
      class="items scroller"
      {...focusgroupAttr}
      role={hasFocusgroup ? undefined : 'menu'}
      aria-labelledby="{uid}-trigger"
      onkeydown={onMenuKey}
      onpointermove={onPointerMove}
    >
      <button type="button" role={itemRole} disabled={app.busy} onclick={(e) => run(e, () => app.newProject())}>
        <span class="lead"><Icon name="plus" size={16} /></span>
        <span class="text">{t('project.new')}</span>
      </button>
      <button
        type="button"
        role={itemRole}
        disabled={app.busy}
        aria-keyshortcuts="Control+O"
        aria-describedby="{uid}-open-hint"
        onclick={(e) => run(e, () => app.openDialog())}
      >
        <span class="lead"><Icon name="folder" size={16} /></span>
        <span class="text two">
          <span class="line">{t('project.open')}</span>
          <span class="sub wrap" id="{uid}-open-hint">{t('project.openHint')}</span>
        </span>
        <span class="kbd" aria-hidden="true">{keys('O')}</span>
      </button>
      <button
        type="button"
        role={itemRole}
        disabled={app.busy}
        aria-keyshortcuts="Control+S"
        onclick={(e) => run(e, () => void app.save())}
      >
        <span class="lead"><Icon name="save" size={16} /></span>
        <span class="text">{t('project.save')}</span>
        <span class="kbd" aria-hidden="true">{keys('S')}</span>
      </button>
      <button
        type="button"
        role={itemRole}
        disabled={app.busy}
        aria-keyshortcuts="Control+Shift+S"
        onclick={(e) => run(e, () => void app.saveAs())}
      >
        <span class="lead"></span>
        <span class="text">{t('project.saveAs')}</span>
        <span class="kbd" aria-hidden="true">{keys('S', true)}</span>
      </button>
      <hr />
      <div role="group" aria-label={t('project.recent')}>
        <span class="heading" aria-hidden="true">{t('project.recent')}</span>
        {#each app.recent as r, i (r.path)}
          {@const current = r.path === app.path}
          <div class="recent-row" role="none">
            <button
              type="button"
              class={['recent', !r.exists && 'missing']}
              role={itemRole}
              disabled={app.busy}
              aria-disabled={!r.exists}
              title={r.path}
              onclick={(e) => openRecent(e, r)}
              onkeydown={(e) => onRecentKey(e, r, i)}
            >
              <span class="lead">{#if current}<Icon name="check" size={16} />{/if}</span>
              <span class="text two">
                <span class="line">{r.name}</span>
                <span class="sub">
                  {#if !r.exists}<span class="lost"><Icon name="alert" size={12} />{t('project.notFound')}</span
                    >{' · '}{/if}{shortPath(r.path)}
                </span>
              </span>
              {#if current}<span class="visually-hidden">, {t('project.currentFile')}</span>{/if}
            </button>
            <button
              type="button"
              class="remove"
              role={itemRole}
              aria-label={t('project.removeRecent', { name: r.name })}
              title={t('project.removeRecent', { name: r.name })}
              onclick={() => void removeRecent(r, i)}
            >
              <Icon name="close" size={14} />
            </button>
          </div>
        {:else}
          <p class="none">{t('project.recentEmpty')}</p>
        {/each}
      </div>
    </div>
  </div>
</div>

<style>
  .switcher {
    display: flex;
    min-inline-size: 0;
  }

  .trigger {
    anchor-name: --project-menu;
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    min-inline-size: 0;
    max-inline-size: 260px;
    min-block-size: 32px;
    padding: 0 var(--sp-2) 0 var(--sp-3);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
    transition:
      background-color var(--dur) var(--ease),
      border-color var(--dur) var(--ease);
  }

  .trigger:hover,
  .trigger[aria-expanded='true'] {
    background: var(--surface-3);
    border-color: var(--line-strong);
  }

  .trigger :global(svg) {
    color: var(--text-dim);
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* The slot is always there, so the bar does not shift when the dot appears. */
  .unsaved {
    flex: none;
    inline-size: 8px;
    block-size: 8px;
    border-radius: 50%;
    background: var(--warn);
    visibility: hidden;
  }

  .unsaved.on {
    visibility: visible;
  }

  .menu {
    position: fixed;
    inset: auto;
    z-index: 30;
    inline-size: min(340px, calc(100vw - 16px));
    margin: 0;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--surface-2);
    color: var(--text);
    box-shadow: var(--shadow-2);
    overflow: hidden;
  }

  /* :is() is forgiving, so engines without :popover-open keep the rule. */
  .menu:not(:is(:popover-open, .is-open)) {
    display: none;
  }

  @supports (position-area: block-end) {
    .menu {
      position-anchor: --project-menu;
      position-area: block-end span-inline-end;
      position-try-fallbacks: flip-block;
      margin-block: 6px;
    }
  }

  .items {
    display: flex;
    flex-direction: column;
    max-block-size: min(70vh, 460px);
    padding: var(--sp-1);
  }

  .items [role='group'] {
    display: flex;
    flex-direction: column;
  }

  .heading {
    padding: var(--sp-2) var(--sp-2) var(--sp-1);
    color: var(--text-dim);
    font-size: var(--fs-xs);
    font-weight: 500;
  }

  .items button {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    min-block-size: 32px;
    padding: 0 var(--sp-2);
    border: 0;
    border-radius: 6px;
    background: none;
    color: var(--text);
    font: inherit;
    text-align: start;
    cursor: pointer;
  }

  .items button:hover,
  .items button:focus {
    background: var(--surface-3);
    outline: none;
  }

  .items button:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: -2px;
  }

  .lead {
    display: flex;
    flex: none;
    inline-size: 16px;
    color: var(--text-dim);
  }

  .text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .items button:disabled {
    color: var(--text-faint);
    cursor: default;
  }

  .text.two {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 1px;
    min-inline-size: 0;
    padding-block: 5px;
    white-space: normal;
  }

  .line,
  .sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub {
    color: var(--text-dim);
    font-size: var(--fs-xs);
  }

  .sub.wrap {
    white-space: normal;
  }

  .kbd {
    flex: none;
    margin-inline-start: auto;
    padding-inline-start: var(--sp-3);
    color: var(--text-dim);
    font-size: var(--fs-xs);
    font-variant-numeric: tabular-nums;
  }

  .recent-row {
    display: flex;
    align-items: stretch;
    gap: 2px;
  }

  .recent {
    flex: 1;
    min-inline-size: 0;
  }

  /* Looks unavailable, but stays focusable so its remove button and its state can be reached. */
  .items button.missing {
    cursor: default;
  }

  .missing .line {
    color: var(--text-dim);
  }

  .lost {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--warn);
  }

  .items button.remove {
    flex: none;
    justify-content: center;
    inline-size: 32px;
    padding: 0;
    color: var(--text-dim);
  }

  .items button.remove:hover,
  .items button.remove:focus {
    color: var(--text);
  }

  .none {
    padding: var(--sp-1) var(--sp-2) var(--sp-2);
    color: var(--text-dim);
    font-size: var(--fs-sm);
  }

  hr {
    block-size: 1px;
    margin: var(--sp-1) 0;
    border: 0;
    background: var(--line);
  }
</style>
