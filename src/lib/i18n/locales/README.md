# Translations

One file per language, named by its code: `de.ts`, `it.ts`, `ja.ts`, `zh.ts`,
`fr.ts`, `es.ts`, `ko.ts`, `pl.ts`, `ru.ts`. English (`../en.ts`) is the source
and is not in this folder.

Each file looks like this and must have exactly the keys of `en.ts`:

```ts
import type { Dict } from '../en';

const de: Dict = {
  common: { save: 'Speichern', /* ... */ },
  // ...
};

export default de;
```

- Keep every `{placeholder}` exactly as in English; the order may change.
- Plural entries are objects with the categories of `Intl.PluralRules` for the
  language (`one`, `few`, `many`, `other`, ...). `other` is always required.
  Polish and Russian need `one`, `few`, `many` and `other`; Japanese, Chinese
  and Korean need only `other`.
- Do not translate MIDI, OSC, MQTT, VRChat, Home Assistant, CC, console
  command words, OSC paths or MQTT topics.

A file that is missing is not an error: the app shows English for that
language and warns in the browser console. Check the files with
`bun scripts/check-i18n.ts`; `bun run check` catches missing or extra keys.
