<script lang="ts">
  import { app } from '../app.svelte';
  import { formatBytes, formatDate, relativeTime, signed } from '../format';
  import { LANGUAGE_NAMES, LOCALES, formatNumber, i18n, isLocale, t } from '../i18n/i18n.svelte';
  import { integer, nonEmpty } from '../parsers';
  import Field from '../components/Field.svelte';
  import Icon from '../components/Icon.svelte';
  import Switch from '../components/Switch.svelte';

  const p = $derived(app.project);

  const touch = () => app.touch();
  const done = () => void app.flush();

  const LIMIT = 127;
  const outOfRange = (n: number) =>
    Math.abs(n) > LIMIT ? t('settings.outOfRange', { min: signed(-LIMIT), max: formatNumber(LIMIT) }) : null;
  const parseMin = $derived(
    integer((n) => outOfRange(n) ?? (n >= p.transpose.max ? t('errors.minBelowMax') : null)),
  );
  const parseMax = $derived(
    integer((n) => outOfRange(n) ?? (n <= p.transpose.min ? t('errors.maxAboveMin') : null)),
  );

  /** The saved choice, or the language in use when none was made yet. */
  const language = $derived(app.ui.language ?? i18n.locale);

  // --- Updates ---------------------------------------------------------------
  const mode = $derived(app.app.update_mode);
  const check = $derived(app.updateCheck);
  const progress = $derived(app.updateProgress);
  const installing = $derived(progress !== null && progress.state !== 'error');

  /** What the live region says: the state, never the byte counts. */
  const spoken = $derived.by(() => {
    if (progress?.state === 'downloading') return t('settings.downloading');
    if (progress?.state === 'installing') return t('settings.installing');
    if (progress?.state === 'error') return t('settings.failed', { detail: progress.detail });
    switch (check.state) {
      case 'checking':
        return t('settings.checking');
      case 'current':
        return t('settings.upToDate');
      case 'available':
        return t('settings.available', { version: check.info.version });
      case 'error':
        return t('errors.updateCheck', { detail: check.detail });
      default:
        return '';
    }
  });
</script>

