<script lang="ts" generics="T extends string">
  // Two to five exclusive options as a radio group, drawn as a segmented control.
  let {
    label,
    showLabel = false,
    options,
    value,
    onchange,
    small = false,
  }: {
    label: string;
    showLabel?: boolean;
    options: { value: T; label: string }[];
    value: T;
    onchange: (value: T) => void;
    small?: boolean;
  } = $props();

  const uid = $props.id();
</script>

<fieldset class={['seg', small && 'small']}>
  <legend class={showLabel ? 'label' : 'visually-hidden'}>{label}</legend>
  <div class="track">
    {#each options as o (o.value)}
      <label class="opt">
        <input
          type="radio"
          name="{uid}-seg"
          value={o.value}
          checked={value === o.value}
          onchange={() => onchange(o.value)}
        />
        <span>{o.label}</span>
      </label>
    {/each}
  </div>
</fieldset>

<style>
  .seg {
    min-inline-size: 0;
    margin: 0;
    padding: 0;
    border: 0;
  }

  legend {
    padding: 0;
  }

  legend.label {
    margin-block-end: var(--sp-1);
  }

  .track {
    display: grid;
    grid-auto-flow: column;
    /* Equal columns that may shrink below their label: long labels wrap instead of overflowing. */
    grid-auto-columns: minmax(0, 1fr);
    gap: 2px;
    padding: 2px;
    border: 1px solid var(--line-strong);
    border-radius: var(--r-sm);
    background: var(--surface-3);
  }

  .opt {
    position: relative;
    display: flex;
    min-inline-size: 0;
    cursor: pointer;
  }

  /* Visually hidden, still focusable and in the accessibility tree. */
  input {
    position: absolute;
    clip-path: inset(50%);
    overflow: hidden;
    inline-size: 1px;
    block-size: 1px;
    margin: -1px;
    white-space: nowrap;
  }

  span {
    display: flex;
    flex: 1;
    align-items: center;
    justify-content: center;
    min-inline-size: 0;
    min-block-size: 28px;
    padding: 2px var(--sp-3);
    border-radius: 6px;
    color: var(--text-dim);
    font-size: var(--fs-sm);
    font-weight: 500;
    line-height: 1.25;
    text-align: center;
    /* Long labels (ru, de) wrap inside their segment instead of overflowing the card. */
    overflow-wrap: anywhere;
    hyphens: auto;
    transition:
      background-color var(--dur) var(--ease),
      color var(--dur) var(--ease);
  }

  .small span {
    min-block-size: 22px;
    padding: 1px var(--sp-2);
  }

  .opt:hover span {
    color: var(--text);
  }

  input:checked + span {
    background: var(--accent);
    color: var(--on-accent);
  }

  input:focus-visible + span {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }

  @media (forced-colors: active) {
    input:checked + span {
      background: Highlight;
      color: HighlightText;
      forced-color-adjust: none;
    }
  }
</style>
