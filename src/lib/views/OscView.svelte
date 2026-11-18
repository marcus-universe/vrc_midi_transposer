<script lang="ts">
  import { tick } from 'svelte';
  import { app } from '../app.svelte';
  import { DEFAULT_LISTEN_PATHS, clamp, slug } from '../format';
  import { OSC_ROLES, type OscRole, type SendAddress } from '../ipc';
  import { float, host, nonEmpty, optionalFloat, oscPath, port } from '../parsers';
  import Field from '../components/Field.svelte';
  import Icon from '../components/Icon.svelte';
  import Segmented from '../components/Segmented.svelte';
  import Switch from '../components/Switch.svelte';

  const osc = $derived(app.project.osc);
  const s = $derived(app.snapshot);

  const touch = () => app.touch();
  const done = () => void app.flush();

  let listenTable = $state<HTMLTableElement>();
  let controlTable = $state<HTMLTableElement>();

  async function focusLastRow(table: HTMLTableElement | undefined) {
    await tick();
    const input = table?.querySelector<HTMLInputElement>('tbody tr:last-child input.input');
    input?.focus();
    input?.select();
  }

  // --- Listening addresses -------------------------------------------------
  const roleHint = (role: OscRole | null) =>
    role ? (OSC_ROLES.find((r) => r.role === role)?.hint ?? '') : 'Ignored';

  const missingDefaults = $derived(
    OSC_ROLES.filter((r) => !osc.listening_addresses.some((a) => a.role === r.role)).map((r) => r.role),
  );

  function addAddress() {
    let n = 1;
    while (osc.listening_addresses.some((a) => a.addr === `/address${n}`)) n++;
    osc.listening_addresses.push({ addr: `/address${n}`, role: null });
    app.touch(true);
    void focusLastRow(listenTable);
  }

  function removeAddress(i: number) {
    osc.listening_addresses.splice(i, 1);
    app.touch(true);
  }

  function restoreDefaults() {
    for (const role of missingDefaults) {
      osc.listening_addresses.push({ addr: DEFAULT_LISTEN_PATHS[role], role });
    }
    app.touch(true);
  }

  // --- Custom controls -----------------------------------------------------
  const controls = $derived(osc.sending_addresses);

  /** Slugs used by more than one control: their MQTT topics would collide. */
  const clashes = $derived.by(() => {
    const count = new Map<string, number>();
    for (const c of controls) count.set(slug(c.name), (count.get(slug(c.name)) ?? 0) + 1);
    return [...count].filter(([, n]) => n > 1).map(([name]) => name);
  });

  function addControl() {
    let n = controls.length + 1;
    while (controls.some((c) => c.name === `Control ${n}`)) n++;
    controls.push({
      name: `Control ${n}`,
      addr: `/avatar/parameters/Control${n}`,
      type: 'bool',
      default: 0,
      min: null,
      max: null,
    });
    app.touch(true);
    void focusLastRow(controlTable);
  }

  function removeControl(i: number) {
    controls.splice(i, 1);
    app.touch(true);
  }

  function setType(c: SendAddress, type: 'bool' | 'float') {
    if (c.type === type) return;
    c.type = type;
    if (type === 'bool') {
      c.default = c.default >= 0.5 ? 1 : 0;
      c.min = null;
      c.max = null;
    } else {
      c.min = 0;
      c.max = 1;
      c.default = clamp(c.default, 0, 1);
    }
    app.touch(true);
  }

  const inRange = (c: SendAddress) => (n: number) =>
    (c.min !== null && n < c.min) || (c.max !== null && n > c.max)
      ? 'The default must lie between the minimum and the maximum.'
      : null;
  const belowMax = (c: SendAddress) => (n: number) =>
    c.max !== null && n >= c.max ? 'The minimum must be lower than the maximum.' : null;
  const aboveMin = (c: SendAddress) => (n: number) =>
    c.min !== null && n <= c.min ? 'The maximum must be higher than the minimum.' : null;
</script>

