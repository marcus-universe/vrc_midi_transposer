<script lang="ts">
  // Native modal <dialog>. The browser provides the focus containment, the
  // inert page, Esc and focus return; this only opens and closes it.
  import type { Snippet } from 'svelte';

  let {
    open,
    title,
    onclose,
    children,
    actions,
    wide = false,
  }: {
    open: boolean;
    title: string;
    /** Called with the `value` of the button that closed it, or '' when dismissed. */
    onclose: (result: string) => void;
    children: Snippet;
    actions: Snippet;
    wide?: boolean;
  } = $props();

  const uid = $props.id();
  let el = $state<HTMLDialogElement>();
  /** Closed because `open` turned false: another dialog took over, nobody answered this one. */
  let closedByProp = false;

  $effect(() => {
    if (!el) return;
    if (open && !el.open) {
      el.showModal();
    } else if (!open && el.open) {
      closedByProp = true;
      el.close();
    }
  });

  function closed() {
    if (!el) return;
    if (closedByProp) {
      // Not an answer: calling onclose here would close the dialog that replaced this one.
      closedByProp = false;
      el.returnValue = '';
      return;
    }
    const result = el.returnValue;
    // Esc sets no value and browsers differ on clearing the previous one.
    el.returnValue = '';
    onclose(result);
  }
</script>

<dialog bind:this={el} class={{ wide }} aria-labelledby="{uid}-title" onclose={closed}>
  <form method="dialog">
    <h2 id="{uid}-title">{title}</h2>
    <div class="body">
      {@render children()}
    </div>
    <div class="actions">
      {@render actions()}
    </div>
  </form>
</dialog>

<style>
  dialog.wide {
    inline-size: min(520px, calc(100vw - 32px));
  }

  form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-5);
  }

  h2 {
    font-size: var(--fs-lg);
    font-weight: 600;
    line-height: 1.3;
    text-wrap: balance;
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    color: var(--text-dim);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--sp-2);
    margin-block-start: var(--sp-2);
  }
</style>
