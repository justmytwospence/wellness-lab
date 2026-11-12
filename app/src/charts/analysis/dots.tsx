import { PAL, diverging } from '@wl/views';

/** Cliff's delta (or any -1..1 effect) as a bar from the centre. */
export function DeltaBar({ v, width = 130, domain = 1 }: { v: number; width?: number; domain?: number }) {
  if (!Number.isFinite(v)) return <svg width={width} height={14} />;
  const mid = width / 2, w = (Math.min(1, Math.abs(v) / domain) * width) / 2;
  const color = diverging((v / domain) * 3);
  return (
    <svg width={width} height={14} role="img" aria-label={`effect ${v.toFixed(2)}`}>
      <rect x={0} y={6} width={width} height={2} fill={PAL.grid} />
      <rect x={v < 0 ? mid - w : mid} y={2} width={Math.max(1, w)} height={10} fill={color} stroke={v < 0 ? PAL.base : PAL.alert} strokeWidth={0.5} />
      <line x1={mid} x2={mid} y1={0} y2={14} stroke={PAL.muted} />
    </svg>
  );
}

/** Deterministic jitter in -1..1 so strips do not move between renders. */
const jit = (i: number) => ((Math.sin(i * 12.9898) * 43758.5453) % 1 + 1) % 1 * 2 - 1;

function sample(v: number[], max: number): number[] {
  if (v.length <= max) return v;
  const step = v.length / max;
  return Array.from({ length: max }, (_, i) => v[Math.floor(i * step)]!);
}

/** Two rows of dots on one axis: the chosen nights (top) and the base nights (bottom), medians as ticks. */
export function PairedStrip({ sel, base, width = 260, selColor = PAL.alert, baseColor = PAL.base }: { sel: number[]; base: number[]; width?: number; selColor?: string; baseColor?: string }) {
  const all = [...sel, ...base].filter(Number.isFinite).sort((a, b) => a - b);
  if (!all.length) return <svg width={width} height={34} />;
  const q = (p: number) => all[Math.min(all.length - 1, Math.floor(p * (all.length - 1)))]!;
  const lo = q(0.01), hi = q(0.99), span = hi - lo || 1;
  const x = (v: number) => 4 + Math.min(1, Math.max(0, (v - lo) / span)) * (width - 8);
  const med = (v: number[]) => { const s = v.filter(Number.isFinite).sort((a, b) => a - b); return s.length ? s[Math.floor((s.length - 1) / 2)]! : NaN; };
  const row = (v: number[], y: number, c: string) => (
    <g>
      {sample(v.filter(Number.isFinite), 120).map((d, i) => <circle key={i} cx={x(d)} cy={y + jit(i) * 4} r={1.8} fill={c} opacity={0.55} />)}
      {Number.isFinite(med(v)) && <line x1={x(med(v))} x2={x(med(v))} y1={y - 7} y2={y + 7} stroke={c} strokeWidth={2} />}
    </g>
  );
  return (
    <svg width={width} height={34} role="img" aria-label="Selected nights against base nights" data-testid="paired-strip">
      {row(sel, 9, selColor)}
      {row(base, 25, baseColor)}
    </svg>
  );
}

/** A coefficient with its standard error as a whisker. */
export function Whisker({ b, se, lo, hi, width = 200 }: { b: number; se: number; lo: number; hi: number; width?: number }) {
  if (!Number.isFinite(b)) return <svg width={width} height={14} />;
  const s = Number.isFinite(se) ? se : 0;
  const span = hi - lo || 1;
  const x = (v: number) => 4 + ((v - lo) / span) * (width - 8);
  return (
    <svg width={width} height={14} role="img" aria-label={`coefficient ${b.toFixed(2)}`}>
      <line x1={x(0)} x2={x(0)} y1={0} y2={14} stroke={PAL.grid} />
      {s > 0 && <line x1={x(b - s)} x2={x(b + s)} y1={7} y2={7} stroke={PAL.base} strokeWidth={2} />}
      <circle cx={x(b)} cy={7} r={3.5} fill={PAL.hrv} />
    </svg>
  );
}

