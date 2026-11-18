---
name: transposer-ui
description: Design system for the VRC MIDI Transposer UI - dark grey-blue colour tokens, typography, spacing, component rules (cards, hero card, pills, switches, selects, tables, piano, console drawer), layout and the performance budget. Load before creating or restyling anything in src/.
---

# VRC MIDI Transposer design system

The look: a calm dark dashboard. Grey-blue surfaces stacked in three levels, one saturated blue for the things that matter (the transpose value, pressed keys, primary actions), rounded cards with soft depth. Dark only; there is no light theme.

All tokens live in `src/app.css` on `:root`. Use the tokens. If a design needs a value that is not there, add a token rather than a literal.

## Colour

| Token | Use |
|---|---|
| `--bg` | Window background (darkest) |
| `--surface-1` | Rail, top bar, console drawer |
| `--surface-2` | Cards |
| `--surface-3` | Inputs, table rows on hover, nested panels |
| `--line` | Hairline borders and dividers |
| `--text` | Primary text |
| `--text-dim` | Labels, secondary text |
| `--text-faint` | Placeholders, disabled |
| `--accent` | Primary blue: active nav item, focus ring, pressed piano keys, primary button |
| `--accent-strong` / `--accent-soft` | Hero gradient end stops, accent tints behind icons |
| `--accent-hover` / `--accent-active` | Hover and pressed states of accent-filled controls |
| `--accent-text` | Accent-coloured text and the console banner; lighter than `--accent` so it reaches contrast on dark surfaces |
| `--focus` | Focus ring (same as `--accent-text`; `--accent` is below 3:1 on `--surface-3`) |
| `--on-accent`, `--on-accent-*` | Text, tints, lines and the slider track on the hero card |
| `--surface-3-hover`, `--line-strong` | Hover fill of quiet controls; control borders, switch track, scrollbar thumb |
| `--ok` / `--warn` / `--err` | Status only. Never decorative |
| `--err-soft` / `--warn-soft` | Background of error and warning bars |
| `--key-white` / `--key-white-line` / `--key-black` | Piano keys |
| `--backdrop` | Dialog backdrop (a flat colour, no blur) |

Rules:

- Text on `--surface-*` must reach 4.5:1; `--text-dim` is tuned for that, `--text-faint` is for non-essential text only.
- Status is never colour alone: a pill has a dot **and** a word ("Connected", "Error").
- One accent. Do not add a second hue for variety; the status colours are the only others.
- Red, amber and green appear only when something has that status.

## Type and spacing

- UI font: the system UI stack (`--font-ui`). Numbers that change (transpose value, ports, values in tables) use `font-variant-numeric: tabular-nums`.
- Monospace (`--font-mono`) for the console, OSC paths and MQTT topics.
- Sizes: `--fs-xs` 11px labels, `--fs-sm` 12.5px secondary, `--fs-md` 14px body, `--fs-lg` 17px card titles, `--fs-hero` for the transpose number.
- Section labels are small, dim, medium weight, sentence case. No all-caps headings.
- Spacing scale `--sp-1`..`--sp-6` (4, 8, 12, 16, 24, 32). Card padding is `--sp-4`; gaps between cards are `--sp-4`.
- Radii: `--r-sm` 8px inputs and buttons, `--r-md` 14px cards, `--r-lg` 20px the hero card.

## Components

- **Card:** `--surface-2`, 1px `--line` border, `--r-md`, `--shadow-1`. A title row with a dim label on the left and optional actions on the right.
- **Hero card (transpose):** the only gradient in the app, `--accent` to `--accent-strong`, white text. Holds the large value, −/+ buttons, the range slider and Reset.
- **Status pill:** small rounded label with a 6px dot. States: neutral (off), ok, warn, err.
- **Buttons:** primary (accent fill), quiet (surface-3 fill), ghost (text only, for icon buttons). Minimum hit area 32×32px. Icon-only buttons need `aria-label`.
- **Switch:** native `<input type="checkbox" switch>` with the fallback from the `switch` guide. Label is always visible and clickable.
- **Select:** native `<select>`, styled; enhanced with `appearance: base-select` only inside `@supports`.
- **Fields:** label above input. Errors appear after interaction (`:user-invalid`), under the field, in words.
- **Tables (OSC addresses):** rows are editable in place: a role select, a path input, a remove button. An "Add" row action sits under the table. Empty state is one sentence plus the add button.
- **Dialogs:** native `<dialog>` with `showModal()`. Destructive confirmations name the thing being deleted.
- **Piano:** 88 keys, A0 to C8. White keys `--key-white`, black keys `--key-black`, pressed keys `--accent` (black pressed keys use `--accent-strong`). C keys carry an octave label.
- **Console drawer:** `--surface-1`, monospace, bottom of the window, resizable by dragging its top edge. Log colours: default `--text-dim`, success `--ok`, warning `--warn`, error `--err`, banner `--accent`.

## Layout

- Fixed left rail (56px, icons with tooltips), top bar (48px), scrolling main area, piano strip pinned above the console drawer.
- Minimum window 940×600. Cards reflow with container queries on the main area, not viewport media queries.
- The dashboard fits without scrolling at 1100×720 with the console closed.

## Motion

- 120–160 ms ease-out for hover, press and drawer open. Nothing loops.
- Respect `prefers-reduced-motion`: transitions off.
- Piano key changes are instant. No transition on key colour; latency reads as lag.

## Performance budget

- No `backdrop-filter`, no large blurred shadows, no animated gradients.
- The piano and the console are `contain: layout style paint` regions.
- Engine state arrives at most once per frame. The piano compares the new key bitmap with the last one and touches only the keys that changed.
- The console keeps at most 2000 lines in memory and 500 in the DOM, and renders nothing while closed.
- No timers or `requestAnimationFrame` loops while idle.

## Copy

- Name things by what the user does: "OSC sending", "Send original notes", "Listening addresses", "Custom controls".
- Buttons say the action: "Save", "Add address", "Delete project".
- Errors say what happened and what to do: "Port 9069 is already in use. Close the other program or choose another port."
