<script lang="ts" generics="T">
  // A labelled text input that keeps its own draft while the user types and
  // hands back only valid values. The error sentence appears under the field
  // once the browser considers the field :user-invalid (edited, then left).
  import { untrack } from 'svelte';
  import type { HTMLInputAttributes } from 'svelte/elements';
  import type { Parser } from '../parsers';
  import Icon from './Icon.svelte';

  type Props = {
    label: string;
    /** Hide the visible label (table cells, where the column header names the field). */
    bare?: boolean;
    value: T;
    parse: Parser<T>;
    format?: (value: T) => string;
    /** Called with every valid value as the user types. */
    oncommit: (value: T) => void;
    /** Called when the user leaves the field after changing it. */
    ondone?: () => void;
    hint?: string;
    mono?: boolean;
    numeric?: boolean;
    input?: HTMLInputElement;
  } & Omit<HTMLInputAttributes, 'value' | 'oninput' | 'onchange' | 'class'>;

  let {
    label,
    bare = false,
    value,
    parse,
    format = (v: T) => (v == null ? '' : String(v)),
    oncommit,
    ondone,
    hint,
    mono = false,
    numeric = false,
    input = $bindable(),
    ...rest
  }: Props = $props();

  const uid = $props.id();
  let draft = $state(untrack(() => format(value)));
  let error = $state('');
  let focused = false;
  let edited = false;

  function parsedDraft(): { ok: true; value: T } | { ok: false } {
    const r = parse(draft);
    return 'error' in r ? { ok: false } : { ok: true, value: r.value };
  }

  // Validity follows the draft, and anything the parser reads (a neighbouring
  // field for "minimum must be below maximum").
  $effect(() => {
    const r = parse(draft);
    const message = 'error' in r ? r.error : '';
    input?.setCustomValidity(message);
    untrack(() => {
      error = message;
      if (!('error' in r) && edited && !Object.is(r.value, value)) oncommit(r.value);
    });
  });

  // The store changed underneath (revert, another control): adopt it unless
  // the user is in the middle of typing here.
  $effect(() => {
    const text = format(value);
    untrack(() => {
      if (focused) return;
      const current = parsedDraft();
      if (!current.ok || !Object.is(current.value, value)) draft = text;
    });
  });

  function onblur() {
    focused = false;
    const current = parsedDraft();
    if (current.ok) draft = format(current.value);
  }
</script>

<div class="field">
  {#if bare}
    <label class="visually-hidden" for="{uid}-in">{label}</label>
  {:else}
    <label class="label" for="{uid}-in">{label}</label>
  {/if}
  {#if hint}
    <span class="hint" id="{uid}-hint">{hint}</span>
  {/if}
  <input
    {...rest}
    bind:this={input}
    id="{uid}-in"
    class={['input', (mono || numeric) && 'num', mono && 'mono']}
    type={rest.type ?? 'text'}
    inputmode={numeric ? 'decimal' : rest.inputmode}
    autocomplete={rest.autocomplete ?? 'off'}
    autocapitalize="none"
    spellcheck="false"
    aria-invalid="false"
    aria-describedby={hint ? `${uid}-hint` : undefined}
    data-error="{uid}-err"
    value={draft}
    oninput={(e) => {
      // One handler for both: the commit effect must see `edited` with the new draft.
      edited = true;
      draft = e.currentTarget.value;
    }}
    onfocus={() => (focused = true)}
    {onblur}
    onchange={() => ondone?.()}
  />
  <div class="error-msg" id="{uid}-err">
    <span role="img" aria-label="Error"><Icon name="alert" size={14} /></span>
    <span>{error}</span>
  </div>
</div>
