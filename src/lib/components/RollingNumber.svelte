<script lang="ts">
  // A number that slides to its new value: the old one slides out and the
  // next one slides in, up when the value increases and down when it
  // decreases. A jump of more than one counts through every value on the way,
  // quickly: each step is one slide of min(150, 400 / distance) ms, so the
  // whole count takes about 400 ms at most. A new target that arrives while
  // counting continues from the number on screen.
  //
  // This always slides, also with prefers-reduced-motion: the owner of the app
  // wants the transpose number to move in every case. It uses the Web
  // Animations API on transform and opacity only, which the reduced-motion
  // rule in app.css (CSS transitions and animations) does not touch.
  //
  // Screen readers get the final value only, from the hidden text below.
  import { onDestroy, tick, untrack } from 'svelte';

  let { value, format = String }: { value: number; format?: (n: number) => string } = $props();

  /** How far a number travels, in percent of its own height. */
  const TRAVEL = 80;
  const MAX_STEP_MS = 150;
  const MAX_TOTAL_MS = 400;

  /** The number at rest on screen. */
  let current = $state(untrack(() => value));
  /** The number sliding in during a step. */
  let incoming = $state(untrack(() => value));

  let curEl = $state<HTMLSpanElement>();
  let nextEl = $state<HTMLSpanElement>();
  let running = false;
  let destroyed = false;
  let live: Animation[] = [];

  $effect(() => {
    void value;
    untrack(() => {
      if (!running) void count();
    });
  });

  async function count() {
    running = true;
    let target = NaN;
    let stepMs = MAX_STEP_MS;
    // When the current step should end. Steps are timed against this schedule,
    // so the few milliseconds each step loses to frame boundaries do not add up.
    let due = 0;
    try {
      while (!destroyed && current !== value) {
        // A new target: spread the remaining distance over the time budget
        // again, but never slower than a count that is already running.
        if (value !== target) {
          const fit = Math.min(MAX_STEP_MS, MAX_TOTAL_MS / Math.abs(value - current));
          stepMs = Number.isNaN(target) ? fit : Math.min(stepMs, fit);
          target = value;
          due = performance.now();
        }
        due += stepMs;
        const duration = Math.max(10, due - performance.now());
        const dir = target > current ? 1 : -1;
        incoming = current + dir;
        await tick();
        if (!curEl || !nextEl || typeof curEl.animate !== 'function') {
          current = value;
          break;
        }
        // One slide alone eases out; inside a count the steps run evenly.
        const easing = stepMs < MAX_STEP_MS ? 'linear' : 'cubic-bezier(0.2, 0.7, 0.3, 1)';
        const timing: KeyframeAnimationOptions = { duration, easing, fill: 'forwards' };
        // Up: the old number leaves upwards and the new one rises from below.
        const out = curEl.animate(
          [
            { transform: 'translateY(0)', opacity: 1 },
            { transform: `translateY(${-dir * TRAVEL}%)`, opacity: 0 },
          ],
          timing,
        );
        const into = nextEl.animate(
          [
            { transform: `translateY(${dir * TRAVEL}%)`, opacity: 0 },
            { transform: 'translateY(0)', opacity: 1 },
          ],
          timing,
        );
        live = [out, into];
        await Promise.all([out.finished, into.finished]).catch(() => {});
        if (destroyed) break;
        current = incoming;
        // Drop the held end state once the resting cell shows the new number,
        // in the same task, so no frame shows the old one.
        await tick();
        out.cancel();
        into.cancel();
        live = [];
      }
    } finally {
      running = false;
    }
  }

  onDestroy(() => {
    destroyed = true;
    for (const a of live) a.cancel();
  });
</script>

<span class="roll motion-safe" aria-hidden="true">
  <span class="cell" bind:this={curEl}>{format(current)}</span>
  <span class="cell next" bind:this={nextEl}>{format(incoming)}</span>
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
    font-variant-numeric: tabular-nums;
  }

  /* At rest only the current number shows; the animation brings this one in. */
  .next {
    opacity: 0;
  }
</style>
