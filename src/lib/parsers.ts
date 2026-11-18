// Field parsers: text in, a value or an error sentence out. Used by Field.svelte.

export type Parsed<T> = { value: T } | { error: string };
export type Parser<T> = (raw: string) => Parsed<T>;

export const anyText: Parser<string> = (raw) => ({ value: raw });

export function nonEmpty(message: string): Parser<string> {
  return (raw) => (raw.trim() ? { value: raw.trim() } : { error: message });
}

export const host: Parser<string> = (raw) => {
  const v = raw.trim();
  if (!v || /\s/.test(v)) {
    return { error: 'Enter a host name or IP address without spaces, for example 127.0.0.1.' };
  }
  return { value: v };
};

export const port: Parser<number> = (raw) => {
  const v = raw.trim();
  const n = Number(v);
  if (!/^\d{1,5}$/.test(v) || n < 1 || n > 65535) {
    return { error: 'Enter a port between 1 and 65535.' };
  }
  return { value: n };
};

export const oscPath: Parser<string> = (raw) => {
  const v = raw.trim();
  if (!v.startsWith('/')) {
    return { error: 'OSC addresses start with a slash, for example /transpose.' };
  }
  if (/\s/.test(v)) return { error: 'OSC addresses cannot contain spaces.' };
  return { value: v };
};

export const mqttTopic: Parser<string> = (raw) => {
  const v = raw.trim();
  if (!v || /[\s#+]/.test(v) || v.startsWith('/') || v.endsWith('/')) {
    return {
      error: 'Enter a topic without spaces, # or +, and no slash at either end, for example midi_transposer.',
    };
  }
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
    if (!Number.isInteger(n)) return { error: 'Enter a whole number, for example 12.' };
    const problem = check?.(n);
    return problem ? { error: problem } : { value: n };
  };
}

export function float(check?: (n: number) => string | null): Parser<number> {
  return (raw) => {
    const n = toNumber(raw);
    if (!Number.isFinite(n)) return { error: 'Enter a number, for example 0.5.' };
    const problem = check?.(n);
    return problem ? { error: problem } : { value: n };
  };
}

/** Like `float`, but an empty field means "not set". */
export function optionalFloat(check?: (n: number) => string | null): Parser<number | null> {
  const inner = float(check);
  return (raw) => (raw.trim() === '' ? { value: null } : inner(raw));
}
