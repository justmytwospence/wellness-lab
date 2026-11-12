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
      scales: { x: { time: true }, y: p.log ? { distr: 3, log: 10 } : {} },
      series,
      bands: p.bands?.map((b) => ({ series: [b.b + 1, b.a + 1] as [number, number], fill: b.fill })),
      axes: [{ ...axisStyle }, { ...axisStyle, size: 56, label: p.yTitle, labelSize: 14, labelFont: axisStyle.font }],
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
      <UPlot options={options} data={data} height={p.height ?? 220} sync={p.sync} />
    </div>
  );
}
