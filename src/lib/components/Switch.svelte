<script lang="ts">
  // Native <input type="checkbox" switch>. Engines without the attribute get
  // role="switch" and thumb dragging from input-switch-polyfill (see main.ts).
  let {
    label,
    hint,
    checked,
    onchange,
    compact = false,
    disabled = false,
  }: {
    label: string;
    hint?: string;
    checked: boolean;
    /** Return a promise of `false` to have the switch snap back when the change is refused. */
    onchange: (checked: boolean) => void | Promise<unknown>;
    /** Label and switch side by side at their natural width (toolbars). */
    compact?: boolean;
    disabled?: boolean;
  } = $props();

  const uid = $props.id();
  // The `switch` attribute is newer than the DOM typings.
  const switchAttr = { switch: '' } as Record<string, string>;
</script>

<label class={['switch-row', compact && 'compact']}>
  <span class="switch-text">
    <span>{label}</span>
    {#if hint}<span class="hint" id="{uid}-hint">{hint}</span>{/if}
  </span>
  <input
    type="checkbox"
    {...switchAttr}
    {checked}
    {disabled}
    aria-describedby={hint ? `${uid}-hint` : undefined}
    onchange={(e) => {
      const el = e.currentTarget;
      const wanted = el.checked;
      const result = onchange(wanted);
      // The control toggled itself; if the engine keeps the old state, show that.
      if (result) {
        void result.then((ok) => {
          if (ok === false) el.checked = checked;
        });
      }
    }}
  />
</label>

<style>
  .compact {
    justify-content: flex-start;
    gap: var(--sp-2);
    color: var(--text-dim);
    font-size: var(--fs-sm);
  }
</style>
