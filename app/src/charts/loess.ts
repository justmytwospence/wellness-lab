/** Local linear regression with tricube weights (what Vega's loess transform does): `bandwidth` is the share of points in each fit. */
export function loess(xs: number[], ys: number[], bandwidth = 0.5, points = 60): [number, number][] {
  const n = xs.length;
  if (n < 5) return [];
  const idx = xs.map((_, i) => i).sort((a, b) => xs[a]! - xs[b]!);
  const x = idx.map((i) => xs[i]!), y = idx.map((i) => ys[i]!);
  const k = Math.max(3, Math.min(n, Math.floor(bandwidth * n)));
  const lo = x[0]!, hi = x[n - 1]!;
  const out: [number, number][] = [];
  for (let p = 0; p < points; p++) {
    const x0 = lo + ((hi - lo) * p) / (points - 1);
    const d = x.map((v) => Math.abs(v - x0));
    const h = [...d].sort((a, b) => a - b)[k - 1]! || 1e-9;
    let sw = 0, sx = 0, sy = 0, sxx = 0, sxy = 0;
    for (let i = 0; i < n; i++) {
      const u = d[i]! / h;
      if (u >= 1) continue;
      const w = (1 - u ** 3) ** 3;
      sw += w; sx += w * x[i]!; sy += w * y[i]!; sxx += w * x[i]! * x[i]!; sxy += w * x[i]! * y[i]!;
    }
    if (sw === 0) continue;
    const den = sw * sxx - sx * sx;
    const b = Math.abs(den) < 1e-12 ? 0 : (sw * sxy - sx * sy) / den;
    const a = (sy - b * sx) / sw;
    out.push([x0, a + b * x0]);
  }
  return out;
}

/** Quantile by the nearest rank (polars' default for `quantile`). */
export function quantile(sorted: number[], q: number): number {
  if (!sorted.length) return NaN;
  return sorted[Math.min(sorted.length - 1, Math.max(0, Math.round((sorted.length - 1) * q)))]!;
}
export const sortedFinite = (v: ArrayLike<number>): number[] => Array.from(v).filter(Number.isFinite).sort((a, b) => a - b);
