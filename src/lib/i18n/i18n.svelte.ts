// A tiny typed i18n runtime. English is bundled; the other languages are
// separate chunks loaded on demand. The active dictionary lives in runes
// state, so every `t()` call in a template or `$derived` re-runs when the
// language changes; nothing needs a reload.
//
//   t('dashboard.transpose')                         -> "Transpose"
//   t('dashboard.listeningOn', { address: bound })  -> "Listening on 127.0.0.1:9069"
//   tn('mqtt.entities', 3)                          -> "3 entities published"
//   tp('dialogs.revertBody', { project: name })     -> parts, to wrap a value in markup
//
// Numbers in params are inserted as they are (ports, note numbers); format
// them first with `formatNumber` where a localised number is wanted. `{count}`
// in a plural entry is formatted for the active language.

import en, { type Dict, type En } from './en';
import type { ArgsFor, At, Params, Plural, PluralPaths, StringPaths, Vars } from './types';

export const LOCALES = ['en', 'de', 'it', 'ja', 'zh', 'fr', 'es', 'ko', 'pl', 'ru'] as const;
export type Locale = (typeof LOCALES)[number];

/** Each language in its own name, for the language picker. */
export const LANGUAGE_NAMES: Record<Locale, string> = {
  en: 'English',
  de: 'Deutsch',
  it: 'Italiano',
  ja: '日本語',
  zh: '简体中文',
  fr: 'Français',
  es: 'Español',
  ko: '한국어',
  pl: 'Polski',
  ru: 'Русский',
};

/** BCP 47 tag for `<html lang>` and Intl. Chinese is Simplified, which selects the right CJK glyphs. */
const TAGS: Record<Locale, string> = {
  en: 'en',
  de: 'de',
  it: 'it',
  ja: 'ja',
  zh: 'zh-Hans',
  fr: 'fr',
  es: 'es',
  ko: 'ko',
  pl: 'pl',
  ru: 'ru',
};

export type Key = StringPaths<En>;
export type PluralKey = PluralPaths<En>;

type PluralVars<K extends PluralKey> = Exclude<Vars<At<En, K>[keyof At<En, K>]>, 'count'>;

class I18nState {
  locale = $state<Locale>('en');
  dict = $state.raw<Dict>(en);
}

export const i18n = new I18nState();

export function isLocale(v: unknown): v is Locale {
  return typeof v === 'string' && (LOCALES as readonly string[]).includes(v);
}

/** The first of the user's preferred languages we have, by language prefix; else English. */
export function detectLocale(preferred: readonly string[] = navigator.languages ?? [navigator.language]): Locale {
  for (const tag of preferred) {
    const base = tag.toLowerCase().split('-')[0];
    if (isLocale(base)) return base;
  }
  return 'en';
}

// Vite turns this into one lazy chunk per language.
const loaders = import.meta.glob<{ default: Dict }>('./locales/*.ts');
let request = 0;

/**
 * Switches the language. Resolves once the dictionary is loaded and active.
 * Never rejects: a language whose file is missing or fails to load falls back
 * to English, with a warning in the console.
 */
export async function setLocale(code: Locale): Promise<void> {
  const mine = ++request;
  let dict: Dict = en;
  if (code !== 'en') {
    const load = loaders[`./locales/${code}.ts`];
    if (!load) {
      console.warn(`[i18n] No dictionary for "${code}" (src/lib/i18n/locales/${code}.ts); using English.`);
      code = 'en';
    } else {
      try {
        dict = (await load()).default;
      } catch (e) {
        console.warn(`[i18n] Could not load the dictionary for "${code}"; using English.`, e);
        code = 'en';
      }
    }
  }
  // A later call won while this one was loading.
  if (mine !== request) return;
  i18n.dict = dict;
  i18n.locale = code;
  if (typeof document !== 'undefined') document.documentElement.lang = TAGS[code];
}

/** BCP 47 tag of the active language, for Intl. */
export function localeTag(): string {
  return TAGS[i18n.locale];
}

// ---------------------------------------------------------------------------
// Lookup
// ---------------------------------------------------------------------------

function lookup(dict: unknown, key: string): unknown {
  let node = dict;
  for (const part of key.split('.')) {
    if (node == null || typeof node !== 'object') return undefined;
    node = (node as Record<string, unknown>)[part];
  }
  return node;
}

function fill(template: string, params?: Params): string {
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (whole, name: string) =>
    name in params ? String(params[name]) : whole,
  );
}

function stringAt(key: string): string {
  const v = lookup(i18n.dict, key);
  if (typeof v === 'string') return v;
  const fallback = lookup(en, key);
  return typeof fallback === 'string' ? fallback : key;
}

const pluralRules = new Map<string, Intl.PluralRules>();

