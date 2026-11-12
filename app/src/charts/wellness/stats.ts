/** Small numeric helpers shared by the chart components. Missing values are NaN or null; they never enter a statistic. */

export const isNum = (v: number | null | undefined): v is number => v !== null && v !== undefined && Number.isFinite(v);

/** Linear-interpolated quantile of a sorted array. */
export function quantileSorted(s: number[], p: number): number {
  if (!s.length) return NaN;
  const x = (s.length - 1) * p;
  const i = Math.floor(x);
  return i + 1 < s.length ? s[i]! + (s[i + 1]! - s[i]!) * (x - i) : s[i]!;
}

export const sortedFinite = (v: ArrayLike<number | null | undefined>): number[] => Array.from(v as ArrayLike<number>).filter(isNum).sort((a, b) => a - b);
export const quantile = (v: ArrayLike<number | null | undefined>, p: number): number => quantileSorted(sortedFinite(v), p);
export const mean = (v: ArrayLike<number | null | undefined>): number => {
  let s = 0, n = 0;
  for (const x of Array.from(v as ArrayLike<number>)) if (isNum(x)) (s += x), n++;
  return n ? s / n : NaN;
};

/** Trailing rolling mean over `win` entries (the array is dense by day), NaN when fewer than `minN` values are present. */
export function rolling(v: ArrayLike<number>, win: number, minN = Math.ceil(win / 2)): number[] {
  const out = new Array<number>(v.length).fill(NaN);
  let sum = 0, cnt = 0;
  for (let i = 0; i < v.length; i++) {
    const a = v[i]!;
    if (isNum(a)) (sum += a), cnt++;
    if (i >= win) {
      const b = v[i - win]!;
      if (isNum(b)) (sum -= b), cnt--;
    }
    if (cnt >= minN) out[i] = sum / cnt;
  }
  return out;
}

/** Put sparse (day, value) columns on a dense daily grid from..to; absent days are NaN so lines break there. */
export function densify(days: ArrayLike<number>, cols: Record<string, ArrayLike<number | null>>, from: number, to: number): { x: number[]; cols: Record<string, number[]> } {
  const n = Math.max(0, to - from + 1);
  const x = Array.from({ length: n }, (_, i) => from + i);
  const out: Record<string, number[]> = {};
  for (const k of Object.keys(cols)) out[k] = new Array<number>(n).fill(NaN);
  for (let i = 0; i < days.length; i++) {
    const j = days[i]! - from;
    if (j < 0 || j >= n) continue;
    for (const k of Object.keys(cols)) {
      const v = cols[k]![i];
      out[k]![j] = isNum(v) ? v : NaN;
    }
  }
  return { x, cols: out };
}

export interface Box { n: number; lo: number; q1: number; med: number; q3: number; hi: number; outliers: number[] }
/** Tukey box: whiskers reach the furthest value within 1.5 IQR. */
export function boxStats(v: ArrayLike<number | null | undefined>): Box | null {
  const s = sortedFinite(v);
  if (s.length < 3) return null;
  const q1 = quantileSorted(s, 0.25), med = quantileSorted(s, 0.5), q3 = quantileSorted(s, 0.75);
  const iqr = q3 - q1;
  const inside = s.filter((x) => x >= q1 - 1.5 * iqr && x <= q3 + 1.5 * iqr);
  return { n: s.length, lo: inside[0]!, q1, med, q3, hi: inside[inside.length - 1]!, outliers: s.filter((x) => x < q1 - 1.5 * iqr || x > q3 + 1.5 * iqr) };
}

/** Histogram with fixed-width bins aligned to multiples of `step`. */
export function histogram(v: ArrayLike<number | null | undefined>, step: number): { x0: number; x1: number; count: number }[] {
  const f = Array.from(v as ArrayLike<number>).filter(isNum);
  if (!f.length) return [];
  const lo = Math.floor(Math.min(...f) / step) * step;
  const hi = Math.floor(Math.max(...f) / step) * step;
  const bins = Array.from({ length: Math.round((hi - lo) / step) + 1 }, (_, i) => ({ x0: lo + i * step, x1: lo + (i + 1) * step, count: 0 }));
  for (const x of f) bins[Math.min(bins.length - 1, Math.floor((x - lo) / step))]!.count++;
  return bins;
}

/** Month index (y * 12 + m) of a day number, and its first day. */
export const monthOf = (day: number): number => {
  const d = new Date(day * 86_400_000);
  return d.getUTCFullYear() * 12 + d.getUTCMonth();
};
export const monthStartDay = (mi: number): number => Math.floor(Date.UTC(Math.floor(mi / 12), mi % 12, 1) / 86_400_000);
export const yearOf = (day: number): number => new Date(day * 86_400_000).getUTCFullYear();
