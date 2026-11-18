<script lang="ts">
  // Every modal of the app. `app.dialog` says which one is open.
  import { tick, untrack } from 'svelte';
  import { api } from '../ipc';
  import { app } from '../app.svelte';
  import { nonEmpty } from '../parsers';
  import Dialog from './Dialog.svelte';
  import Field from './Field.svelte';
  import Icon from './Icon.svelte';

  const d = $derived(app.dialog);
  const projectName = $derived(app.project.name || 'this project');

  // --- Name (new / duplicate) ---------------------------------------------
  let name = $state('');
  let nameInput = $state<HTMLInputElement>();
  let nameRound = $state(0);
  const nameDialog = $derived(d?.kind === 'name' ? d : null);

  $effect(() => {
    if (!nameDialog) return;
    name = nameDialog.initial;
    // A fresh field per round; select its text so typing replaces the suggestion.
    untrack(() => nameRound++);
    void tick().then(() => nameInput?.select());
  });

  const nameTaken = $derived(app.projects.some((p) => p.name === name.trim()));

  function close() {
    app.dialog = null;
  }

  function onQuit(result: string) {
    close();
    if (result === 'save') void app.saveAndQuit();
    else if (result === 'discard') void app.discardAndQuit();
  }

  function onUnsaved(result: string) {
    const pending = d?.kind === 'unsaved' ? d.proceed : null;
    close();
    if (!pending) return;
    if (result === 'save') void app.save().then((ok) => ok && pending());
    else if (result === 'discard') void pending();
  }

  function onRevert(result: string) {
    close();
    if (result === 'revert') void app.revert();
  }

  function onName(result: string) {
    const mode = nameDialog?.mode;
    close();
    if (result === 'ok' && mode && name.trim()) void app.createProject(mode, name.trim());
  }

  function onDelete(result: string) {
    close();
    if (result === 'delete') void app.deleteProject();
  }
</script>

<Dialog open={d?.kind === 'quit'} title="Save changes before quitting?" onclose={onQuit}>
  <p>
    <strong class="strong">{projectName}</strong> has changes that are not saved. If you quit without saving, they
    are lost.
  </p>
  {#snippet actions()}
    <button class="btn btn-danger first" value="discard">Discard and quit</button>
    <button class="btn" value="cancel">Cancel</button>
    <!-- svelte-ignore a11y_autofocus -->
    <button class="btn btn-primary" value="save" autofocus>Save and quit</button>
  {/snippet}
</Dialog>

<Dialog open={d?.kind === 'unsaved'} title="Save changes first?" onclose={onUnsaved}>
  <p>
    <strong class="strong">{projectName}</strong> has changes that are not saved. Save them before you
    {d?.kind === 'unsaved' ? d.action : 'continue'}?
  </p>
  {#snippet actions()}
    <button class="btn btn-danger first" value="discard">Discard changes</button>
    <button class="btn" value="cancel">Cancel</button>
    <!-- svelte-ignore a11y_autofocus -->
    <button class="btn btn-primary" value="save" autofocus>Save and continue</button>
  {/snippet}
</Dialog>

<Dialog open={d?.kind === 'revert'} title="Discard unsaved changes?" onclose={onRevert}>
  <p>
    <strong class="strong">{projectName}</strong> goes back to the way it was last saved. This cannot be undone.
  </p>
  {#snippet actions()}
    <!-- svelte-ignore a11y_autofocus -->
    <button class="btn" value="cancel" autofocus>Cancel</button>
    <button class="btn btn-danger" value="revert">Discard changes</button>
  {/snippet}
</Dialog>

<Dialog
  open={d?.kind === 'name'}
  title={nameDialog?.mode === 'duplicate' ? 'Duplicate project' : 'New project'}
  onclose={onName}
>
  {#if nameDialog?.mode === 'duplicate'}
    <p>Saves the current settings, including unsaved changes, as a new project and switches to it.</p>
  {:else}
    <p>Starts from the default settings: no MIDI ports, the seven standard OSC addresses, MQTT off.</p>
  {/if}
  {#key nameRound}
    <!-- svelte-ignore a11y_autofocus -->
    <Field
      label="Project name"
      value={name}
      parse={nonEmpty('Enter a name for the project.')}
      oncommit={(v) => (name = v)}
      bind:input={nameInput}
      required
      autofocus
      maxlength={80}
    />
  {/key}
  <p class="note" aria-live="polite">
    {#if nameTaken}Another project already has this name. You can still use it.{/if}
  </p>
  {#snippet actions()}
    <!-- Not a submit button: Enter in the name field must reach the button after it. -->
    <button type="button" class="btn" onclick={(e) => e.currentTarget.closest('dialog')?.close('cancel')}>
      Cancel
    </button>
    <button class="btn btn-primary" value="ok">
      {nameDialog?.mode === 'duplicate' ? 'Duplicate' : 'Create project'}
    </button>
  {/snippet}
</Dialog>

<Dialog open={d?.kind === 'delete'} title="Delete “{projectName}”?" onclose={onDelete}>
  <p>The project file is removed from disk. This cannot be undone.</p>
  {#snippet actions()}
    <!-- svelte-ignore a11y_autofocus -->
    <button class="btn" value="cancel" autofocus>Cancel</button>
    <button class="btn btn-danger" value="delete">Delete project</button>
  {/snippet}
</Dialog>

<Dialog open={d?.kind === 'about'} title={app.app.name} onclose={close} wide>
  <p>
    Forwards MIDI with live transposition, mirrors pressed keys to a VRChat avatar over OSC, and exposes its
    controls to Home Assistant over MQTT.
  </p>
  <dl class="about">
    <dt>Version</dt>
    <dd class="num selectable">{app.app.version || 'unknown'}</dd>
    <dt>Made by</dt>
    <dd class="selectable">{app.app.owner}</dd>
    <dt>Contact</dt>
    <dd class="selectable">{app.app.email}</dd>
    <dt>License</dt>
    <dd>{app.app.license}</dd>
    <dt>Source</dt>
    <dd>
      <button type="button" class="link repo" onclick={() => api.openRepository().catch((e) => app.fail(e))}>
        <span class="selectable">{app.app.repository.replace(/^https?:\/\//, '')}</span>
        <Icon name="external" size={14} />
        <span class="visually-hidden">(opens in your browser)</span>
      </button>
    </dd>
  </dl>
  {#snippet actions()}
    <!-- svelte-ignore a11y_autofocus -->
    <button class="btn btn-primary" value="close" autofocus>Close</button>
  {/snippet}
</Dialog>

<style>
  .strong {
    color: var(--text);
    font-weight: 600;
  }

  /* Keeps the destructive choice away from the default one. */
  :global(.btn.first) {
    margin-inline-end: auto;
  }

  .note {
    min-block-size: 1.45em;
    color: var(--text-dim);
    font-size: var(--fs-sm);
  }

  .about {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--sp-2) var(--sp-5);
    margin: 0;
    padding: var(--sp-3) var(--sp-4);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--surface-1);
  }

  .about dt {
    color: var(--text-dim);
    font-size: var(--fs-sm);
    line-height: 1.6;
  }

  .about dd {
    min-inline-size: 0;
    color: var(--text);
    overflow-wrap: anywhere;
  }

  .repo {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    text-align: start;
  }
</style>