<div class="view">
  <header class="view-head">
    <h1>{t('settings.title')}</h1>
    <p>{t('settings.intro')}</p>
  </header>

  <section class="card" aria-labelledby="set-app-title" aria-describedby="set-app-note">
    <div class="card-head">
      <h2 class="card-title" id="set-app-title">{t('settings.app')}</h2>
      <span class="aside" id="set-app-note">{t('settings.appNote')}</span>
    </div>
    <div class="field language">
      <label class="label" for="app-language">{t('settings.language')}</label>
      <select
        class="select"
        id="app-language"
        name="language"
        value={language}
        onchange={(e) => {
          const code = e.currentTarget.value;
          if (isLocale(code)) void app.setLanguage(code);
        }}
      >
        {#each LOCALES as code (code)}
          <option value={code} lang={code === 'zh' ? 'zh-Hans' : code}>{LANGUAGE_NAMES[code]}</option>
        {/each}
      </select>
    </div>

    <hr class="divider" />

    <section class="updates" aria-labelledby="set-updates-title">
      <div class="updates-head">
        <h3 class="sub-title" id="set-updates-title">{t('settings.updates')}</h3>
        <span class="aside num selectable">{t('settings.version', { version: app.app.version || '?' })}</span>
      </div>

      {#if mode === 'none'}
        <p class="hint">{t('settings.devBuild')}</p>
      {/if}

      <div class="row">
        <button
          type="button"
          class="btn"
          disabled={mode === 'none' || check.state === 'checking' || installing}
          onclick={() => app.checkUpdate(true)}
        >
          <Icon name="refresh" size={16} />
          {check.state === 'checking' ? t('settings.checking') : t('settings.check')}
        </button>
        {#if app.ui.last_update_check && mode !== 'none'}
          <span class="hint">{t('settings.lastChecked', { time: relativeTime(app.ui.last_update_check, app.now) })}</span>
        {/if}
      </div>

      <!-- One short sentence per change is announced; the byte counts are not. -->
      <p class="visually-hidden" role="status">{spoken}</p>
      <div class="result">
        {#if check.state === 'current'}
          <p class="line"><Icon name="check" size={16} /><span>{t('settings.upToDate')}</span></p>
        {:else if check.state === 'error'}
          <p class="line problem">
            <Icon name="alert" size={16} /><span>{t('errors.updateCheck', { detail: check.detail })}</span>
          </p>
        {:else if check.state === 'available'}
          {@const info = check.info}
          <div class="found">
            <p class="line strong">
              <Icon name="download" size={16} />
              <span>{t('settings.available', { version: info.version })}</span>
              {#if info.date}<span class="hint">{t('settings.released', { date: formatDate(info.date) })}</span>{/if}
            </p>
            {#if info.notes.trim()}
              <!-- Plain text: rendered as a text node, line breaks kept by CSS. -->
              <div class="notes">
                <span class="label">{t('settings.notes')}</span>
                <p class="notes-text selectable">{info.notes.trim()}</p>
              </div>
            {/if}

            {#if progress && progress.state === 'downloading'}
              <div class="progress">
                {#if progress.total}
                  <label class="label" for="update-progress">{t('settings.downloading')}</label>
                  <progress id="update-progress" max={progress.total} value={progress.downloaded}></progress>
                  <span class="hint num">
                    {t('settings.progress', { done: formatBytes(progress.downloaded), total: formatBytes(progress.total) })}
                  </span>
                {:else}
                  <!-- Size unknown: words instead of an endlessly moving bar. -->
                  <span class="label">{t('settings.downloading')}</span>
                  <span class="hint num">{t('settings.progressUnknown', { done: formatBytes(progress.downloaded) })}</span>
                {/if}
              </div>
            {:else if progress && progress.state === 'installing'}
              <p class="line"><Icon name="download" size={16} /><span>{t('settings.installing')}</span></p>
            {:else}
              {#if progress && progress.state === 'error'}
                <p class="line problem">
                  <Icon name="alert" size={16} /><span>{t('settings.failed', { detail: progress.detail })}</span>
                </p>
              {/if}
              <div class="row">
                {#if mode === 'auto'}
                  <button type="button" class="btn btn-primary" disabled={app.busy} onclick={() => app.installUpdate()}>
                    {progress?.state === 'error' ? t('settings.retry') : t('settings.install')}
                  </button>
                {/if}
                <button
                  type="button"
                  class={['btn', mode === 'auto' && 'btn-ghost']}
                  onclick={() => app.openRelease(info.version)}
                >
                  {t('settings.openDownload')}
                  <Icon name="external" size={14} />
                  <span class="visually-hidden">{t('dialogs.opensInBrowser')}</span>
                </button>
              </div>
              {#if mode === 'notify'}<p class="hint">{t('settings.notifyHint')}</p>{/if}
            {/if}
          </div>
        {/if}
      </div>

      <Switch
        label={t('settings.auto')}
        hint={t('settings.autoHint')}
        checked={app.ui.check_updates}
        disabled={mode === 'none'}
        onchange={(on) => app.setCheckUpdates(on)}
      />
    </section>
  </section>

  <div class="duo">
    <section class="card" aria-labelledby="set-project-title">
      <div class="card-head">
        <h2 class="card-title" id="set-project-title">{t('settings.project')}</h2>
      </div>
      <Field
        label={t('settings.projectName')}
        value={p.name}
        parse={nonEmpty('errors.projectName')}
        oncommit={(v) => ((p.name = v), touch())}
        ondone={done}
        required
        maxlength={80}
      />
      <Switch
        label={t('settings.debugStart')}
        hint={t('settings.debugHint')}
        checked={p.debug}
        onchange={(on) => {
          p.debug = on;
          app.touch(true);
        }}
      />
    </section>

    <section class="card" aria-labelledby="set-range-title">
      <div class="card-head">
        <h2 class="card-title" id="set-range-title">{t('settings.range')}</h2>
        <span class="aside num">
          {t('settings.rangeSummary', { min: signed(p.transpose.min), max: signed(p.transpose.max) })}
        </span>
      </div>
      <div class="halves">
        <Field
          label={t('settings.min')}
          value={p.transpose.min}
          parse={parseMin}
          oncommit={(v) => ((p.transpose.min = v), touch())}
          ondone={done}
          numeric
          required
        />
        <Field
          label={t('settings.max')}
          value={p.transpose.max}
          parse={parseMax}
          oncommit={(v) => ((p.transpose.max = v), touch())}
          ondone={done}
          numeric
          required
        />
      </div>
      <p class="hint">{t('settings.rangeHint')}</p>
    </section>
  </div>

  <section class="card" aria-labelledby="set-data-title">
    <div class="card-head">
      <h2 class="card-title" id="set-data-title">{t('settings.dataFolder')}</h2>
    </div>
    <div class="field">
      <label class="label" for="project-file">{t('settings.projectFile')}</label>
      <input class="input mono" id="project-file" type="text" readonly value={app.path ?? t('project.notSaved')} />
    </div>
    <div class="field">
      <label class="label" for="documents-dir">{t('settings.documents')}</label>
      <input class="input mono" id="documents-dir" type="text" readonly value={app.app.documents_dir} />
    </div>
    <div class="field">
      <label class="label" for="data-dir">{t('settings.storedIn')}</label>
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

  .aside {
    color: var(--text-dim);
    font-size: var(--fs-sm);
  }

  .language {
    max-inline-size: 320px;
  }

  .updates {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    max-inline-size: 720px;
  }

  .updates-head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--sp-2) var(--sp-3);
  }

  .sub-title {
    font-size: var(--fs-md);
    font-weight: 600;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
  }

  .result:empty {
    display: none;
  }

  .found {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-3);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--surface-1);
  }

  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }

  .line > :global(svg) {
    color: var(--text-dim);
  }

  .line.strong {
    font-weight: 600;
  }

  .line.strong > :global(svg) {
    color: var(--accent-text);
  }

  .line.strong .hint {
    font-weight: 400;
  }

  .problem {
    color: var(--err);
  }

  .problem > :global(svg) {
    color: var(--err);
  }

  .notes {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
  }

  .notes-text {
    max-block-size: 160px;
    overflow: auto;
    color: var(--text-dim);
    font-size: var(--fs-sm);
    white-space: pre-line;
    overflow-wrap: anywhere;
  }

  .progress {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--sp-1) var(--sp-3);
  }

  .progress .label {
    grid-column: 1 / -1;
  }

  /* Native <progress>, fully restyled: the default light track glares on the dark surface. */
  progress {
    appearance: none;
    inline-size: 100%;
    block-size: 6px;
    border: 0 solid;
    border-radius: var(--r-pill);
    background: var(--line-strong);
    overflow: clip;
  }

  progress::-webkit-progress-bar {
    background: transparent;
  }

  /* WebKit and Chromium. Kept apart from the Firefox rule, or both are dropped. */
  progress::-webkit-progress-value {
    background: var(--accent);
    border-radius: var(--r-pill);
  }

  progress::-moz-progress-bar {
    background: var(--accent);
    border-radius: var(--r-pill);
  }
</style>