/** Horizontal bars with labels: loadings, shares. */
export function HBars({ items, width = 360, min, max, color = PAL.hrv }: { items: { label: string; value: number; note?: string }[]; width?: number; min?: number; max?: number; color?: string }) {
  const vals = items.map((i) => i.value).filter(Number.isFinite);
  const lo = min ?? Math.min(0, ...vals), hi = max ?? Math.max(0, ...vals);
  const span = hi - lo || 1, lw = 190, bw = width - lw - 60;
  const x = (v: number) => lw + ((v - lo) / span) * bw;
  return (
    <svg width={width} height={items.length * 22 + 4} role="img" aria-label="bars" data-testid="hbars">
      {items.map((it, i) => (
        <g key={it.label}>
          <text x={lw - 6} y={i * 22 + 15} textAnchor="end" fontSize={11} fill={PAL.muted}>{it.label}</text>
          {Number.isFinite(it.value) && <rect x={Math.min(x(0), x(it.value))} y={i * 22 + 4} width={Math.max(1, Math.abs(x(it.value) - x(0)))} height={14} fill={it.value < 0 ? PAL.base : color} rx={2} />}
          <text x={lw + bw + 6} y={i * 22 + 15} fontSize={11} className="mono" fill={PAL.text}>{Number.isFinite(it.value) ? it.value.toFixed(2) : '\u2013'}{it.note ?? ''}</text>
        </g>
      ))}
      <line x1={x(0)} x2={x(0)} y1={0} y2={items.length * 22 + 4} stroke={PAL.grid} />
    </svg>
  );
}

/** Inverse normal CDF by bisection (twenty dots do not need more). */
export function invNormal(p: number): number {
  const cdf = (z: number) => 0.5 * (1 + erf(z / Math.SQRT2));
  let [lo, hi] = [-6, 6];
  for (let i = 0; i < 60; i++) { const m = (lo + hi) / 2; if (cdf(m) < p) lo = m; else hi = m; }
  return (lo + hi) / 2;
}
function erf(x: number): number {
  const t = 1 / (1 + 0.3275911 * Math.abs(x));
  const y = 1 - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t * Math.exp(-x * x);
  return x >= 0 ? y : -y;
}

/** A quantile dotplot: each dot is a 1-in-N chance of the true effect; the SWC is shaded, zero is marked. */
export function QuantileDots({ q, swc, width = 560 }: { q: number[]; swc: number; width?: number }) {
  if (!q.length) return null;
  const lo = Math.min(...q, -swc), hi = Math.max(...q, swc), pad = (hi - lo) * 0.08 || 1;
  const a = lo - pad, b = hi + pad;
  const x = (v: number) => 10 + ((v - a) / (b - a)) * (width - 20);
  const r = 7, bins = Math.max(8, Math.floor((width - 20) / (2 * r)));
  const stacks = new Map<number, number>();
  const dots = q.map((v) => {
    const k = Math.round(((v - a) / (b - a)) * bins);
    const h = stacks.get(k) ?? 0;
    stacks.set(k, h + 1);
    return { cx: x(a + (k / bins) * (b - a)), h, v };
  });
  const tall = Math.max(...[...stacks.values()]);
  const H = tall * (2 * r + 1) + 30;
  const base = H - 22;
  return (
    <svg width={width} height={H} role="img" aria-label="Quantile dotplot of the effect" data-testid="quantile-dots">
      {swc > 0 && <rect x={x(-swc)} y={0} width={Math.max(1, x(swc) - x(-swc))} height={base + 4} fill={PAL.base} opacity={0.16} />}
      <line x1={x(0)} x2={x(0)} y1={0} y2={base + 4} stroke={PAL.muted} strokeDasharray="3 3" />
      <line x1={10} x2={width - 10} y1={base + 4} y2={base + 4} stroke={PAL.grid} />
      {dots.map((d, i) => <circle key={i} cx={d.cx} cy={base - r - d.h * (2 * r + 1)} r={r} fill={d.v > swc ? PAL.hrv : d.v < -swc ? PAL.alert : PAL.base} opacity={0.9} />)}
      <text x={x(0)} y={H - 6} textAnchor="middle" fontSize={11} fill={PAL.muted}>0</text>
      <text x={x(-swc)} y={H - 6} textAnchor="middle" fontSize={11} fill={PAL.muted}>{swc > 0 ? `\u2212${swc.toPrecision(2)}` : ''}</text>
      <text x={x(swc)} y={H - 6} textAnchor="middle" fontSize={11} fill={PAL.muted}>{swc > 0 ? `+${swc.toPrecision(2)}` : ''}</text>
    </svg>
  );
}
