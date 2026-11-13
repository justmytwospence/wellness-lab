import { useMemo, useRef, useState, type ReactNode } from 'react';
import uPlot from 'uplot';
import { UPlot, axisStyle } from './UPlot';

export interface Line { y: number[]; stroke: string; width?: number; dash?: number[]; points?: boolean; spanGaps?: boolean }
export interface Dots { y: number[]; color(i: number): string; r?(i: number): number }
export interface Shade { d0: number; d1: number; fill: string }
export interface TimeChartProps {
  /** local calendar days (days since 1970-01-01) */
  days: number[];
  lines: Line[];
  /** fill between two lines (indices into `lines`): `a` is the lower, `b` the upper */
  bands?: { a: number; b: number; fill: string }[];
  dots?: Dots;
  /** vertical spans drawn behind the lines */
  shade?: Shade[];
  log?: boolean;
  height?: number;
  yTitle?: string;
  sync?: string;
  /** the line above the chart that follows the cursor */
  readout?(i: number): ReactNode;
  /** what the readout line says while nothing is hovered */
  idle?: string;
  label?: string;
}

const clean = (v: number[]) => v.map((x) => (Number.isFinite(x) ? x : null)) as (number | null)[];

/** A uPlot time series over calendar days: lines, bands, per-point colored dots and shaded spans, with a hover readout. */
export function TimeChart(p: TimeChartProps) {
  const [idx, setIdx] = useState<number | null>(null);
  const live = useRef(p);
  live.current = p;
  const setter = useRef(setIdx);
  const key = JSON.stringify([p.lines.map((l) => [l.stroke, l.width, l.dash, l.points, l.spanGaps]), p.bands, !!p.dots, p.log, p.yTitle]);
  const options = useMemo<Omit<uPlot.Options, 'width' | 'height'>>(() => {
    const series: uPlot.Series[] = [
      {},
      ...p.lines.map((l): uPlot.Series => ({
        stroke: l.stroke, width: l.width ?? 1.5, dash: l.dash, spanGaps: l.spanGaps,
        points: { show: !!l.points, size: 5, fill: l.stroke, stroke: l.stroke },
      })),
    ];
    if (p.dots) series.push({ paths: () => null, points: { show: false }, stroke: 'transparent' });
    return {
      scales: {
        x: { time: true },
        // fit the data: uPlot's own log range snaps to whole decades (50 to 1,000 for nightly HRV)
        y: p.log
          ? { distr: 3, log: 10, range: (_u: uPlot, min: number, max: number): [number, number] => (min > 0 && max > 0 ? [min / 1.08, max * 1.08] : [1, 10]) }
          : { range: (_u: uPlot, min: number, max: number): [number, number] => { const pad = max > min ? (max - min) * 0.06 : Math.max(Math.abs(max) * 0.05, 0.5); return [min - pad, max + pad]; } },
      },
      series,
      bands: p.bands?.map((b) => ({ series: [b.b + 1, b.a + 1] as [number, number], fill: b.fill })),
      axes: [
        { ...axisStyle },
        {
          ...axisStyle, size: 56, label: p.yTitle, labelSize: 14, labelFont: axisStyle.font,
          ...(p.log ? {
            // round numbers inside the fitted range, at most about seven of them
            splits: (_u: uPlot, _i: number, min: number, max: number) => {
              const nice = [1, 1.5, 2, 2.5, 3, 4, 5, 6, 7, 8, 9].flatMap((m) => [1, 10, 100, 1000].map((e) => m * e)).sort((a, b) => a - b);
              let s = nice.filter((v) => v >= min && v <= max);
              while (s.length > 7) s = s.filter((_, k) => k % 2 === 0);
              return s;
            },
            values: (_u: uPlot, v: (number | null)[]) => v.map((x) => (x === null || x === undefined ? '' : x >= 10 ? x.toFixed(0) : String(+x.toFixed(1)))),
          } : {}),
        },
      ],
      legend: { show: false },
      cursor: { points: { show: false }, drag: { x: false, y: false, setScale: false } },
      tzDate: (ts: number) => uPlot.tzDate(new Date(ts * 1000), 'UTC'),
      hooks: {
        drawClear: [(u: uPlot) => {
          const shade = live.current.shade;
          if (!shade?.length) return;
          const { ctx, bbox: b } = u;
          ctx.save();
          for (const s of shade) {
            const x0 = Math.max(u.valToPos(s.d0 * 86400, 'x', true), b.left), x1 = Math.min(u.valToPos((s.d1 + 1) * 86400, 'x', true), b.left + b.width);
            if (x1 <= x0) continue;
            ctx.fillStyle = s.fill;
            ctx.fillRect(x0, b.top, x1 - x0, b.height);
          }
          ctx.restore();
        }],
        draw: [(u: uPlot) => {
          const d = live.current.dots;
          if (!d) return;
          const { ctx, bbox: b } = u;
          const xs = u.data[0] as number[];
          ctx.save();
          ctx.beginPath();
          ctx.rect(b.left, b.top, b.width, b.height);
          ctx.clip();
          for (let i = 0; i < xs.length; i++) {
            const y = d.y[i];
            if (y === undefined || !Number.isFinite(y)) continue;
            ctx.fillStyle = d.color(i);
            ctx.beginPath();
            ctx.arc(u.valToPos(xs[i]!, 'x', true), u.valToPos(y, 'y', true), (d.r?.(i) ?? 3.2) * devicePixelRatio, 0, Math.PI * 2);
            ctx.fill();
          }
          ctx.restore();
        }],
        setCursor: [(u: uPlot) => setter.current(u.cursor.idx ?? null)],
      },
    };
    // the static part of the spec is keyed; data and callbacks go through `live`
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  const data = useMemo<uPlot.AlignedData>(
    () => [p.days.map((d) => d * 86400), ...p.lines.map((l) => clean(l.y)), ...(p.dots ? [clean(p.dots.y)] : [])] as uPlot.AlignedData,
    [p.days, p.lines, p.dots],
  );
  return (
    <div data-chart="ts" role="img" aria-label={p.label}>
      {p.readout && <div className="mono muted" style={{ fontSize: 12, minHeight: 18 }}>{idx !== null ? p.readout(idx) : (p.idle ?? 'Hover for the details')}</div>}
      <UPlot options={options} data={data} height={(p.height ?? 220) - (p.readout ? 18 : 0)} sync={p.sync} />
    </div>
  );
}
