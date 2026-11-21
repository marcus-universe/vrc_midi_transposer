// Field parsers: text in, a value or an error sentence out. Used by Field.svelte.
// Error sentences are looked up when the parser runs, inside Field's effect,
// so a shown error follows a change of language. Inputs keep "." decimals
// whatever the language ("," is accepted too); only displays are localised.

import { t, type Key } from './i18n/i18n.svelte';

export type Parsed<T> = { value: T } | { error: string };
export type Parser<T> = (raw: string) => Parsed<T>;

/** Keys of messages without placeholders, for parsers that take a message. */
type MessageKey = Extract<Key, 'errors.projectName' | 'errors.controlName'>;

export const anyText: Parser<string> = (raw) => ({ value: raw });

export function nonEmpty(message: MessageKey): Parser<string> {
  return (raw) => (raw.trim() ? { value: raw.trim() } : { error: t(message) });
}

export const host: Parser<string> = (raw) => {
  const v = raw.trim();
  if (!v || /\s/.test(v)) return { error: t('errors.host') };
  return { value: v };
};

export const port: Parser<number> = (raw) => {
  const v = raw.trim();
  const n = Number(v);
  if (!/^\d{1,5}$/.test(v) || n < 1 || n > 65535) return { error: t('errors.port') };
  return { value: n };
};

export const oscPath: Parser<string> = (raw) => {
  const v = raw.trim();
  if (!v.startsWith('/')) return { error: t('errors.oscSlash') };
  if (/\s/.test(v)) return { error: t('errors.oscSpaces') };
  return { value: v };
};

export const mqttTopic: Parser<string> = (raw) => {
  const v = raw.trim();
  if (!v || /[\s#+]/.test(v) || v.startsWith('/') || v.endsWith('/')) return { error: t('errors.mqttTopic') };
  return { value: v };
};

/** Accepts the typographic minus as well, since the UI displays it. */
function toNumber(raw: string): number {
  const v = raw.trim().replace('−', '-').replace(',', '.');
  return v === '' ? NaN : Number(v);
}

export function integer(check?: (n: number) => string | null): Parser<number> {
  return (raw) => {
    const n = toNumber(raw);
    if (!Number.isInteger(n)) return { error: t('errors.integer') };
    const problem = check?.(n);
    return problem ? { error: problem } : { value: n };
  };
}

export function float(check?: (n: number) => string | null): Parser<number> {
  return (raw) => {
    const n = toNumber(raw);
    if (!Number.isFinite(n)) return { error: t('errors.number') };
    const problem = check?.(n);
    return problem ? { error: problem } : { value: n };
  };
}

/** Like `float`, but an empty field means "not set". */
export function optionalFloat(check?: (n: number) => string | null): Parser<number | null> {
  const inner = float(check);
  return (raw) => (raw.trim() === '' ? { value: null } : inner(raw));
}
