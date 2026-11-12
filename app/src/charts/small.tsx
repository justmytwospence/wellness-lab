import { line as d3line } from 'd3-shape';
import { PAL } from '@wl/views';

/** A small line chart with no axes. */
export function Spark({ values, width = 160, height = 32, color = PAL.hrv, band }: { values: number[]; width?: number; height?: number; color?: string; band?: [number, number] }) {
  const f = values.map((v, i) => [i, v] as const).filter((p) => Number.isFinite(p[1]));
  if (f.length < 2) return <svg width={width} height={height} />;
  const ys = f.map((p) => p[1]).concat(band ?? []);
  const [lo, hi] = [Math.min(...ys), Math.max(...ys)];
  const x = (i: number) => (i / (values.length - 1)) * (width - 2) + 1;
  const y = (v: number) => height - 2 - ((v - lo) / (hi - lo || 1)) * (height - 4);
  const d = d3line<readonly [number, number]>().x((p) => x(p[0])).y((p) => y(p[1]))(f as never) ?? '';
  return (
    <svg width={width} height={height}>
      {band && <rect x={0} width={width} y={y(band[1])} height={Math.max(1, y(band[0]) - y(band[1]))} fill={PAL.base} opacity={0.18} />}
      <path d={d} fill="none" stroke={color} strokeWidth={1.5} />
    </svg>
  );
}

/** A value on a band: a dot on a bar with the band shaded, for "last night against your range". */
export function BandBar({ v, lo, hi, width = 120 }: { v: number; lo: number; hi: number; width?: number }) {
  if (![v, lo, hi].every(Number.isFinite)) return <svg width={width} height={14} />;
  const span = hi - lo || 1;
  const min = lo - span, max = hi + span;
  const x = (a: number) => Math.min(width - 4, Math.max(4, ((a - min) / (max - min)) * width));
  const out = v < lo || v > hi;
  return (
    <svg width={width} height={14}>
      <rect x={0} y={5} width={width} height={4} rx={2} fill={PAL.grid} />
      <rect x={x(lo)} y={4} width={x(hi) - x(lo)} height={6} rx={2} fill={PAL.base} opacity={0.35} />
      <circle cx={x(v)} cy={7} r={4} fill={out ? PAL.alert : PAL.hrv} />
    </svg>
  );
}

export function Empty({ children }: { children: React.ReactNode }) {
  return <div className="empty">{children}</div>;
}

export function Grade({ g }: { g: 'A' | 'B' | 'C' | 'D' }) {
  const text = { A: 'Solid on wrist data', B: 'Good for your own trends', C: 'Experimental', D: 'Shown for completeness; wrist sensors measure this poorly' }[g];
  return <span className={`grade ${g}`} title={text}>{g}</span>;
}
