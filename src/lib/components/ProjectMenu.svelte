<script lang="ts">
  // Project switcher: a menu button listing the projects and the project
  // commands. Uses the Popover API (top layer, light dismiss, Esc) where it
  // exists and a plain positioned element with the same behaviour where not.
  import { tick } from 'svelte';
  import { app } from '../app.svelte';
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
  >
    <span class="name">{app.project.name || 'Untitled project'}</span>
    <span class={['unsaved', app.dirty && 'on']} aria-hidden="true"></span>
    {#if app.dirty}<span class="visually-hidden">, unsaved changes</span>{/if}
    <Icon name="chevron" size={16} />
  </button>

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
      <div role="group" aria-label="Open project">
        <span class="heading" aria-hidden="true">Projects</span>
        {#each app.projects as p (p.id)}
          <button
            type="button"
            role="menuitemradio"
            aria-checked={p.id === app.project.id}
            onclick={(e) => run(e, () => app.openProject(p.id))}
          >
            <span class="lead">{#if p.id === app.project.id}<Icon name="check" size={16} />{/if}</span>
            <span class="text">{p.id === app.project.id ? app.project.name || p.name : p.name}</span>
          </button>
        {/each}
      </div>
      <hr />
      <button type="button" role={itemRole} onclick={(e) => run(e, () => app.askNewProject())}>
        <span class="lead"><Icon name="plus" size={16} /></span>
        <span class="text">New project…</span>
      </button>
      <button type="button" role={itemRole} onclick={(e) => run(e, () => app.askDuplicateProject())}>
        <span class="lead"><Icon name="copy" size={16} /></span>
        <span class="text">Duplicate…</span>
      </button>
      <button type="button" role={itemRole} onclick={(e) => run(e, () => app.importProject())}>
        <span class="lead"><Icon name="folder" size={16} /></span>
        <span class="text">Import…</span>
      </button>
      <button type="button" role={itemRole} onclick={(e) => run(e, () => void app.exportProject())}>
        <span class="lead"><Icon name="external" size={16} /></span>
        <span class="text">Export…</span>
      </button>
      <hr />
      <button
        type="button"
        class="danger"
        role={itemRole}
        onclick={(e) => run(e, () => (app.dialog = { kind: 'delete' }))}
      >
        <span class="lead"><Icon name="trash" size={16} /></span>
        <span class="text">Delete project…</span>
      </button>
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
    inline-size: 264px;
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

  [aria-checked='true'] .lead {
    color: var(--accent-text);
  }

  [aria-checked='true'] .text {
    font-weight: 600;
  }

  .text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .items button.danger,
  .items button.danger .lead {
    color: var(--err);
  }

  hr {
    block-size: 1px;
    margin: var(--sp-1) 0;
    border: 0;
    background: var(--line);
  }
</style>
