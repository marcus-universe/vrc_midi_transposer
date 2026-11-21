// Keeps ARIA in step with the visual :user-invalid state for every field in the
// app, and announces an error once when it appears. One set of document-level
// listeners; fields opt in with `data-error="<id of their error message>"`.

const INVALID = ':is(:user-invalid, .user-invalid-fallback)';

type FieldEl = HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement;

function isField(el: EventTarget | null): el is FieldEl {
  return (
    el instanceof HTMLInputElement || el instanceof HTMLSelectElement || el instanceof HTMLTextAreaElement
  );
}

/** List the error message in aria-describedby only while it is shown. */
function linkError(el: FieldEl, shown: boolean) {
  const errorId = el.dataset.error;
  if (!errorId) return;
  const ids = (el.getAttribute('aria-describedby') || '')
    .split(/\s+/)
    .filter((id) => id && id !== errorId);
  if (shown) ids.push(errorId);
  if (ids.length) el.setAttribute('aria-describedby', ids.join(' '));
  else el.removeAttribute('aria-describedby');
}

/** Text as a screen reader reads it: icon alternatives in, aria-hidden text out. */
function spokenText(node: Element): string {
  const copy = node.cloneNode(true) as Element;
  copy.querySelectorAll('[aria-hidden="true"]').forEach((n) => n.remove());
  copy
    .querySelectorAll('[role="img"][aria-label]')
    .forEach((n) => n.replaceWith(`${n.getAttribute('aria-label')}: `));
  return (copy.textContent ?? '').replace(/\s+/g, ' ').trim();
}

export function initValidation() {
  const announcer = document.createElement('div');
  announcer.setAttribute('aria-live', 'polite');
  announcer.className = 'visually-hidden';
  document.body.append(announcer);

  let announceTimer: ReturnType<typeof setTimeout> | undefined;
  let announcedField: FieldEl | undefined;
  const flagged = new WeakSet<FieldEl>();
  const announced = new WeakSet<FieldEl>();

  const announceError = (el: FieldEl) => {
    const error = el.dataset.error ? document.getElementById(el.dataset.error) : null;
    if (!error) return;
    const name = el.labels?.[0] ? spokenText(el.labels[0]) : (el.getAttribute('aria-label') ?? '');
    clearTimeout(announceTimer);
    announcer.textContent = '';
    announcedField = el;
    // Wait until the screen reader has announced the field that receives focus next.
    announceTimer = setTimeout(() => {
      announcer.textContent = (name ? `${name}: ` : '') + spokenText(error);
    }, 500);
  };

  const syncAria = (target: EventTarget | null, announce = false) => {
    if (!isField(target) || !target.hasAttribute('aria-invalid')) return;
    const el = target;
    if (el.matches(INVALID)) {
      if (announce && !announced.has(el)) {
        announceError(el);
        announced.add(el);
      }
      el.setAttribute('aria-invalid', 'true');
      linkError(el, true);
      flagged.add(el);
    } else if (flagged.has(el) || el.getAttribute('aria-invalid') !== 'true') {
      el.setAttribute('aria-invalid', 'false');
      linkError(el, false);
      flagged.delete(el);
      announced.delete(el);
      if (announcedField === el) {
        clearTimeout(announceTimer);
        announcer.textContent = '';
      }
    }
  };

  // Engines without :user-invalid get a class with the same meaning: the field
  // was edited and then left (or a submit was attempted) while invalid.
  if (!CSS.supports('selector(:user-invalid)')) {
    const dirty = new WeakMap<FieldEl, { edited: boolean; left: boolean }>();
    const state = (el: FieldEl) => {
      let s = dirty.get(el);
      if (!s) dirty.set(el, (s = { edited: false, left: false }));
      return s;
    };
    const update = (el: FieldEl) => el.classList.toggle('user-invalid-fallback', !el.validity.valid);

    document.addEventListener(
      'blur',
      (e) => {
        if (!isField(e.target)) return;
        const s = state(e.target);
        s.left = true;
        if (s.edited) update(e.target);
      },
      true,
    );
    const onEdit = (e: Event) => {
      if (!isField(e.target)) return;
      const s = state(e.target);
      s.edited = true;
      if (s.left) update(e.target);
    };
    document.addEventListener('input', onEdit, true);
    document.addEventListener('change', onEdit, true);
    document.addEventListener(
      'invalid',
      (e) => {
        if (!isField(e.target)) return;
        dirty.set(e.target, { edited: true, left: true });
        update(e.target);
      },
      true,
    );
  }

  // Registered after the fallback above so the class is already up to date.
  document.addEventListener('blur', (e) => syncAria(e.target, true), true);
  document.addEventListener('input', (e) => syncAria(e.target), true);
  document.addEventListener('change', (e) => syncAria(e.target, true), true);
  // A failed submit fires 'invalid' on each failing field; focus moving to the
  // first one announces its error, so these are not announced a second time.
  document.addEventListener(
    'invalid',
    (e) => {
      if (isField(e.target)) announced.add(e.target);
      syncAria(e.target);
    },
    true,
  );
}
