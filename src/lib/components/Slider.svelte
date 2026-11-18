<script lang="ts">
  // Native range input bound to engine state. While the user drags, the slider
  // shows its own value; engine values take over again once they change.
  import type { HTMLInputAttributes } from 'svelte/elements';

  type Props = {
    value: number;
    min: number;
    max: number;
    step?: number;
    /** Text for screen readers, for example "+3 semitones". */
    valueText?: string;
    oninput: (value: number) => void;
    /** The value currently shown: the local one while dragging. */
    shown?: number;
  } & Omit<HTMLInputAttributes, 'value' | 'min' | 'max' | 'step' | 'oninput' | 'type'>;

  let { value, min, max, step = 1, valueText, oninput, shown = $bindable(value), ...rest }: Props = $props();

  let local = $state<number | null>(null);
  let active = false;

  // A new value from the engine ends the local override, unless a drag is in progress.
  $effect(() => {
    void value;
    if (!active) local = null;
  });

  $effect(() => {
    shown = local ?? value;
  });

  const display = $derived(local ?? value);
</script>

<input
  {...rest}
  type="range"
  class={['range', rest.class]}
  {min}
  {max}
  {step}
  value={display}
  aria-valuetext={valueText}
  style:--attr-min={min}
  style:--attr-max={max}
  style:--control-value={display}
  oninput={(e) => {
    active = true;
    local = e.currentTarget.valueAsNumber;
    oninput(local);
  }}
  onchange={() => (active = false)}
/>
