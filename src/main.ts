import { mount } from 'svelte';
import './app.css';
import App from './App.svelte';
import { app } from './lib/app.svelte';
import { initValidation } from './lib/validation';

/**
 * <input type="checkbox" switch> is native in WebKit only. Elsewhere the
 * polyfill adds role="switch" and thumb dragging. The switch is drawn by
 * app.css in every engine, so the polyfill's own stylesheet is not needed: it
 * fetches that file relative to its module URL, where a bundler emits nothing,
 * and the one request is answered here with an empty sheet instead of a 404.
 */
async function loadSwitchPolyfill() {
  const realFetch = window.fetch;
  window.fetch = (input, init) =>
    String(input instanceof Request ? input.url : input).endsWith('/input-switch-polyfill.css')
      ? Promise.resolve(new Response('', { headers: { 'content-type': 'text/css' } }))
      : realFetch(input, init);
  try {
    // The polyfill issues its fetch while the module evaluates.
    await import('input-switch-polyfill');
  } finally {
    window.fetch = realFetch;
  }
}

if (!('switch' in HTMLInputElement.prototype)) {
  void loadSwitchPolyfill();
}

initValidation();
mount(App, { target: document.getElementById('app')! });
void app.start();
