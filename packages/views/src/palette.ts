/** The "Night monitor" palette, from the HRV notebook. */
export const PAL = {
  bg: '#0b1020', panel: '#111831', panel2: '#172042', grid: '#222c52', text: '#e6ebff', muted: '#8892b8',
  hrv: '#2ee6c5', hr: '#ffb547', alert: '#ff6b6b', base: '#7aa2ff', violet: '#b892ff',
} as const;

export const ADVICE_COLORS = { normal: '#2ee6c5', high: '#7aa2ff', watch: '#ffb547', easy: '#ff9a5c', back_off: '#ff6b6b' } as const;
export const LOAD_COLORS = { rest: '#7aa2ff', easy: '#2ee6c5', moderate: '#ffb547', hard: '#ff6b6b' } as const;
export const STRESS_BANDS = ['#2ee6c5', '#7aa2ff', '#ffb547', '#ff6b6b'] as const;

function hex(c: string): [number, number, number] {
  const n = parseInt(c.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}
export function mix(a: string, b: string, t: number): string {
  const [x, y] = [hex(a), hex(b)];
  const m = x.map((v, i) => Math.round(v + (y[i]! - v) * Math.min(1, Math.max(0, t))));
  return `#${m.map((v) => v.toString(16).padStart(2, '0')).join('')}`;
}
/** A sequential color for t in 0..1 from dark to the HRV teal to amber. */
export function sequential(t: number): string {
  const stops = ['#16204a', '#2c4aa0', '#2ee6c5', '#ffb547'];
  const x = Math.min(1, Math.max(0, t)) * (stops.length - 1);
  const i = Math.min(stops.length - 2, Math.floor(x));
  return mix(stops[i]!, stops[i + 1]!, x - i);
}
/** Diverging color for z in about -3..3: blue below, dark at 0, red above. */
export function diverging(z: number): string {
  const t = Math.min(1, Math.abs(z) / 3);
  return mix('#16204a', z < 0 ? '#7aa2ff' : '#ff6b6b', t);
}
