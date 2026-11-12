import { useState } from 'react';
import { scaleLinear } from 'd3-scale';
import { area as d3area, line as d3line } from 'd3-shape';
import { PAL, clock } from '@wl/views';
import { AX, Sized, YAxis } from './svg';

/** The engine's `Typical` struct: per-bin quantiles across nights (bins with fewer than 5 nights are absent). */
export interface Typ { minute: number[]; p05: number[]; p25: number[]; p50: number[]; p75: number[]; p95: number[]; n: number[] }
export type AlignMode = 'onset' | 'clock';

/** Clock runs from 20:00 the evening before; onset runs from falling asleep. */
export const CLOCK_START_MIN = 20 * 60;
export const minuteLabel = (m: number, align: AlignMode): string => (align === 'clock' ? clock(m + CLOCK_START_MIN) : `+${Math.floor(m / 60)}:${String(Math.round(m % 60)).padStart(2, '0')}`);

interface P { m: number; lo: number; hi: number; mid: number }
/** Points for one band, with a break (NaN) wherever bins are missing. */
function pts(t: Typ, lo: keyof Typ, hi: keyof Typ, bin: number): P[] {
  const out: P[] = [];
  t.minute.forEach((m, i) => {
    if (i > 0 && m - t.minute[i - 1]! > bin * 1.5) out.push({ m: NaN, lo: NaN, hi: NaN, mid: NaN });
    out.push({ m, lo: t[lo][i]!, hi: t[hi][i]!, mid: t.p50[i]! });
  });
  return out;
}

export interface QuantileBandsProps { a: Typ; b?: Typ | null; align: AlignMode; bin?: number; unit?: string; fmt?: (v: number) => string; height?: number; span: number; labelA?: string; labelB?: string }

/** The AGP-style chart: nested 5-95 and 25-75 bands around the median, per bin, with the nights per bin under the axis. */
export function QuantileBands({ a, b, align, bin = 10, unit = '', fmt = (v) => v.toFixed(1), height = 340, span, labelA = 'A', labelB = 'B' }: QuantileBandsProps) {
  const [hover, setHover] = useState<number | null>(null);
  const series = [{ t: a, color: PAL.hrv, label: labelA }, ...(b ? [{ t: b, color: PAL.hr, label: labelB }] : [])];
  const vals = series.flatMap((s) => [...s.t.p05, ...s.t.p95]).filter(Number.isFinite);
  const nmax = Math.max(1, ...series.flatMap((s) => s.t.n));
  const strip = 22;
  const near = (t: Typ, m: number) => {
    let best = -1, bd = Infinity;
    t.minute.forEach((x, i) => { const d = Math.abs(x - m); if (d < bd) (bd = d), (best = i); });
    return bd <= bin ? best : -1;
  };
  return (
    <div data-testid="chart">
      <Sized height={height}>
        {(w) => {
          const m = { l: 46, r: 10, t: 8, b: 24 + strip * series.length };
          const x = scaleLinear().domain([0, span]).range([m.l, w - m.r]);
          const y = scaleLinear().domain([Math.min(...vals), Math.max(...vals)]).range([height - m.b, m.t]).nice();
          const step = align === 'clock' ? 120 : 60;
          const ticks = Array.from({ length: Math.floor(span / step) + 1 }, (_, i) => i * step);
          const band = (t: Typ, lo: keyof Typ, hi: keyof Typ) => d3area<P>().defined((p) => Number.isFinite(p.m)).x((p) => x(p.m + bin / 2)).y0((p) => y(p.lo)).y1((p) => y(p.hi))(pts(t, lo, hi, bin)) ?? '';
          const med = (t: Typ) => d3line<P>().defined((p) => Number.isFinite(p.m)).x((p) => x(p.m + bin / 2)).y((p) => y(p.mid))(pts(t, 'p50', 'p50', bin)) ?? '';
          return (
            <svg width={w} height={height} role="img" aria-label="Typical night quantile bands" onMouseLeave={() => setHover(null)}
              onMouseMove={(e) => { const r = e.currentTarget.getBoundingClientRect(); const mn = ((e.clientX - r.left - m.l) / (w - m.l - m.r)) * span; setHover(mn < 0 || mn > span ? null : (a.minute[near(a, mn)] ?? null)); }}>
              <YAxis scale={y} x={m.l} w={w - m.l - m.r} fmt={fmt} />
              {ticks.map((t) => (
                <g key={t} transform={`translate(${x(t)},${height - m.b})`}><line y2={4} stroke={AX.stroke} /><text y={16} textAnchor={t === span ? 'end' : 'middle'} fontSize={11} fill={AX.stroke} fontFamily="JetBrains Mono, monospace">{minuteLabel(t, align)}</text></g>
              ))}
              {series.map((s) => (
                <g key={s.label}>
                  <path d={band(s.t, 'p05', 'p95')} fill={s.color} fillOpacity={0.14} />
                  <path d={band(s.t, 'p25', 'p75')} fill={s.color} fillOpacity={0.3} />
                  <path d={med(s.t)} fill="none" stroke={s.color} strokeWidth={2} strokeDasharray={s.label === labelB ? '5 3' : undefined} />
                </g>
              ))}
              {series.map((s, k) => {
                const top = height - m.b + 24 + k * strip;
                const bw = Math.max(1, (w - m.l - m.r) / (span / bin) - 1);
                return (
                  <g key={s.label}>
                    <text x={m.l - 6} y={top + strip / 2 - 2} textAnchor="end" fontSize={10} fill={AX.stroke}>n {s.label}</text>
                    <text x={m.l - 6} y={top + strip / 2 + 9} textAnchor="end" fontSize={9} fill={AX.stroke}>max {Math.max(...s.t.n)}</text>
                    {s.t.minute.map((mn, i) => <rect key={mn} x={x(mn)} width={bw} y={top + strip - 4 - (s.t.n[i]! / nmax) * (strip - 6)} height={(s.t.n[i]! / nmax) * (strip - 6)} fill={s.color} fillOpacity={0.55} />)}
                  </g>
                );
              })}
              {hover !== null && <line x1={x(hover + bin / 2)} x2={x(hover + bin / 2)} y1={m.t} y2={height - m.b} stroke={PAL.text} strokeOpacity={0.5} />}
            </svg>
          );
        }}
      </Sized>
      <div className="mono muted" style={{ fontSize: 12, minHeight: 18 }} aria-live="polite">
        {hover === null ? 'Hover for the quantiles of a bin.' : series.map((s) => {
          const i = near(s.t, hover);
          if (i < 0) return null;
          return <span key={s.label} style={{ marginRight: 16, color: s.color }}>{series.length > 1 ? `${s.label} ` : ''}{minuteLabel(s.t.minute[i]!, align)}: median {fmt(s.t.p50[i]!)}{unit}, 25-75% {fmt(s.t.p25[i]!)} to {fmt(s.t.p75[i]!)}, 5-95% {fmt(s.t.p05[i]!)} to {fmt(s.t.p95[i]!)}, n {s.t.n[i]}</span>;
        })}
      </div>
    </div>
  );
}