<div class="view">
  <header class="view-head">
    <h1>OSC</h1>
    <p>Receive commands from VRChat or other OSC programs, and send pressed keys and custom values to your avatar.</p>
  </header>

  <div class="trio">
    <section class="card" aria-labelledby="osc-listener-title">
      <div class="card-head">
        <h2 class="card-title" id="osc-listener-title">Listener</h2>
        <span class="state">
          <span class={['dot', s.osc_listener.state === 'running' ? 'ok' : s.osc_listener.state === 'error' ? 'err' : '']}></span>
          {s.osc_listener.state === 'running' ? 'Running' : s.osc_listener.state === 'error' ? 'Error' : 'Stopped'}
        </span>
      </div>
      <div class="pair">
        <Field
          label="Host"
          value={osc.listen_host}
          parse={host}
          oncommit={(v) => ((osc.listen_host = v), touch())}
          ondone={done}
          mono
          required
        />
        <Field
          label="Port"
          value={osc.listen_port}
          parse={port}
          oncommit={(v) => ((osc.listen_port = v), touch())}
          ondone={done}
          numeric
          required
        />
      </div>
      {#if s.osc_listener.state === 'error' && s.osc_listener.detail}
        <p class="problem"><Icon name="alert" size={14} /><span>{s.osc_listener.detail}</span></p>
      {:else}
        <p class="hint">The address this app receives OSC messages on (UDP).</p>
      {/if}
    </section>

    <section class="card" aria-labelledby="osc-target-title">
      <div class="card-head">
        <h2 class="card-title" id="osc-target-title">Target</h2>
      </div>
      <div class="pair">
        <Field
          label="Host"
          value={osc.target_host}
          parse={host}
          oncommit={(v) => ((osc.target_host = v), touch())}
          ondone={done}
          mono
          required
        />
        <Field
          label="Port"
          value={osc.target_port}
          parse={port}
          oncommit={(v) => ((osc.target_port = v), touch())}
          ondone={done}
          numeric
          required
        />
      </div>
      <p class="hint">Where notes and custom values are sent. VRChat listens on port 9000.</p>
    </section>

    <section class="card" aria-labelledby="osc-startup-title">
      <div class="card-head">
        <h2 class="card-title" id="osc-startup-title">When this project opens</h2>
      </div>
      <Switch
        label="Start with OSC sending on"
        checked={osc.sending_enabled}
        onchange={(on) => {
          osc.sending_enabled = on;
          app.touch(true);
        }}
      />
      <Segmented
        showLabel
        label="Start by sending"
        options={[
          { value: 'original', label: 'Original notes' },
          { value: 'transposed', label: 'Transposed notes' },
        ]}
        value={osc.send_original ? 'original' : 'transposed'}
        onchange={(v) => {
          osc.send_original = v === 'original';
          app.touch(true);
        }}
      />
    </section>
  </div>

  <section class="card" aria-labelledby="osc-listen-title">
    <div class="card-head">
      <h2 class="card-title" id="osc-listen-title">Listening addresses</h2>
      <div class="card-actions">
        <button
          type="button"
          class="btn btn-ghost"
          disabled={!missingDefaults.length}
          title={missingDefaults.length ? 'Add the standard addresses that are missing' : 'All seven standard actions have an address'}
          onclick={restoreDefaults}
        >
          Restore defaults
        </button>
      </div>
    </div>

    {#if osc.listening_addresses.length}
      <table class="table" bind:this={listenTable} aria-labelledby="osc-listen-title">
        <thead>
          <tr>
            <th scope="col" class="col-role">Action</th>
            <th scope="col">OSC address</th>
            <th scope="col" class="col-hint">Expects</th>
            <th scope="col" class="col-x"><span class="visually-hidden">Remove</span></th>
          </tr>
        </thead>
        <tbody>
          {#each osc.listening_addresses as a, i (a)}
            <tr>
              <td>
                <select
                  class="select"
                  name="listen-role-{i}"
                  aria-label="Action for address {i + 1}"
                  value={a.role ?? ''}
                  onchange={(e) => {
                    a.role = (e.currentTarget.value || null) as OscRole | null;
                    app.touch(true);
                  }}
                >
                  {#each OSC_ROLES as r (r.role)}
                    <option value={r.role}>{r.label}</option>
                  {/each}
                  <option value="">No action</option>
                </select>
              </td>
              <td>
                <Field
                  bare
                  label="OSC address {i + 1}"
                  value={a.addr}
                  parse={oscPath}
                  oncommit={(v) => ((a.addr = v), touch())}
                  ondone={done}
                  mono
                  required
                  placeholder="/address"
                />
              </td>
              <td><span class="cell-text">{roleHint(a.role)}</span></td>
              <td>
                <button
                  type="button"
                  class="btn btn-ghost btn-icon"
                  aria-label="Remove address {a.addr || i + 1}"
                  onclick={() => removeAddress(i)}
                >
                  <Icon name="trash" size={16} />
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <p class="empty">No listening addresses, so incoming OSC messages do nothing.</p>
    {/if}

    <div class="under">
      <button type="button" class="btn" onclick={addAddress}>
        <Icon name="plus" size={16} />
        Add address
      </button>
      <span class="hint">Several addresses may share one action. Only the first argument of a message is read.</span>
    </div>
  </section>

  <section class="card" aria-labelledby="osc-controls-title">
    <div class="card-head">
      <h2 class="card-title" id="osc-controls-title">Custom controls</h2>
    </div>

    {#if controls.length}
      <table class="table" bind:this={controlTable} aria-labelledby="osc-controls-title">
        <thead>
          <tr>
            <th scope="col" class="col-name">Name</th>
            <th scope="col">OSC address</th>
            <th scope="col" class="col-type">Type</th>
            <th scope="col" class="col-num">Default</th>
            <th scope="col" class="col-num">Min</th>
            <th scope="col" class="col-num">Max</th>
            <th scope="col" class="col-x"><span class="visually-hidden">Remove</span></th>
          </tr>
        </thead>
        <tbody>
          {#each controls as c, i (c)}
            <tr>
              <td>
                <Field
                  bare
                  label="Name of control {i + 1}"
                  value={c.name}
                  parse={nonEmpty('Enter a name. It labels the control here and in Home Assistant.')}
                  oncommit={(v) => ((c.name = v), touch())}
                  ondone={done}
                  required
                />
              </td>
              <td>
                <Field
                  bare
                  label="OSC address of {c.name || `control ${i + 1}`}"
                  value={c.addr}
                  parse={oscPath}
                  oncommit={(v) => ((c.addr = v), touch())}
                  ondone={done}
                  mono
                  required
                  placeholder="/avatar/parameters/Name"
                />
              </td>
              <td>
                <select
                  class="select"
                  name="control-type-{i}"
                  aria-label="Type of {c.name || `control ${i + 1}`}"
                  value={c.type}
                  onchange={(e) => setType(c, e.currentTarget.value as 'bool' | 'float')}
                >
                  <option value="bool">Switch</option>
                  <option value="float">Slider</option>
                </select>
              </td>
              {#if c.type === 'bool'}
                <td>
                  <select
                    class="select"
                    name="control-default-{i}"
                    aria-label="Default of {c.name || `control ${i + 1}`}"
                    value={c.default >= 0.5 ? '1' : '0'}
                    onchange={(e) => {
                      c.default = Number(e.currentTarget.value);
                      app.touch(true);
                    }}
                  >
                    <option value="0">Off</option>
                    <option value="1">On</option>
                  </select>
                </td>
                {#each ['Minimum', 'Maximum'] as what (what)}
                  <td>
                    <span class="cell-text">
                      <span aria-hidden="true">–</span>
                      <span class="visually-hidden">{what} not used for a switch</span>
                    </span>
                  </td>
                {/each}
              {:else}
                <td>
                  <Field
                    bare
                    label="Default of {c.name || `control ${i + 1}`}"
                    value={c.default}
                    parse={float(inRange(c))}
                    oncommit={(v) => ((c.default = v), touch())}
                    ondone={done}
                    numeric
                    required
                  />
                </td>
                <td>
                  <Field
                    bare
                    label="Minimum of {c.name || `control ${i + 1}`}"
                    value={c.min}
                    parse={optionalFloat(belowMax(c))}
                    oncommit={(v) => ((c.min = v), touch())}
                    ondone={done}
                    numeric
                    placeholder="0"
                  />
                </td>
                <td>
                  <Field
                    bare
                    label="Maximum of {c.name || `control ${i + 1}`}"
                    value={c.max}
                    parse={optionalFloat(aboveMin(c))}
                    oncommit={(v) => ((c.max = v), touch())}
                    ondone={done}
                    numeric
                    placeholder="1"
                  />
                </td>
              {/if}
              <td>
                <button
                  type="button"
                  class="btn btn-ghost btn-icon"
                  aria-label="Remove control {c.name || i + 1}"
                  onclick={() => removeControl(i)}
                >
                  <Icon name="trash" size={16} />
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <p class="empty">
        No custom controls. A control sends one value to an avatar parameter and also appears in Home Assistant.
      </p>
    {/if}

    {#if clashes.length}
      <p class="warning" role="status">
        <Icon name="alert" size={14} />
        <span>
          Warning: two controls share the MQTT name
          {#each clashes as name, i (name)}{i ? ', ' : ''}<code>{name}</code>{/each}. Rename one, or they will use the
          same MQTT topic and Home Assistant entity.
        </span>
      </p>
    {/if}

    <div class="under">
      <button type="button" class="btn" onclick={addControl}>
        <Icon name="plus" size={16} />
        Add control
      </button>
      <span class="hint">
        A switch sends 0 or 1. A slider sends a decimal number within its range (0 to 1 when the range is empty).
      </span>
    </div>
  </section>
</div>

<style>
  .trio {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--sp-4);
  }

  @container main (min-width: 600px) {
    .trio {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .trio > :last-child {
      grid-column: 1 / -1;
    }
  }

  @container main (min-width: 840px) {
    .trio {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
    .trio > :last-child {
      grid-column: auto;
    }
  }

  .pair {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 92px;
    gap: var(--sp-3);
    align-items: start;
  }

  .state {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-dim);
    font-size: var(--fs-sm);
  }

  .problem,
  .warning {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    font-size: var(--fs-sm);
    line-height: 1.4;
  }

  .problem {
    color: var(--err);
  }

  .warning {
    padding: var(--sp-2) var(--sp-3);
    border: 1px solid var(--warn);
    border-radius: var(--r-sm);
    background: var(--warn-soft);
    color: var(--text);
  }

  .problem :global(svg),
  .warning :global(svg) {
    margin-block-start: 2px;
  }

  .warning :global(svg) {
    color: var(--warn);
  }

  code {
    font-family: var(--font-mono);
    font-size: var(--fs-sm);
  }

  .col-role {
    inline-size: 230px;
  }

  .col-hint {
    inline-size: 170px;
  }

  .col-x {
    inline-size: 40px;
  }

  .col-name {
    inline-size: 23%;
  }

  .col-type {
    inline-size: 112px;
  }

  .col-num {
    inline-size: 84px;
  }

  .under {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-4);
  }
</style>
