// Types shared by the dictionaries and the runtime. Pure types: no runtime code,
// so locale files and scripts/check-i18n.ts can import them anywhere.

/** A plural entry. `other` is required; the rest depend on the language (Intl.PluralRules). */
export type Plural = {
  zero?: string;
  one?: string;
  two?: string;
  few?: string;
  many?: string;
  other: string;
};

/** Literal strings of the source dictionary become `string`; plural objects become `Plural`. */
export type Widen<T> = T extends string
  ? string
  : T extends { readonly other: string }
    ? Plural
    : { -readonly [K in keyof T]: Widen<T[K]> };

type Join<P extends string, K extends string> = P extends '' ? K : `${P}.${K}`;

/** Dot paths of every plain string. */
export type StringPaths<T, P extends string = ''> = {
  [K in keyof T & string]: T[K] extends string
    ? Join<P, K>
    : T[K] extends { readonly other: string }
      ? never
      : StringPaths<T[K], Join<P, K>>;
}[keyof T & string];

/** Dot paths of every plural entry. */
export type PluralPaths<T, P extends string = ''> = {
  [K in keyof T & string]: T[K] extends string
    ? never
    : T[K] extends { readonly other: string }
      ? Join<P, K>
      : PluralPaths<T[K], Join<P, K>>;
}[keyof T & string];

/** The value at a dot path. */
export type At<T, K extends string> = K extends `${infer H}.${infer R}`
  ? H extends keyof T
    ? At<T[H], R>
    : never
  : K extends keyof T
    ? T[K]
    : never;

/** `{name}` placeholders of a string literal (distributes over unions). */
export type Vars<S> = S extends `${string}{${infer V}}${infer R}` ? V | Vars<R> : never;

export type Params = Record<string, string | number>;

/** Rest arguments: none when the string has no placeholders, otherwise exactly its names. */
export type ArgsFor<V extends string> = [V] extends [never] ? [] : [params: Record<V, string | number>];