/** One tiny median curve (with its 25-75 band) per period, all on one y scale. */
export function SmallMultiples({ items, domain, span, bin = 10 }: { items: { label: string; t: Typ | null }[]; domain: [number, number]; span: number; bin?: number }) {
  const W = 220, H = 72;
  const x = scaleLinear().domain([0, span]).range([2, W - 2]);
  const y = scaleLinear().domain(domain).range([H - 3, 3]);
  return (
    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(190px, 1fr))', gap: 8 }} data-testid="chart">
      {items.map((it) => {
        const t = it.t && it.t.minute.length ? it.t : null;
        const band = t ? d3area<P>().defined((p) => Number.isFinite(p.m)).x((p) => x(p.m + bin / 2)).y0((p) => y(p.lo)).y1((p) => y(p.hi))(pts(t, 'p25', 'p75', bin)) : null;
        const med = t ? d3line<P>().defined((p) => Number.isFinite(p.m)).x((p) => x(p.m + bin / 2)).y((p) => y(p.mid))(pts(t, 'p50', 'p50', bin)) : null;
        return (
          <div key={it.label} style={{ background: PAL.panel2, borderRadius: 6, padding: '4px 6px' }}>
            <div className="muted mono" style={{ fontSize: 11 }}>{it.label}{t ? '' : ' (too few nights)'}</div>
            <svg viewBox={`0 0 ${W} ${H}`} width="100%" height={H} preserveAspectRatio="none" role="img" aria-label={`Median curve for ${it.label}`}>
              {band && <path d={band} fill={PAL.hrv} fillOpacity={0.25} />}
              {med && <path d={med} fill="none" stroke={PAL.hrv} strokeWidth={1.5} vectorEffect="non-scaling-stroke" />}
            </svg>
          </div>
        );
      })}
    </div>
  );
}
