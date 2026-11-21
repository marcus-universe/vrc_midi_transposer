// Checks the translations in src/lib/i18n/locales against the English source.
//
//   bun scripts/check-i18n.ts
//
// For every locale file present: the same keys as en.ts (deep), the same
// {placeholder} names in every string, plural entries are objects with
// `other`, and no empty strings. Exits 1 on any problem. With no locale files
// it checks English alone and passes.
//
// Plural categories the language uses but the file leaves out (and ones the
// language never uses) are reported as warnings: they fall back to `other`.

import { readdirSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import en from '../src/lib/i18n/en';

const LOCALES_DIR = join(import.meta.dir, '..', 'src', 'lib', 'i18n', 'locales');
const KNOWN = ['de', 'it', 'ja', 'zh', 'fr', 'es', 'ko', 'pl', 'ru'];
const TAG: Record<string, string> = { zh: 'zh-Hans' };
const CATEGORIES = ['zero', 'one', 'two', 'few', 'many', 'other'];

type Node = string | { [key: string]: Node };

const errors: string[] = [];
const warnings: string[] = [];

const isObject = (v: unknown): v is Record<string, Node> => typeof v === 'object' && v !== null && !Array.isArray(v);
const isPlural = (v: unknown): v is Record<string, string> => isObject(v) && 'other' in v;

function vars(s: string): Set<string> {
  return new Set([...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]));
}

const same = (a: Set<string>, b: Set<string>) => a.size === b.size && [...a].every((x) => b.has(x));
const show = (s: Set<string>) => (s.size ? [...s].map((x) => `{${x}}`).join(', ') : 'none');

function checkPlural(file: string, path: string, value: Record<string, unknown>, source: Record<string, string> | null, tag: string) {
  if (typeof value.other !== 'string') {
    errors.push(`${file}: ${path}: plural entry without 'other'`);
    return;
  }
  const allowed = source ? new Set(Object.values(source).flatMap((s) => [...vars(s)])) : null;
  for (const [cat, text] of Object.entries(value)) {
    if (!CATEGORIES.includes(cat)) {
      errors.push(`${file}: ${path}.${cat}: '${cat}' is not a plural category (${CATEGORIES.join(', ')})`);
      continue;
    }
    if (typeof text !== 'string') {
      errors.push(`${file}: ${path}.${cat}: must be a string`);
      continue;
    }
    if (!text.trim()) errors.push(`${file}: ${path}.${cat}: empty string`);
    if (allowed) {
      const extra = [...vars(text)].filter((v) => !allowed.has(v));
      if (extra.length) errors.push(`${file}: ${path}.${cat}: unknown placeholder ${extra.map((v) => `{${v}}`).join(', ')}`);
    }
  }
  if (source) {
    // `other` carries every placeholder in English; it must in every language.
    const want = vars(source.other);
    const got = vars(value.other as string);
    if (!same(want, got)) errors.push(`${file}: ${path}.other: placeholders ${show(got)}, English has ${show(want)}`);
  }
  const used = new Intl.PluralRules(tag).resolvedOptions().pluralCategories as string[];
  const missing = used.filter((c) => !(c in value));
  if (missing.length) warnings.push(`${file}: ${path}: no ${missing.join(', ')} (falls back to 'other')`);
  const unused = Object.keys(value).filter((c) => CATEGORIES.includes(c) && !used.includes(c));
  if (unused.length) warnings.push(`${file}: ${path}: ${unused.join(', ')} not used by ${tag}`);
}

/** Walks `value` (the locale) beside `source` (English). */
function walk(file: string, tag: string, path: string, source: unknown, value: unknown) {
  if (typeof source === 'string') {
    if (typeof value !== 'string') {
      errors.push(`${file}: ${path}: must be a string`);
      return;
    }
    if (!value.trim()) errors.push(`${file}: ${path}: empty string`);
    const want = vars(source);
    const got = vars(value);
    if (!same(want, got)) errors.push(`${file}: ${path}: placeholders ${show(got)}, English has ${show(want)}`);
    return;
  }
  if (isPlural(source)) {
    if (!isObject(value)) {
      errors.push(`${file}: ${path}: must be a plural object with 'other'`);
      return;
    }
    checkPlural(file, path, value, source, tag);
    return;
  }
  if (!isObject(source)) return;
  if (!isObject(value)) {
    errors.push(`${file}: ${path || '(root)'}: must be an object`);
    return;
  }
  for (const key of Object.keys(source)) {
    const sub = path ? `${path}.${key}` : key;
    if (!(key in value)) errors.push(`${file}: ${sub}: missing`);
    else walk(file, tag, sub, (source as Record<string, unknown>)[key], value[key]);
  }
  for (const key of Object.keys(value)) {
    if (!(key in source)) errors.push(`${file}: ${path ? `${path}.${key}` : key}: not in English`);
  }
}

/** English itself: plural objects with `other`, no empty strings. */
function walkSource(path: string, node: unknown) {
  if (typeof node === 'string') {
    if (!node.trim()) errors.push(`en.ts: ${path}: empty string`);
  } else if (isPlural(node)) {
    checkPlural('en.ts', path, node, null, 'en');
  } else if (isObject(node)) {
    for (const [k, v] of Object.entries(node)) walkSource(path ? `${path}.${k}` : k, v);
  } else {
    errors.push(`en.ts: ${path}: unexpected value`);
  }
}

walkSource('', en);

let files: string[] = [];
try {
  files = readdirSync(LOCALES_DIR).filter((f) => f.endsWith('.ts')).sort();
} catch {
  // No locales folder: English only.
}

for (const file of files) {
  const code = file.slice(0, -3);
  if (!KNOWN.includes(code)) {
    errors.push(`${file}: unknown language code '${code}' (expected one of ${KNOWN.join(', ')})`);
    continue;
  }
  let dict: unknown;
  try {
    dict = (await import(pathToFileURL(join(LOCALES_DIR, file)).href)).default;
  } catch (e) {
    errors.push(`${file}: could not be loaded: ${e instanceof Error ? e.message : String(e)}`);
    continue;
  }
  if (dict === undefined) {
    errors.push(`${file}: no default export`);
    continue;
  }
  walk(file, TAG[code] ?? code, '', en, dict);
}

for (const w of warnings) console.warn(`warning: ${w}`);
for (const e of errors) console.error(`error: ${e}`);

const checked = files.length ? `${files.length} locale file(s): ${files.join(', ')}` : 'no locale files';
if (errors.length) {
  console.error(`check-i18n: ${errors.length} problem(s) in ${checked}.`);
  process.exit(1);
}
console.log(`check-i18n: OK (${checked}; ${warnings.length} warning(s)).`);
