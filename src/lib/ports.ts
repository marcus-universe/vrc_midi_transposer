// MIDI port selects: which option is selected and the status line under them.
// Shared by the MIDI card on the dashboard and the clock device select.

import { t } from './i18n/i18n.svelte';
import type { MidiStatus, PortInfo, PortRef } from './ipc';

/** The "None" option. */
export const NONE = '';
/** The extra "(not connected)" option for a saved port that is not present. */
export const SAVED = '__saved__';

/**
 * Which option is selected. A connected port wins (the engine may have matched
 * a saved name pattern that has no id); otherwise the saved reference by id,
 * then by exact name; otherwise an extra "(not connected)" option.
 */
export function portSelection(
  ref: PortRef | null,
  status: MidiStatus,
  ports: PortInfo[],
): { value: string; extra: PortRef | null } {
  if (!ref) return { value: NONE, extra: null };
  if (status.state === 'connected' && ports.some((x) => x.id === status.id)) {
    return { value: status.id, extra: null };
  }
  const match = ports.find((x) => ref.id !== '' && x.id === ref.id) ?? ports.find((x) => x.name === ref.name);
  return match ? { value: match.id, extra: null } : { value: SAVED, extra: ref };
}

/** The port reference an option stands for; undefined for the saved option (no change). */
export function portFromOption(value: string, ports: PortInfo[]): PortRef | null | undefined {
  if (value === SAVED) return undefined;
  const port = ports.find((x) => x.id === value);
  return port ? { id: port.id, name: port.name } : null;
}

/** Status line under a port select: a tone for the dot and the words. */
export function midiLine(st: MidiStatus): { tone: string; text: string } {
  switch (st.state) {
    case 'connected':
      return { tone: 'ok', text: t('status.connected') };
    case 'missing':
      return { tone: 'warn', text: t('dashboard.portMissing') };
    case 'error':
      return {
        tone: 'err',
        text: st.detail ? t('common.errorDetail', { detail: st.detail }) : t('dashboard.portErrorUnknown'),
      };
    default:
      return { tone: '', text: t('dashboard.noPort') };
  }
}
