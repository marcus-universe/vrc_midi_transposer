<script lang="ts">
  import { app } from '../app.svelte';
  import { signed } from '../format';
  import { integer, nonEmpty } from '../parsers';
  import Field from '../components/Field.svelte';
  import Switch from '../components/Switch.svelte';

  const p = $derived(app.project);

  const touch = () => app.touch();
  const done = () => void app.flush();

  const LIMIT = 127;
  const outOfRange = (n: number) => (Math.abs(n) > LIMIT ? `Enter a value between −${LIMIT} and ${LIMIT}.` : null);
  const parseMin = $derived(
    integer((n) => outOfRange(n) ?? (n >= p.transpose.max ? 'The minimum must be lower than the maximum.' : null)),
  );
  const parseMax = $derived(
    integer((n) => outOfRange(n) ?? (n <= p.transpose.min ? 'The maximum must be higher than the minimum.' : null)),
  );
</script>

<div class="view">
  <header class="view-head">
    <h1>Settings</h1>
    <p>Settings on this page belong to the project and are saved with it.</p>
  </header>

  <div class="duo">
    <section class="card" aria-labelledby="set-project-title">
      <div class="card-head">
        <h2 class="card-title" id="set-project-title">Project</h2>
      </div>
      <Field
        label="Project name"
        value={p.name}
        parse={nonEmpty('Enter a name for the project.')}
        oncommit={(v) => ((p.name = v), touch())}
        ondone={done}
        required
        maxlength={80}
      />
      <Switch
        label="Start with debug logging"
        hint="Logs every MIDI and OSC message to the console when this project opens."
        checked={p.debug}
        onchange={(on) => {
          p.debug = on;
          app.touch(true);
        }}
      />
    </section>

    <section class="card" aria-labelledby="set-range-title">
      <div class="card-head">
        <h2 class="card-title" id="set-range-title">Transpose range</h2>
        <span class="range num">{signed(p.transpose.min)} to {signed(p.transpose.max)} semitones</span>
      </div>
      <div class="halves">
        <Field
          label="Minimum"
          value={p.transpose.min}
          parse={parseMin}
          oncommit={(v) => ((p.transpose.min = v), touch())}
          ondone={done}
          numeric
          required
        />
        <Field
          label="Maximum"
          value={p.transpose.max}
          parse={parseMax}
          oncommit={(v) => ((p.transpose.max = v), touch())}
          ondone={done}
          numeric
          required
        />
      </div>
      <p class="hint">
        Limits the slider, the console, OSC and MQTT alike. The Home Assistant number entity uses the same range.
      </p>
    </section>
  </div>

  <section class="card" aria-labelledby="set-data-title">
    <div class="card-head">
      <h2 class="card-title" id="set-data-title">Data folder</h2>
    </div>
    <div class="field">
      <label class="label" for="data-dir">Projects and settings are stored in</label>
      <input class="input mono" id="data-dir" type="text" readonly value={app.app.data_dir} />
    </div>
  </section>
</div>

<style>
  .duo {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--sp-4);
    align-items: start;
  }

  @container main (min-width: 700px) {
    .duo {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  .halves {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-3);
    align-items: start;
  }

  .range {
    color: var(--text-dim);
    font-size: var(--fs-sm);
  }
</style>
