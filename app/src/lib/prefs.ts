import { useSyncExternalStore } from 'react';

/** Display preferences kept on this device (localStorage), shared by every view. */
export type Theme = 'dark' | 'light';

const listeners = new Set<() => void>();
const emit = () => listeners.forEach((l) => l());
const subscribe = (cb: () => void) => {
  listeners.add(cb);
  window.addEventListener('storage', cb);
  return () => {
    listeners.delete(cb);
    window.removeEventListener('storage', cb);
  };
};
const read = (k: string): string | null => {
  try { return localStorage.getItem(k); } catch { return null; }
};
const write = (k: string, v: string) => {
  try { localStorage.setItem(k, v); } catch { /* private mode: the choice lasts until reload */ }
  emit();
};

export const getTheme = (): Theme => (read('wl.theme') === 'light' ? 'light' : 'dark');
export function setTheme(t: Theme) {
  write('wl.theme', t);
  document.documentElement.dataset.theme = t;
}
export const useTheme = (): [Theme, (t: Theme) => void] => [useSyncExternalStore(subscribe, getTheme), setTheme];

/** True when clock times show as 12 hour (3:05 pm); the default is 24 hour. */
export const getH12 = (): boolean => read('wl.h12') === '1';
export const setH12 = (b: boolean) => write('wl.h12', b ? '1' : '0');
export const useH12 = (): [boolean, (b: boolean) => void] => [useSyncExternalStore(subscribe, getH12), setH12];

/** A local clock time from hours since midnight (fractions allowed), in the chosen 12 or 24 hour style. */
export function fmtClock(hours: number, h12 = getH12()): string {
  if (!Number.isFinite(hours)) return '';
  const total = Math.round((((hours % 24) + 24) % 24) * 60) % 1440;
  const h = Math.floor(total / 60), m = total % 60;
  const mm = String(m).padStart(2, '0');
  if (!h12) return `${String(h).padStart(2, '0')}:${mm}`;
  return `${h % 12 === 0 ? 12 : h % 12}:${mm} ${h < 12 ? 'am' : 'pm'}`;
}