function pluralAt(key: string, count: number): string {
  const tag = localeTag();
  let rules = pluralRules.get(tag);
  if (!rules) pluralRules.set(tag, (rules = new Intl.PluralRules(tag)));
  const category = rules.select(count) as keyof Plural;
  const entry = (lookup(i18n.dict, key) ?? lookup(en, key)) as Plural | undefined;
  if (!entry) return key;
  return entry[category] ?? entry.other;
}

/** Translate a plain entry. Placeholders are type-checked against the English string. */
export function t<K extends Key>(key: K, ...args: ArgsFor<Vars<At<En, K>>>): string {
  return fill(stringAt(key), args[0] as Params | undefined);
}

/** Translate a plural entry for `count`. `{count}` is formatted for the active language. */
export function tn<K extends PluralKey>(key: K, count: number, ...args: ArgsFor<PluralVars<K>>): string {
  return fill(pluralAt(key, count), { ...(args[0] as Params | undefined), count: formatNumber(count) });
}

export type Part = { text: string; param?: string };

function split(template: string, params: Params): Part[] {
  const parts: Part[] = [];
  const re = /\{(\w+)\}/g;
  let at = 0;
  for (let m = re.exec(template); m; m = re.exec(template)) {
    if (!(m[1] in params)) continue;
    if (m.index > at) parts.push({ text: template.slice(at, m.index) });
    parts.push({ text: String(params[m[1]]), param: m[1] });
    at = m.index + m[0].length;
  }
  if (at < template.length) parts.push({ text: template.slice(at) });
  return parts;
}

/**
 * Translate into parts, for sentences that wrap a value in markup. Each
 * placeholder becomes its own part with `param` set, in the order the
 * language puts it:
 *
 *   {#each tp('dialogs.revertBody', { project }) as p}
 *     {#if p.param}<strong>{p.text}</strong>{:else}{p.text}{/if}
 *   {/each}
 */
export function tp<K extends Key>(key: K, ...args: ArgsFor<Vars<At<En, K>>>): Part[] {
  return split(stringAt(key), (args[0] ?? {}) as Params);
}

/** `tp` for a plural entry. */
export function tpn<K extends PluralKey>(key: K, count: number, ...args: ArgsFor<PluralVars<K>>): Part[] {
  return split(pluralAt(key, count), { ...(args[0] as Params | undefined), count: formatNumber(count) });
}

// ---------------------------------------------------------------------------
// Formatting (cached Intl objects for the active language)
// ---------------------------------------------------------------------------

const numberFormats = new Map<string, Intl.NumberFormat>();

/** A number for display. Never use this for the value of an editable input. */
export function formatNumber(n: number, options?: Intl.NumberFormatOptions): string {
  const tag = localeTag();
  const id = `${tag}|${options ? JSON.stringify(options) : ''}`;
  let f = numberFormats.get(id);
  if (!f) numberFormats.set(id, (f = new Intl.NumberFormat(tag, options)));
  return f.format(n);
}

const relativeFormats = new Map<string, Intl.RelativeTimeFormat>();

/** "just now", "42 seconds ago", "yesterday" in the active language. */
export function formatRelative(ts: number, now: number): string {
  const s = Math.max(0, Math.round((now - ts) / 1000));
  if (s < 5) return t('format.justNow');
  const tag = localeTag();
  let f = relativeFormats.get(tag);
  if (!f) relativeFormats.set(tag, (f = new Intl.RelativeTimeFormat(tag, { numeric: 'auto' })));
  if (s < 60) return f.format(-s, 'second');
  const m = Math.round(s / 60);
  if (m < 60) return f.format(-m, 'minute');
  const h = Math.round(m / 60);
  if (h < 24) return f.format(-h, 'hour');
  return f.format(-Math.round(h / 24), 'day');
}

const clockFormats = new Map<string, Intl.DateTimeFormat>();

/** Wall-clock time with seconds, 24-hour, so console timestamps keep one width. */
export function formatClock(ts: number): string {
  const tag = localeTag();
  let f = clockFormats.get(tag);
  if (!f) {
    f = new Intl.DateTimeFormat(tag, { hour: '2-digit', minute: '2-digit', second: '2-digit', hourCycle: 'h23' });
    clockFormats.set(tag, f);
  }
  return f.format(ts);
}

const listFormats = new Map<string, Intl.ListFormat>();

/** "a, b and c" as parts, so each item can be wrapped in markup. */
export function listParts(items: string[]): { text: string; item: boolean }[] {
  const tag = localeTag();
  let f = listFormats.get(tag);
  if (!f) listFormats.set(tag, (f = new Intl.ListFormat(tag, { style: 'long', type: 'conjunction' })));
  return f.formatToParts(items).map((p) => ({ text: p.value, item: p.type === 'element' }));
}
