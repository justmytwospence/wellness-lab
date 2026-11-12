/** Route plus query as the app's state: only non-defaults are written, so URLs stay short and shareable. */
export interface Loc { path: string; query: Record<string, string> }

export const parseLoc = (href: string): Loc => {
  const u = new URL(href, 'http://x');
  const query: Record<string, string> = {};
  u.searchParams.forEach((v, k) => (query[k] = v));
  return { path: u.pathname, query };
};

export function formatLoc(loc: Loc, defaults: Record<string, string> = {}): string {
  const p = new URLSearchParams();
  for (const k of Object.keys(loc.query).sort()) {
    const v = loc.query[k];
    if (v !== undefined && v !== '' && v !== defaults[k]) p.set(k, v);
  }
  const q = p.toString();
  return q ? `${loc.path}?${q}` : loc.path;
}

/** "a-b,c-d" day ranges, as the Compare view's `sel`. */
export const parseSel = (s: string | undefined): [number, number][] =>
  (s ?? '').split(',').filter(Boolean).map((p) => {
    const [a, b] = p.split('-').map(Number);
    return [a!, b ?? a!] as [number, number];
  }).filter((r) => Number.isFinite(r[0]) && Number.isFinite(r[1]));
export const formatSel = (r: [number, number][]): string => r.map(([a, b]) => (a === b ? `${a}` : `${a}-${b}`)).join(',');
