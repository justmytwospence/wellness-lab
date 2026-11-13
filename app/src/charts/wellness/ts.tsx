import { useMemo, useRef } from 'react';
import uPlot from 'uplot';
import { isoDay } from '@wl/views';
import { UPlot, axisStyle } from '../UPlot';
import { isNum } from './stats';

export interface TsSeries {
  label: string;
  values: ArrayLike<number | null>;
  color: string;
  /** line (default), dots, step, bars, or none (a band edge that draws nothing) */
  mode?: 'line' | 'dots' | 'step' | 'bars' | 'none';
  width?: number;
  dash?: number[];
  size?: number;
}
/** `lo` and `hi` index into `series` (0-based): the band fills between those two lines */
export interface TsBand { lo: number; hi: number; fill: string }
export interface TsProps {
  /** day numbers (one per value) */
  x: ArrayLike<number>;
  series: TsSeries[];
  bands?: TsBand[];
  height?: number;
  yLabel?: string;
  yRange?: [number | null, number | null];
  yFmt?: (v: number) => string;
  /** put y ticks at multiples of this step (clock hours: 3) */
  ySplit?: number;
  sync?: string;
  testId?: string;
}

/** `#rrggbb` plus an opacity. */
export const withAlpha = (hex: string, a: number): string => (/^#[0-9a-f]{6}$/i.test(hex) ? hex + Math.round(a * 255).toString(16).padStart(2, '0') : hex);

/** Keep the previous array while every member is identical, so uPlot only redraws on real changes. */
function useStable<T extends unknown[]>(next: T): T {
  const prev = useRef<T>(next);
  const same = prev.current.length === next.length && prev.current.every((a, i) => a === next[i]);
  if (!same) prev.current = next;
  return prev.current;
}

/** A daily time series (days on x, drawn in UTC so the date is the date in the data). */
export function TimeSeries({ x, series, bands, height = 180, yLabel, yRange, yFmt, ySplit, sync, testId }: TsProps) {
  const fmt = useRef(yFmt);
  fmt.current = yFmt;
  const key = JSON.stringify([series.map((s) => [s.label, s.color, s.mode, s.width, s.dash, s.size]), bands, yLabel, yRange, ySplit]);
  const options = useMemo<Omit<uPlot.Options, 'width' | 'height'>>(() => {
    const f = (v: number) => (fmt.current ? fmt.current(v) : Math.abs(v) >= 100 ? v.toFixed(0) : v.toFixed(1));
    return {
      padding: [8, 8, 0, 0],
      tzDate: (ts: number) => uPlot.tzDate(new Date(ts * 1e3), 'UTC'),
      scales: {
        x: { time: true },
        y: {
          range: (_u, dmin, dmax) => {
            if (!Number.isFinite(dmin) || !Number.isFinite(dmax)) return [0, 1];
            const pad = dmax > dmin ? (dmax - dmin) * 0.08 : Math.max(Math.abs(dmax) * 0.05, 0.5);
            return [yRange?.[0] ?? dmin - pad, yRange?.[1] ?? dmax + pad];
          },
        },
      },
      axes: [
        { ...axisStyle, size: 28 },
        {
          ...axisStyle,
          size: yLabel ? 62 : 50,
          label: yLabel,
          labelSize: 14,
          labelFont: axisStyle.font,
          values: (_u, v) => v.map((t) => (t === null ? '' : f(t))),
          splits: ySplit ? (_u, _i, min, max) => Array.from({ length: Math.max(0, Math.floor(max / ySplit) - Math.ceil(min / ySplit) + 1) }, (_, k) => (Math.ceil(min / ySplit) + k) * ySplit) : undefined,
        },
      ],
      legend: { live: true },
      series: [
        { label: 'Date', value: (_u, v) => (v === null ? '' : isoDay(v / 86400)) },
        ...series.map((s): uPlot.Series => {
          const base: uPlot.Series = { label: s.label, spanGaps: false, value: (_u, v) => (v === null || v === undefined ? '' : f(v)) };
          if (s.mode === 'dots') return { ...base, paths: () => null, points: { show: true, size: s.size ?? 4, fill: s.color, stroke: s.color, width: 0 } };
          if (s.mode === 'bars') return { ...base, fill: s.color, stroke: s.color, paths: uPlot.paths.bars!({ size: [0.7, 40] }), points: { show: false } };
          if (s.mode === 'none') return { ...base, stroke: 'transparent', width: 0, points: { show: false } };
          return { ...base, stroke: s.color, width: s.width ?? 1.5, dash: s.dash, paths: s.mode === 'step' ? uPlot.paths.stepped!({ align: 1 }) : undefined, points: { show: false } };
        }),
      ],
      bands: bands?.map((b) => ({ series: [b.hi + 1, b.lo + 1] as [number, number], fill: b.fill })),
    };
    // yFmt is read through a ref; everything else that shapes the plot is in the key
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  const xs = useStable([x] as unknown[]);
  const cols = useStable(series.map((s) => s.values) as unknown[]);
  const data = useMemo<uPlot.AlignedData>(
    () => [Array.from(x as ArrayLike<number>, (d) => d * 86400), ...series.map((s) => Array.from(s.values as ArrayLike<number>, (v) => (isNum(v) ? v : null)))] as uPlot.AlignedData,
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [xs, cols],
  );
  return (
    <div data-testid={testId ?? 'chart'}>
      <UPlot options={options} data={data} height={height} sync={sync} />
    </div>
  );
}
