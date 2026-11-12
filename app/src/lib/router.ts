import { useCallback, useSyncExternalStore } from 'react';
import { formatLoc, parseLoc, type Loc } from '@wl/views';

const listeners = new Set<() => void>();
const emit = () => listeners.forEach((l) => l());
window.addEventListener('popstate', emit);

let cache: { href: string; loc: Loc } = { href: '', loc: { path: '/', query: {} } };
const snapshot = (): Loc => {
  const href = location.pathname + location.search;
  if (href !== cache.href) cache = { href, loc: parseLoc(href) };
  return cache.loc;
};
export const useLoc = (): Loc => useSyncExternalStore((cb) => (listeners.add(cb), () => listeners.delete(cb)), snapshot);

export function navigate(to: string, replace = false) {
  if (to === location.pathname + location.search) return;
  history[replace ? 'replaceState' : 'pushState'](null, '', to);
  emit();
}

/** A query parameter as state: only non-defaults reach the URL (replaceState). */
export function useParam(key: string, def: string): [string, (v: string | null) => void] {
  const loc = useLoc();
  const set = useCallback(
    (v: string | null) => {
      const cur = parseLoc(location.pathname + location.search);
      const q = { ...cur.query };
      if (v === null || v === def) delete q[key];
      else q[key] = v;
      navigate(formatLoc({ path: cur.path, query: q }), true);
    },
    [key, def],
  );
  return [loc.query[key] ?? def, set];
}
export function useNumParam(key: string, def: number): [number, (v: number | null) => void] {
  const [s, set] = useParam(key, String(def));
  const n = Number(s);
  return [Number.isFinite(n) ? n : def, (v) => set(v === null ? null : String(v))];
}

export const link = (to: string) => (e: React.MouseEvent) => {
  if (e.metaKey || e.ctrlKey || e.shiftKey || e.button !== 0) return;
  e.preventDefault();
  navigate(to);
};
