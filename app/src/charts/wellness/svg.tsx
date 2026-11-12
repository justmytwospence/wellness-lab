import { useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { scaleLinear } from 'd3-scale';
import { area as d3area, stack as d3stack, curveMonotoneX } from 'd3-shape';
import { PAL } from '@wl/views';
import type { Box } from './stats';
import { monthOf, monthStartDay } from './stats';

export const AX = { stroke: PAL.muted, grid: PAL.grid, font: '11px "JetBrains Mono", monospace' } as const;
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

/** The width of a container, tracked; children draw into it. */
export function Sized({ height, children, className }: { height: number; children: (w: number) => ReactNode; className?: string }) {
  const el = useRef<HTMLDivElement>(null);
  const [w, setW] = useState(0);
  useLayoutEffect(() => {
    const host = el.current!;
    setW(host.clientWidth);
    const ro = new ResizeObserver(() => setW(host.clientWidth));
    ro.observe(host);
    return () => ro.disconnect();
  }, []);
  return <div ref={el} className={className} style={{ width: '100%', minHeight: height }}>{w > 0 && children(w)}</div>;
}

/** Tick days for a date axis: month starts, thinned to the span. */
export function dayTicks(d0: number, d1: number): { day: number; label: string }[] {
  const span = d1 - d0;
  const step = span <= 200 ? 1 : span <= 520 ? 2 : span <= 1100 ? 3 : span <= 2200 ? 6 : 12;
  const out: { day: number; label: string }[] = [];
  for (let m = monthOf(d0); monthStartDay(m) <= d1; m++) {
    const day = monthStartDay(m);
    if (day < d0 || (step > 1 && m % step !== 0)) continue;
    out.push({ day, label: m % 12 === 0 ? String(Math.floor(m / 12)) : MONTHS[m % 12]! });
  }
  return out;
}

export function YAxis({ scale, x, w, fmt = (v: number) => String(v), ticks = 5, grid = true }: { scale: ReturnType<typeof scaleLinear<number, number>>; x: number; w: number; fmt?: (v: number) => string; ticks?: number; grid?: boolean }) {
  return (
    <g fontFamily="JetBrains Mono, monospace" fontSize={11} fill={AX.stroke}>
      {scale.ticks(ticks).map((t) => (
        <g key={t} transform={`translate(0,${scale(t)})`}>
          {grid && <line x1={x} x2={x + w} stroke={AX.grid} />}
          <text x={x - 6} textAnchor="end" dy="0.32em">{fmt(t)}</text>
        </g>
      ))}
    </g>
  );
}

export function DayAxis({ d0, d1, x0, x1, y }: { d0: number; d1: number; x0: number; x1: number; y: number }) {
  const sx = scaleLinear().domain([d0, d1]).range([x0, x1]);
  return (
    <g fontFamily="JetBrains Mono, monospace" fontSize={11} fill={AX.stroke}>
      {dayTicks(d0, d1).map((t) => (
        <g key={t.day} transform={`translate(${sx(t.day)},${y})`}><line y2={4} stroke={AX.stroke} /><text y={16} textAnchor="middle">{t.label}</text></g>
      ))}
    </g>
  );
}

/** Box plots, one per group, sharing a y axis. */
export function BoxPlots({ groups, color = PAL.hrv, height = 200, fmt, unit }: { groups: { label: string; box: Box | null }[]; color?: string; height?: number; fmt?: (v: number) => string; unit?: string }) {
  const all = groups.flatMap((g) => (g.box ? [g.box.lo, g.box.hi, ...g.box.outliers] : []));
  return (
    <Sized height={height}>
      {(w) => {
        const m = { l: 44, r: 8, t: 8, b: 24 };
        const lo = Math.min(...all), hi = Math.max(...all);
        const pad = (hi - lo || 1) * 0.06;
        const y = scaleLinear().domain([lo - pad, hi + pad]).range([height - m.b, m.t]).nice();
        const bw = (w - m.l - m.r) / Math.max(1, groups.length);
        return (
          <svg width={w} height={height} role="img" aria-label={`Box plots${unit ? ` in ${unit}` : ''}`}>
            <YAxis scale={y} x={m.l} w={w - m.l - m.r} fmt={fmt} />
            {groups.map((g, i) => {
              const cx = m.l + bw * (i + 0.5);
              const half = Math.min(18, bw * 0.3);
              const b = g.box;
              return (
                <g key={g.label}>
                  <text x={cx} y={height - 8} textAnchor="middle" fontSize={11} fill={AX.stroke} fontFamily="JetBrains Mono, monospace">{g.label}</text>
                  {b && (
                    <g>
                      <title>{`${g.label}: median ${b.med.toFixed(2)}, IQR ${b.q1.toFixed(2)} to ${b.q3.toFixed(2)}, n ${b.n}`}</title>
                      <line x1={cx} x2={cx} y1={y(b.lo)} y2={y(b.hi)} stroke={color} />
                      <rect x={cx - half} width={half * 2} y={y(b.q3)} height={Math.max(1, y(b.q1) - y(b.q3))} fill={color} fillOpacity={0.3} stroke={color} />
                      <line x1={cx - half} x2={cx + half} y1={y(b.med)} y2={y(b.med)} stroke={PAL.text} strokeWidth={2} />
                      {b.outliers.slice(0, 40).map((o, k) => <circle key={k} cx={cx} cy={y(o)} r={1.8} fill={color} fillOpacity={0.5} />)}
                    </g>
                  )}
                </g>
              );
            })}
          </svg>
        );
      }}
    </Sized>
  );
}

/** Vertical bars from precomputed bins. */
export function Histogram({ bins, color = PAL.hrv, height = 200, xLabel, median }: { bins: { x0: number; x1: number; count: number }[]; color?: string; height?: number; xLabel?: string; median?: number }) {
  return (
    <Sized height={height}>
      {(w) => {
        const m = { l: 40, r: 8, t: 8, b: 34 };
        const x = scaleLinear().domain([bins[0]?.x0 ?? 0, bins[bins.length - 1]?.x1 ?? 1]).range([m.l, w - m.r]);
        const y = scaleLinear().domain([0, Math.max(1, ...bins.map((b) => b.count))]).range([height - m.b, m.t]).nice();
        return (
          <svg width={w} height={height} role="img" aria-label="Histogram">
            <YAxis scale={y} x={m.l} w={w - m.l - m.r} />
            {bins.map((b) => (
              <rect key={b.x0} x={x(b.x0) + 1} width={Math.max(1, x(b.x1) - x(b.x0) - 2)} y={y(b.count)} height={y(0) - y(b.count)} fill={color} rx={2}>
                <title>{`${b.x0} to ${b.x1}: ${b.count} nights`}</title>
              </rect>
            ))}
            {x.ticks(8).map((t) => <text key={t} x={x(t)} y={height - m.b + 14} textAnchor="middle" fontSize={11} fill={AX.stroke} fontFamily="JetBrains Mono, monospace">{t}</text>)}
            {median !== undefined && Number.isFinite(median) && <g><line x1={x(median)} x2={x(median)} y1={m.t} y2={height - m.b} stroke={PAL.hr} strokeDasharray="4 3" /><text x={x(median) + 4} y={m.t + 10} fontSize={11} fill={PAL.hr}>median {median.toFixed(0)}</text></g>}
            {xLabel && <text x={(m.l + w - m.r) / 2} y={height - 4} textAnchor="middle" fontSize={11} fill={AX.stroke}>{xLabel}</text>}
          </svg>
        );
      }}
    </Sized>
  );
}

export interface StackSpec { key: string; label: string; color: string }

/** Stacked bars by month (or any dated category). rows[i].values aligns with `keys`. */
export function StackedBars({ rows, keys, height = 220, fmt = (v: number) => v.toFixed(1), unit = '' }: { rows: { day: number; values: number[] }[]; keys: StackSpec[]; height?: number; fmt?: (v: number) => string; unit?: string }) {
  return (
    <Sized height={height}>
      {(w) => {
        const m = { l: 40, r: 8, t: 8, b: 26 };
        const tot = rows.map((r) => r.values.reduce((a, b) => a + (Number.isFinite(b) ? b : 0), 0));
        const y = scaleLinear().domain([0, Math.max(1, ...tot)]).range([height - m.b, m.t]).nice();
        const bw = (w - m.l - m.r) / Math.max(1, rows.length);
        return (
          <svg width={w} height={height} role="img" aria-label="Stacked bars">
            <YAxis scale={y} x={m.l} w={w - m.l - m.r} fmt={(v) => fmt(v)} />
            {rows.map((r, i) => {
              let acc = 0;
              const d = new Date(r.day * 86_400_000);
              return (
                <g key={r.day}>
                  {r.values.map((v, k) => {
                    const h = Number.isFinite(v) ? v : 0;
                    const y1 = y(acc + h), y0 = y(acc);
                    acc += h;
                    return <rect key={k} x={m.l + i * bw + 1} width={Math.max(1, bw - 2)} y={y1} height={Math.max(0, y0 - y1)} fill={keys[k]!.color}><title>{`${MONTHS[d.getUTCMonth()]} ${d.getUTCFullYear()} ${keys[k]!.label}: ${fmt(h)}${unit}`}</title></rect>;
                  })}
                  {(rows.length <= 14 || i % Math.ceil(rows.length / 12) === 0) && <text x={m.l + i * bw + bw / 2} y={height - 8} textAnchor="middle" fontSize={10} fill={AX.stroke} fontFamily="JetBrains Mono, monospace">{d.getUTCMonth() === 0 || i === 0 ? `${MONTHS[d.getUTCMonth()]} ${String(d.getUTCFullYear()).slice(2)}` : MONTHS[d.getUTCMonth()]}</text>}
                </g>
              );
            })}
          </svg>
        );
      }}
    </Sized>
  );
}

/** Stacked areas along a date axis; `rows` are one per month. */
export function StackedAreas({ rows, keys, height = 220, fmt = (v: number) => v.toFixed(1) }: { rows: { day: number; values: number[] }[]; keys: StackSpec[]; height?: number; fmt?: (v: number) => string }) {
  return (
    <Sized height={height}>
      {(w) => {
        const m = { l: 40, r: 8, t: 8, b: 26 };
        const d0 = rows[0]?.day ?? 0, d1 = rows[rows.length - 1]?.day ?? 1;
        const x = scaleLinear().domain([d0, Math.max(d1, d0 + 1)]).range([m.l, w - m.r]);
        const data = rows.map((r) => Object.fromEntries([['day', r.day], ...keys.map((k, i) => [k.key, Number.isFinite(r.values[i]) ? r.values[i]! : 0])])) as Record<string, number>[];
        const layers = d3stack<Record<string, number>>().keys(keys.map((k) => k.key))(data);
        const top = Math.max(1, ...layers[layers.length - 1]!.map((p) => p[1]));
        const y = scaleLinear().domain([0, top]).range([height - m.b, m.t]).nice();
        const gen = d3area<{ 0: number; 1: number; data: Record<string, number> }>().x((p) => x(p.data.day!)).y0((p) => y(p[0])).y1((p) => y(p[1])).curve(curveMonotoneX);
        return (
          <svg width={w} height={height} role="img" aria-label="Stacked areas">
            <YAxis scale={y} x={m.l} w={w - m.l - m.r} fmt={(v) => fmt(v)} />
            {layers.map((l, i) => <path key={keys[i]!.key} d={gen(l as never) ?? ''} fill={keys[i]!.color} fillOpacity={0.9}><title>{keys[i]!.label}</title></path>)}
            <DayAxis d0={d0} d1={Math.max(d1, d0 + 1)} x0={m.l} x1={w - m.r} y={height - m.b} />
          </svg>
        );
      }}
    </Sized>
  );
}

export function Legend({ items }: { items: { label: string; color: string }[] }) {
  return (
    <div className="row muted" style={{ fontSize: 12, gap: 12 }}>
      {items.map((i) => <span key={i.label}><span style={{ display: 'inline-block', width: 9, height: 9, borderRadius: 2, background: i.color, marginRight: 5 }} />{i.label}</span>)}
    </div>
  );
}

/** A grid of rectangles: rows by columns, with a function for the color of each cell. */
export function HeatGrid({ rows, cols, color, tip, xLabel, cellH = 22, colLabelEvery = 8, labelW = 104 }: {
  rows: string[];
  cols: number;
  color: (r: number, c: number) => string;
  tip: (r: number, c: number) => string;
  xLabel: (c: number) => string;
  cellH?: number;
  colLabelEvery?: number;
  labelW?: number;
}) {
  const h = rows.length * cellH + 22;
  return (
    <Sized height={h}>
      {(w) => {
        const cw = (w - labelW - 4) / Math.max(1, cols);
        return (
          <svg width={w} height={h} role="img" aria-label="Heatmap">
            {rows.map((name, r) => <text key={name} x={labelW - 8} y={r * cellH + cellH / 2} textAnchor="end" dy="0.32em" fontSize={11} fill={AX.stroke}>{name}</text>)}
            {rows.map((_, r) => Array.from({ length: cols }, (_, c) => <rect key={`${r}-${c}`} x={labelW + c * cw} y={r * cellH} width={Math.max(1, cw - 1)} height={cellH - 2} rx={2} fill={color(r, c)}><title>{tip(r, c)}</title></rect>))}
            {Array.from({ length: cols }, (_, c) => (c % colLabelEvery === 0 ? <text key={c} x={labelW + c * cw} y={rows.length * cellH + 14} fontSize={10} fill={AX.stroke} fontFamily="JetBrains Mono, monospace">{xLabel(c)}</text> : null))}
          </svg>
        );
      }}
    </Sized>
  );
}
