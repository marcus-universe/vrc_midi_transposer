<script lang="ts">
  // Pitch and mod wheel, drawn like the wheels on a keyboard: a ridged drum in
  // a slot. The drum surface slides with the value so it looks like it turns;
  // the pitch wheel springs back to the centre because the engine reports it.
  // Updated from the key stream by patching two custom properties, not by
  // re-rendering.
  import { onMount } from 'svelte';
  import { app } from '../app.svelte';
  import { t } from '../i18n/i18n.svelte';

  let pitchEl = $state<HTMLDivElement>();
  let modEl = $state<HTMLDivElement>();
  let lastPitch = -1;
  let lastMod = -1;

  function paint(_keys: readonly number[], _vel: readonly number[], wheels: readonly [number, number]) {
    const [pitch, mod] = wheels;
    if (pitch !== lastPitch && pitchEl) {
      lastPitch = pitch;
      // -1 (down) … 0 (centre) … +1 (up)
      const v = Math.max(-1, Math.min(1, (pitch - 8192) / 8191));
      pitchEl.style.setProperty('--pos', v.toFixed(3));
      pitchEl.setAttribute('aria-valuenow', String(Math.round(v * 100)));
    }
    if (mod !== lastMod && modEl) {
      lastMod = mod;
      // Mapped to -1 (wheel down, 0) … +1 (wheel up, 127)
      modEl.style.setProperty('--pos', ((mod / 127) * 2 - 1).toFixed(3));
      modEl.setAttribute('aria-valuenow', String(mod));
    }
  }

  onMount(() => {
    paint(app.keys, app.vel, app.wheels);
    return app.onKeys(paint);
  });
</script>

<div class="wheels motion-safe">
  <div
    class="wheel pitch"
    bind:this={pitchEl}
    role="meter"
    aria-label={t('piano.pitchWheel')}
    aria-valuemin={-100}
    aria-valuemax={100}
    aria-valuenow={0}
    title={t('piano.pitchWheel')}
  >
    <span class="drum"><span class="surface"></span></span>
  </div>
  <div
    class="wheel mod"
    bind:this={modEl}
    role="meter"
    aria-label={t('piano.modWheel')}
    aria-valuemin={0}
    aria-valuemax={127}
    aria-valuenow={0}
    title={t('piano.modWheel')}
  >
    <span class="drum"><span class="surface"></span></span>
  </div>
</div>

<style>
  .wheels {
    display: flex;
    gap: 6px;
    padding: 4px 6px;
    border-radius: 6px;
    background: var(--surface-3);
    box-shadow: inset 0 1px 3px rgba(0, 0, 0, 0.45);
  }

  .wheel {
    --pos: 0;
    /* How far the surface travels between the two ends of the wheel. */
    --travel: 26px;
    position: relative;
    inline-size: 22px;
    border-radius: 5px;
    background: var(--key-black);
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.6);
    overflow: hidden;
  }

  .drum {
    position: absolute;
    inset: 2px;
    border-radius: 4px;
    overflow: hidden;
    background: #2b3346;
  }

  /* The ridged surface is taller than the drum and slides with the value. */
  .surface {
    position: absolute;
    inset-inline: 0;
    inset-block: -40px;
    translate: 0 calc(var(--pos) * var(--travel) * -1);
    /* Smooths the steps between MIDI values; it shows the wheel, so it stays on with reduced motion. */
    transition: translate 70ms linear;
    background:
      /* the marker line, at the centre of the surface */
      linear-gradient(to bottom, transparent calc(50% - 1.5px), var(--accent-text) calc(50% - 1.5px), var(--accent-text) calc(50% + 1.5px), transparent calc(50% + 1.5px)),
      /* the ridges */
      repeating-linear-gradient(to bottom, #4a556e 0 2px, #2f374a 2px 5px);

  }

  /* Shading that makes the flat surface read as a cylinder. Static: only the surface moves. */
  .drum::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(
      to bottom,
      rgba(0, 0, 0, 0.75),
      rgba(0, 0, 0, 0.15) 30%,
      rgba(255, 255, 255, 0.08) 50%,
      rgba(0, 0, 0, 0.15) 70%,
      rgba(0, 0, 0, 0.75)
    );
    pointer-events: none;
  }

  /* The pitch wheel has a centre notch on the housing. */
  .pitch::before {
    content: '';
    position: absolute;
    inset-inline-start: -1px;
    inset-block-start: calc(50% - 2px);
    z-index: 1;
    inline-size: 3px;
    block-size: 4px;
    border-radius: 0 2px 2px 0;
    background: var(--text-faint);
  }

  @media (forced-colors: active) {
    .surface {
      forced-color-adjust: none;
      background: linear-gradient(to bottom, Canvas calc(50% - 2px), Highlight calc(50% - 2px), Highlight calc(50% + 2px), Canvas calc(50% + 2px));
    }
  }
</style>
