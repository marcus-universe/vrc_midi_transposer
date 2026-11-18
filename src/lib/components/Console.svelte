<script lang="ts">
  // Console drawer. Mounted only while open. Log lines are appended to the DOM
  // by hand (bounded to MAX_DOM_LINES) instead of going through a reactive
  // list, so a burst of debug output costs one append per line and nothing else.
  import { onMount } from 'svelte';
  import { app, CONSOLE_MIN_HEIGHT } from '../app.svelte';
  import { clockTime } from '../format';
  import type { LogLine } from '../ipc';
  import Icon from './Icon.svelte';
  import Switch from './Switch.svelte';

  const MAX_DOM_LINES = 500;
  const HISTORY_LIMIT = 100;
  const TAG: Partial<Record<LogLine['level'], string>> = { warn: 'warn', error: 'error' };

  let scroller = $state<HTMLDivElement>();
  let lines = $state<HTMLDivElement>();
  let input = $state<HTMLInputElement>();
  let stick = true;

  let command = $state('');
  const history: string[] = [];
  let historyAt = -1;
  let stash = '';

  let copyState = $state<'' | 'ok' | 'failed'>('');
  let copyTimer: ReturnType<typeof setTimeout> | undefined;

  function lineEl(l: LogLine): HTMLDivElement {
    const row = document.createElement('div');
    row.className = `ln ${l.level}`;
    const ts = document.createElement('span');
    ts.className = 'ts';
    ts.textContent = clockTime(l.ts);
    row.append(ts);
    const tag = TAG[l.level];
    if (tag) {
      const t = document.createElement('span');
      t.className = 'tag';
      t.textContent = tag;
      row.append(t);
    }
    row.append(l.text);
    return row;
  }

  function append(added: readonly LogLine[]) {
    if (!lines || !added.length) return;
    const frag = document.createDocumentFragment();
    for (const l of added.slice(-MAX_DOM_LINES)) frag.append(lineEl(l));
    lines.append(frag);
    while (lines.childElementCount > MAX_DOM_LINES) lines.firstElementChild?.remove();
  }

  function toBottom() {
    if (scroller) scroller.scrollTop = scroller.scrollHeight;
  }

  onMount(() => {
    append(app.logs.slice(-MAX_DOM_LINES));
    toBottom();
    if (app.focusConsole) {
      app.focusConsole = false;
      input?.focus();
    }
    const off = app.onLogs((added, cleared) => {
      if (cleared) lines?.replaceChildren();
      append(cleared ? app.logs.slice(-MAX_DOM_LINES) : added);
      if (stick) toBottom();
    });
    return () => {
      off();
      clearTimeout(copyTimer);
    };
  });

  function onScroll() {
    if (!scroller) return;
    stick = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 8;
  }

  // --- Command line --------------------------------------------------------

  function submit(e: SubmitEvent) {
    e.preventDefault();
    const line = command.trim();
    if (!line) return;
    if (history[history.length - 1] !== line) history.push(line);
    if (history.length > HISTORY_LIMIT) history.shift();
    historyAt = -1;
    command = '';
    stick = true;
    void app.consoleLine(line);
  }

  function onKey(e: KeyboardEvent) {
    if (e.isComposing) return;
    if (e.key === 'ArrowUp') {
      if (!history.length) return;
      e.preventDefault();
      if (historyAt === -1) {
        stash = command;
        historyAt = history.length - 1;
      } else if (historyAt > 0) historyAt--;
      command = history[historyAt];
    } else if (e.key === 'ArrowDown') {
      if (historyAt === -1) return;
      e.preventDefault();
      if (historyAt < history.length - 1) command = history[++historyAt];
      else {
        historyAt = -1;
        command = stash;
      }
    }
  }

  // --- Copy all ------------------------------------------------------------

  function legacyCopy(text: string): boolean {
    const ta = document.createElement('textarea');
    ta.value = text;
    ta.setAttribute('readonly', '');
    ta.style.position = 'fixed';
    ta.style.opacity = '0';
    document.body.append(ta);
    ta.select();
    let ok = false;
    try {
      ok = document.execCommand('copy');
    } catch {
      ok = false;
    }
    ta.remove();
    return ok;
  }

  async function copyAll() {
    const text = app.logs.map((l) => `${clockTime(l.ts)} ${TAG[l.level] ? `[${TAG[l.level]}] ` : ''}${l.text}`).join('\n');
    let ok = false;
    try {
      await navigator.clipboard.writeText(text);
      ok = true;
    } catch {
      ok = legacyCopy(text);
    }
    clearTimeout(copyTimer);
    copyState = ok ? 'ok' : 'failed';
    copyTimer = setTimeout(() => (copyState = ''), 2000);
  }

  // --- Resize --------------------------------------------------------------

  function maxHeight(): number {
    return Math.max(CONSOLE_MIN_HEIGHT, window.innerHeight - 220);
  }

  function setHeight(h: number) {
    app.ui.console_height = Math.round(Math.min(maxHeight(), Math.max(CONSOLE_MIN_HEIGHT, h)));
  }

  function startDrag(e: PointerEvent) {
    if (e.button !== 0) return;
    const handle = e.currentTarget as HTMLElement;
    const startY = e.clientY;
    const startH = app.ui.console_height;
    handle.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => setHeight(startH + (startY - ev.clientY));
    const end = () => {
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', end);
      handle.removeEventListener('pointercancel', end);
      app.saveUiSoon();
      if (stick) toBottom();
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', end);
    handle.addEventListener('pointercancel', end);
    e.preventDefault();
  }

  function resizeKey(e: KeyboardEvent) {
    const step = e.shiftKey ? 64 : 16;
    if (e.key === 'ArrowUp') setHeight(app.ui.console_height + step);
    else if (e.key === 'ArrowDown') setHeight(app.ui.console_height - step);
    else if (e.key === 'Home') setHeight(CONSOLE_MIN_HEIGHT);
    else if (e.key === 'End') setHeight(maxHeight());
    else return;
    e.preventDefault();
    app.saveUiSoon();
  }
</script>

<section class="console" aria-label="Console" style:block-size="{app.ui.console_height}px">
  <!-- A focusable separator is a window splitter: an interactive widget. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="grip"
    role="separator"
    tabindex="0"
    aria-orientation="horizontal"
    aria-label="Resize console"
    aria-valuenow={app.ui.console_height}
    aria-valuemin={CONSOLE_MIN_HEIGHT}
    aria-valuemax={maxHeight()}
    onpointerdown={startDrag}
    onkeydown={resizeKey}
  ></div>

  <div class="head">
    <h2>Console</h2>
    <div class="tools">
      <Switch
        compact
        label="Debug"
        checked={app.snapshot.debug}
        onchange={(on) => app.engine({ type: 'set_debug', on })}
      />
      <button type="button" class="btn btn-ghost small" onclick={() => app.clearLog()}>Clear</button>
      <span class="copy">
        <button type="button" class="btn btn-ghost small" onclick={copyAll}>
          <Icon name={copyState === 'ok' ? 'check' : 'copy'} size={15} />
          Copy all
        </button>
        <span class={['tip', copyState && 'show']} role="status">
          {copyState === 'ok' ? 'Copied' : copyState === 'failed' ? 'Copy failed' : ''}
        </span>
      </span>
      <button
        type="button"
        class="btn btn-ghost btn-icon small"
        aria-label="Close console"
        onclick={() => app.toggleConsole(false)}
      >
        <Icon name="close" size={16} />
      </button>
    </div>
  </div>

  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="log scroller selectable"
    role="log"
    aria-live="off"
    aria-label="Log output"
    tabindex="0"
    bind:this={scroller}
    onscroll={onScroll}
  >
    {#if app.banner}<pre class="banner" aria-hidden="true">{app.banner}</pre>{/if}
    <div class="lines" bind:this={lines}></div>
  </div>

  <form class="prompt" onsubmit={submit}>
    <label for="console-input" class="visually-hidden">Command</label>
    <span class="caret" aria-hidden="true">&gt;</span>
    <input
      bind:this={input}
      id="console-input"
      type="text"
      placeholder="Type a command, for example help"
      autocomplete="off"
      autocapitalize="none"
      spellcheck="false"
      enterkeyhint="send"
      bind:value={command}
      onkeydown={onKey}
    />
  </form>
</section>

<style>
  .console {
    contain: layout style paint;
    position: relative;
    display: flex;
    flex-direction: column;
    min-block-size: 120px;
    max-block-size: calc(100dvh - 220px);
    background: var(--surface-1);
    border-block-start: 1px solid var(--line);
  }

  .grip {
    position: absolute;
    inset-inline: 0;
    inset-block-start: -4px;
    z-index: 1;
    block-size: 8px;
    cursor: ns-resize;
    touch-action: none;
  }

  .grip::after {
    content: '';
    position: absolute;
    inset-inline: 0;
    inset-block-start: 3px;
    block-size: 2px;
    background: transparent;
    transition: background-color var(--dur) var(--ease);
  }

  .grip:hover::after,
  .grip:active::after,
  .grip:focus-visible::after {
    background: var(--accent-text);
  }

  .grip:focus-visible {
    outline: none;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
    min-block-size: 36px;
    padding: 0 var(--sp-2) 0 var(--sp-4);
    border-block-end: 1px solid var(--line);
  }

  h2 {
    color: var(--text-dim);
    font-size: var(--fs-sm);
    font-weight: 500;
  }

  .tools {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
  }

  .tools :global(.switch-row) {
    margin-inline-end: var(--sp-2);
  }

  .small {
    min-block-size: 28px;
    min-inline-size: 28px;
    font-size: var(--fs-sm);
  }

  .btn-icon.small {
    inline-size: 28px;
  }

  .copy {
    position: relative;
    display: flex;
  }

  /* The bubble is the live region; it stays in the accessibility tree while invisible. */
  .tip {
    position: absolute;
    inset-block-start: 50%;
    inset-inline-end: calc(100% + 6px);
    translate: 0 -50%;
    padding: 4px var(--sp-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--surface-3);
    color: var(--text);
    font-size: var(--fs-sm);
    white-space: nowrap;
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--dur) var(--ease);
  }

  .tip.show {
    opacity: 1;
  }

  .log {
    flex: 1;
    min-block-size: 0;
    padding: var(--sp-2) var(--sp-4);
    font-family: var(--font-mono);
    font-size: var(--fs-sm);
    line-height: 1.5;
    cursor: text;
  }

  .log:focus-visible {
    outline-offset: -2px;
  }

  .banner {
    margin: 0 0 var(--sp-2);
    color: var(--accent-text);
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    line-height: 1.15;
    -webkit-user-select: none;
    user-select: none;
  }

  /* Lines are created from script, so their classes are global within .lines. */
  .lines :global(.ln) {
    color: var(--text-dim);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .lines :global(.ts) {
    margin-inline-end: var(--sp-3);
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
    -webkit-user-select: none;
    user-select: none;
  }

  .lines :global(.tag) {
    margin-inline-end: var(--sp-2);
    font-weight: 600;
  }

  .lines :global(.ln.success) {
    color: var(--ok);
  }

  .lines :global(.ln.warn) {
    color: var(--warn);
  }

  .lines :global(.ln.error) {
    color: var(--err);
  }

  .lines :global(.ln.accent) {
    color: var(--accent-text);
  }

  .prompt {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: 0 var(--sp-4);
    border-block-start: 1px solid var(--line);
    background: var(--bg);
  }

  .caret {
    color: var(--accent-text);
    font-family: var(--font-mono);
    font-weight: 700;
  }

  .prompt input {
    flex: 1;
    min-inline-size: 0;
    min-block-size: 34px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    font-family: var(--font-mono);
    font-size: var(--fs-sm);
  }

  .prompt input::placeholder {
    color: var(--text-faint);
  }

  .prompt input:focus-visible {
    outline: none;
  }

  .prompt:focus-within {
    box-shadow: inset 0 0 0 2px var(--focus);
  }
</style>
