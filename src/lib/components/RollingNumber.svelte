<script lang="ts">
  // A number that rolls to its new value: the old one slides out and the new
  // one slides in from the direction of the change (up for higher, down for
  // lower). Only transform and opacity animate. With reduced motion the
  // numbers only cross-fade.
  import { untrack } from 'svelte';

  let { value, format = String }: { value: number; format?: (n: number) => string } = $props();

  let dir = $state(1);
  let previous = untrack(() => value);

  // Runs before the keyed block below swaps its content, so both the leaving
  // and the entering number see the new direction.
  $effect.pre(() => {
    if (value !== previous) {
      dir = value > previous ? 1 : -1;
      previous = value;
    }
  });

  const reduced = typeof matchMedia === 'function' ? matchMedia('(prefers-reduced-motion: reduce)') : null;

  function roll(_node: Element, { dir, leaving }: { dir: number; leaving: boolean }) {
    return {
      duration: 190,
      // easeOutCubic
      easing: (t: number) => 1 - Math.pow(1 - t, 3),
      css: (t: number) => {
        // Entering comes from below when going up; leaving goes up and out.
        if (reduced?.matches) return `opacity: ${t.toFixed(3)}`;
        const from = leaving ? -dir : dir;
        return `transform: translateY(${((1 - t) * from * 55).toFixed(2)}%); opacity: ${t.toFixed(3)}`;
      },
    };
  }
</script>

<span class="roll" aria-hidden="true">
  {#key value}
    <span class="cell" in:roll={{ dir, leaving: false }} out:roll={{ dir, leaving: true }}>{format(value)}</span>
  {/key}
</span>
<span class="visually-hidden">{format(value)}</span>

<style>
  .roll {
    display: inline-grid;
    overflow: clip;
    /* Room for the glyphs while they slide, without clipping descenders. */
    overflow-clip-margin: 0.08em;
  }

  .cell {
    grid-area: 1 / 1;
    translate: 0;
  }
</style>
