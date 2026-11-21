<script lang="ts">
  // Every modal of the app. `app.dialog` says which one is open.
  import { api } from '../ipc';
  import { app } from '../app.svelte';
  import { t, tp, type Part } from '../i18n/i18n.svelte';
  import Dialog from './Dialog.svelte';
  import Icon from './Icon.svelte';

  const d = $derived(app.dialog);
  const projectName = $derived(app.project.name || t('project.untitled'));

  /** The whole sentence of the unsaved-changes dialog, for what the user was about to do. */
  const unsavedParts = $derived.by((): Part[] => {
    if (d?.kind !== 'unsaved') return [];
    const project = projectName;
    switch (d.action.kind) {
      case 'open':
        return tp('dialogs.unsavedOpen', { project, target: d.action.name });
      case 'openOther':
      case 'pending':
        return tp('dialogs.unsavedOpenOther', { project });
      case 'new':
        return tp('dialogs.unsavedNew', { project });
      case 'update':
        return tp('dialogs.unsavedUpdate', { project });
    }
  });

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
    // Save goes through Save As when the project has no file; cancelling that cancels the action.
    if (result === 'save') void app.save().then((ok) => ok && pending());
    else if (result === 'discard') void pending();
  }

  function onRevert(result: string) {
    close();
    if (result === 'revert') void app.revert();
  }
</script>

<!-- The project name is set in bold wherever the language puts it in the sentence. -->
{#snippet sentence(parts: Part[])}
  {#each parts as part, i (i)}{#if part.param === 'project'}<strong class="strong">{part.text}</strong
      >{:else}{part.text}{/if}{/each}
{/snippet}

<Dialog open={d?.kind === 'quit'} title={t('dialogs.quitTitle')} onclose={onQuit}>
  <p>{@render sentence(tp('dialogs.quitBody', { project: projectName }))}</p>
  {#snippet actions()}
    <button class="btn btn-danger first" value="discard">{t('dialogs.discardQuit')}</button>
    <button class="btn" value="cancel">{t('common.cancel')}</button>
    <!-- svelte-ignore a11y_autofocus -->
    <button class="btn btn-primary" value="save" autofocus>{t('dialogs.saveQuit')}</button>
  {/snippet}
</Dialog>

<Dialog open={d?.kind === 'unsaved'} title={t('dialogs.unsavedTitle')} onclose={onUnsaved}>
  <p>{@render sentence(unsavedParts)}</p>
  {#snippet actions()}
    <button class="btn btn-danger first" value="discard">{t('dialogs.dontSave')}</button>
    <button class="btn" value="cancel">{t('common.cancel')}</button>
    <!-- svelte-ignore a11y_autofocus -->
    <button class="btn btn-primary" value="save" autofocus>{t('common.save')}</button>
  {/snippet}
</Dialog>

<Dialog open={d?.kind === 'revert'} title={t('dialogs.revertTitle')} onclose={onRevert}>
  <p>{@render sentence(tp('dialogs.revertBody', { project: projectName }))}</p>
  {#snippet actions()}
    <!-- svelte-ignore a11y_autofocus -->
    <button class="btn" value="cancel" autofocus>{t('common.cancel')}</button>
    <button class="btn btn-danger" value="revert">{t('dialogs.discardChanges')}</button>
  {/snippet}
</Dialog>

<Dialog open={d?.kind === 'about'} title={app.app.name} onclose={close} wide>
  <p>{t('dialogs.aboutText')}</p>
  <dl class="about">
    <dt>{t('dialogs.version')}</dt>
    <dd class="num selectable">{app.app.version || t('dialogs.versionUnknown')}</dd>
    <dt>{t('dialogs.madeBy')}</dt>
    <dd class="selectable">{app.app.owner}</dd>
    <dt>{t('dialogs.contact')}</dt>
    <dd class="selectable">{app.app.email}</dd>
    <dt>{t('dialogs.license')}</dt>
    <dd>{app.app.license}</dd>
    <dt>{t('dialogs.source')}</dt>
    <dd>
      <button
        type="button"
        class="link repo"
        onclick={() => api.openRepository().catch((e) => app.fail(e, 'errors.openLink'))}
      >
        <span class="selectable">{app.app.repository.replace(/^https?:\/\//, '')}</span>
        <Icon name="external" size={14} />
        <span class="visually-hidden">{t('dialogs.opensInBrowser')}</span>
      </button>
    </dd>
  </dl>
  {#if app.updateCheck.state === 'available'}
    {@const version = app.updateCheck.info.version}
    <p class="update" role="status">
      <span>{t('settings.available', { version })}</span>
      <button
        type="button"
        class="link"
        onclick={(e) => {
          e.currentTarget.closest('dialog')?.close('close');
          app.setView('settings');
        }}
      >
        {t('settings.noticeView')}
      </button>
    </p>
  {/if}
  {#snippet actions()}
    <!-- svelte-ignore a11y_autofocus -->
    <button class="btn btn-primary" value="close" autofocus>{t('common.close')}</button>
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

  .update {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
    color: var(--text);
  }
</style>
