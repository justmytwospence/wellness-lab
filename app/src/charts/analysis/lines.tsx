import { useMemo, useState } from 'react';
import uPlot from 'uplot';
import { UPlot, axisStyle } from '../UPlot';
import { nn } from './inputs';

export interface LineSpec { label: string; y: (number | null)[]; color: string; dash?: number[]; width?: number; points?: boolean; faint?: boolean; format?: (v: number) => string }
export interface BandSpec { lo: (number | null)[]; hi: (number | null)[]; color: string; gradient?: boolean }
export interface SpanSpec { a: number; b: number; color: string }
export interface LineChartProps {
  x: number[];
  lines: LineSpec[];
  band?: BandSpec;
  /** x is unix seconds on a date axis; otherwise a plain number axis */
  time?: boolean;
  height?: number;
  hline?: number | null;
  vline?: number | null;
  /** coloured spans along the bottom edge of the plot (the labels gutter) */
  spans?: SpanSpec[];
  bars?: boolean;
  sync?: string;
  yLabel?: string;
  xFmt?: (v: number) => string;
  ariaLabel: string;
  testid?: string;
}

const hex = (c: string, a: number) => c + Math.round(a * 255).toString(16).padStart(2, '0');

/** uPlot lines with an optional shaded band (a gradient for confidence bands), reference lines, a spans gutter and a hover readout. Memoise the props. */
export function LineChart({ x, lines, band, time = false, height = 170, hline, vline, spans, bars, sync, yLabel, xFmt, ariaLabel, testid }: LineChartProps) {
  const [idx, setIdx] = useState<number | null>(null);
  const data = useMemo<uPlot.AlignedData>(() => {
    const cols: (number | null)[][] = [x, ...lines.map((l) => nn(l.y))];
    if (band) cols.push(nn(band.lo), nn(band.hi));
    return cols as unknown as uPlot.AlignedData;
  }, [x, lines, band]);
  const options = useMemo<Omit<uPlot.Options, 'width' | 'height'>>(() => {
    const series: uPlot.Series[] = [{}];
    lines.forEach((l, i) => {
      series.push({
        label: l.label,
        stroke: l.faint ? hex(l.color, 0.4) : l.color,
        width: l.width ?? (l.faint ? 1 : 1.8),
        dash: l.dash,
        spanGaps: false,
        points: { show: !!l.points, size: 5, fill: l.color, stroke: l.color },
        ...(bars && i === 0 ? { paths: uPlot.paths.bars!({ size: [0.7, 60] }), fill: hex(l.color, 0.55), width: 0 } : {}),
      });
    });
    const bands: uPlot.Band[] = [];
    if (band) {
      const hiIdx = series.length + 1;
      series.push({ label: 'lo', stroke: 'transparent', width: 0, points: { show: false } }, { label: 'hi', stroke: 'transparent', width: 0, points: { show: false } });
      bands.push({
        series: [hiIdx, hiIdx - 1],
        fill: band.gradient
          ? (u: uPlot) => {
              const g = u.ctx.createLinearGradient(0, u.bbox.top, 0, u.bbox.top + u.bbox.height);
              g.addColorStop(0, hex(band.color, 0.5));
              g.addColorStop(0.5, hex(band.color, 0.18));
              g.addColorStop(1, hex(band.color, 0.5));
              return g;
            }
          : hex(band.color, 0.22),
      });
    }
    const guide = (u: uPlot) => {
      const { ctx, bbox } = u;
      ctx.save();
      ctx.lineWidth = 1;
      if (hline !== null && hline !== undefined) {
        const y = Math.round(u.valToPos(hline, 'y', true));
        if (y >= bbox.top && y <= bbox.top + bbox.height) {
          ctx.strokeStyle = '#8892b8aa';
          ctx.beginPath(); ctx.moveTo(bbox.left, y); ctx.lineTo(bbox.left + bbox.width, y); ctx.stroke();
        }
      }
      if (vline !== null && vline !== undefined) {
        const px = Math.round(u.valToPos(vline, 'x', true));
        if (px >= bbox.left && px <= bbox.left + bbox.width) {
          ctx.strokeStyle = '#8892b8aa'; ctx.setLineDash([4, 4]);
          ctx.beginPath(); ctx.moveTo(px, bbox.top); ctx.lineTo(px, bbox.top + bbox.height); ctx.stroke();
        }
      }
      for (const s of spans ?? []) {
        const a = u.valToPos(s.a, 'x', true), b = u.valToPos(s.b, 'x', true);
        ctx.fillStyle = s.color;
        ctx.fillRect(Math.max(bbox.left, a), bbox.top + bbox.height - 6, Math.max(2, Math.min(bbox.left + bbox.width, b) - Math.max(bbox.left, a)), 6);
      }
      ctx.restore();
    };
    return {
      series,
      bands,
      legend: { show: false },
      scales: {
        x: { time },
        y: bars
          ? { range: (_u: uPlot, _min: number, max: number): [number, number] => [0, Math.max(1, max) * 1.15] }
          : {
              // fit the lines, not a band that may be wide where a filter has not settled yet
              range: (_u: uPlot, min: number, max: number): [number, number] => {
                const v = lines.flatMap((l) => l.y.filter((x): x is number => x !== null && Number.isFinite(x)));
                if (hline !== null && hline !== undefined) v.push(hline);
                let lo = v.length ? Math.min(...v) : min, hi = v.length ? Math.max(...v) : max;
                if (!(hi > lo)) { lo -= 0.5; hi += 0.5; }
                const pad = (hi - lo) * 0.08;
                return [lo - pad, hi + pad];
              },
            },
      },
      ...(time ? { tzDate: (ts: number) => uPlot.tzDate(new Date(ts * 1e3), 'UTC') } : {}),
      axes: [
        { ...axisStyle, size: 30, ...(time ? {} : { incrs: [1, 2, 5, 10, 20, 50, 100, 200, 500], ...(xFmt ? { values: (_u: uPlot, v: number[]) => v.map(xFmt) } : {}) }) },
        { ...axisStyle, size: 54, label: yLabel, labelSize: yLabel ? 16 : 0, labelFont: axisStyle.font },
      ],
      hooks: { draw: [guide], setCursor: [(u: uPlot) => setIdx(u.cursor.idx ?? null)] },
    };
  }, [lines, band, time, hline, vline, spans, bars, yLabel, xFmt]);
  const read = idx === null ? null : lines.map((l) => `${l.label} ${l.y[idx] === null || l.y[idx] === undefined || !Number.isFinite(l.y[idx]!) ? '\u2013' : l.format ? l.format(l.y[idx]!) : (+l.y[idx]!.toPrecision(4)).toString()}`).join('   ');
  const at = idx === null ? '' : xFmt ? xFmt(x[idx]!) : String(x[idx]);
  return (
    <div role="img" aria-label={ariaLabel} data-testid={testid ?? 'chart'} data-points={x.length}>
      <UPlot options={options} data={data} height={Math.max(40, height - 16)} sync={sync} />
      <div className="mono muted" style={{ fontSize: 11, minHeight: 16 }}>{read !== null && `${at}   ${read}`}</div>
    </div>
  );
}
