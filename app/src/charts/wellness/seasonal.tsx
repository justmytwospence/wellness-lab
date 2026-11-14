import { useMemo, useState, type ReactNode } from 'react';
import { scaleLinear } from 'd3-scale';
import { line as d3line } from 'd3-shape';
import type { Job, Table } from '@wl/schema';
import { PAL } from '@wl/views';
import { useJob } from '../../lib/useJob';
import { useParam } from '../../lib/router';
import { Segmented } from '../../ui/Segmented';
import { AX, Sized, YAxis } from './svg';
import { quantile } from './stats';
import { byYear, niceScale, yearColor, type YearLine } from './seasonalMath';

export { byYear, niceScale, yearColor, type YearLine };

export type SeasonalMode = 'timeline' | 'year' | 'radial';
export interface SeasonalSpec { source: 'days' | 'nights'; column: string; label: string; unit: string; digits: number }
export interface SeasonalSlot {
  /** the Timeline / By year / Radial switch, or null while the range holds fewer than two qualifying years */
  toggle: ReactNode | null;
  /** the chart for the chosen view at a pixel height; null under the timeline */
  seasonal: ((h: number) => ReactNode) | null;
  sub: string | null;
  method: string | null;
  job: Job | null;
}
const MODES: [SeasonalMode, string][] = [['timeline', 'Timeline'], ['year', 'By year'], ['radial', 'Radial']];
const SUB = 'Deviation from your own centred one-year median, 15-day mean; one line per year, current year thicker. Click a year to pick it out.';
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
/** First day of each month on the 366-day calendar (the same in every year), and the day after the last. */
const MONTH_START = [1, 32, 61, 92, 122, 153, 183, 214, 245, 275, 306, 336, 367];
const LEGEND_H = 28;
export function SeasonalView({ spec, from, to, children }: { spec: SeasonalSpec; from: number; to: number; children: (s: SeasonalSlot) => ReactNode }) {
  const [sv, setSv] = useParam('sv', 'timeline');
  const mode: SeasonalMode = sv === 'year' || sv === 'radial' ? sv : 'timeline';
  const job: Job | null = to - from + 1 >= 360 ? { kind: 'seasonal', from, to, source: spec.source, column: spec.column } : null;
  const { data } = useJob(job);
  const lines = useMemo(() => (data ? byYear(data as Table) : []), [data]);
  const available = job !== null && lines.length >= 2;
  const toggle = available ? <Segmented role="radiogroup" label="Seasonal view" value={mode} options={MODES} onChange={(v) => setSv(v)} /> : null;
  if (!available || mode === 'timeline') return <>{children({ toggle, seasonal: null, sub: null, method: null, job: null })}</>;
  const p = { lines, unit: spec.unit, digits: spec.digits, label: spec.label };
  const seasonal = (height: number) => (mode === 'year' ? <SeasonalYears {...p} height={height} /> : <SeasonalRadial {...p} height={height} />);
  return <>{children({ toggle, seasonal, sub: SUB, method: 'display.seasonal', job })}</>;
}

interface ChartProps { lines: YearLine[]; unit: string; digits: number; height: number; label: string }

/** The year buttons: pick one year out (the others fade), click again to clear. */
function YearLegend({ lines, picked, setPicked }: { lines: YearLine[]; picked: number | null; setPicked: (y: number | null) => void }) {
  return (
    <div className="row muted" style={{ fontSize: 12, gap: 12, height: LEGEND_H, alignItems: 'center', flexWrap: 'wrap' }}>
      {lines.map((l, i) => (
        <button key={l.year} aria-pressed={picked === l.year} onClick={() => setPicked(picked === l.year ? null : l.year)}
          style={{ background: 'none', border: 'none', padding: 0, cursor: 'pointer', font: 'inherit', color: picked === l.year ? PAL.text : 'inherit', fontWeight: picked === l.year ? 700 : 400 }}>
          <span style={{ display: 'inline-block', width: 9, height: 9, borderRadius: 2, background: yearColor(i, lines.length), marginRight: 5 }} />{l.year}
        </button>
      ))}
    </div>
  );
}

const paint = (lines: YearLine[], picked: number | null) =>
  lines.map((l, i) => ({
    year: l.year, dev: l.dev, color: yearColor(i, lines.length),
    width: i === lines.length - 1 ? 2.5 : 1.2,
    opacity: picked === null || picked === l.year ? 1 : 0.15,
  }));

const finiteDevs = (lines: YearLine[]) => lines.flatMap((l) => l.dev.slice(1).filter(Number.isFinite));

export function SeasonalYears({ lines, unit, digits, height, label }: ChartProps) {
  const [picked, setPicked] = useState<number | null>(null);
  const h = Math.max(60, height - LEGEND_H);
  const all = finiteDevs(lines);
  return (
    <div style={{ height }}>
      <Sized height={h}>
        {(w) => {
          const m = { t: 8, r: 8, b: 22, l: 52 };
          const lo = Math.min(...all, 0), hi = Math.max(...all, 0);
          const pad = (hi - lo || 1) * 0.05;
          const x = scaleLinear().domain([1, 367]).range([m.l, w - m.r]);
          const y = scaleLinear().domain([lo - pad, hi + pad]).range([h - m.b, m.t]).nice();
          const gen = d3line<number>().defined((v) => Number.isFinite(v)).x((_, i) => x(i + 1)).y((v) => y(v));
          return (
            <svg role="img" aria-label={`${label}, by year`} data-chart data-testid="chart" width={w} height={h}>
              {MONTH_START.slice(0, 12).map((s, i) => (
                <g key={s}>
                  <line x1={x(s)} x2={x(s)} y1={m.t} y2={h - m.b} stroke={AX.grid} />
                  <text x={x((s + MONTH_START[i + 1]!) / 2)} y={h - 6} textAnchor="middle" fontSize={11} fontFamily="JetBrains Mono, monospace" fill={AX.stroke}>{MONTHS[i]}</text>
                </g>
              ))}
              <YAxis scale={y} x={m.l} w={w - m.l - m.r} fmt={(v) => v.toFixed(digits)} />
              <text transform={`translate(11,${(m.t + h - m.b) / 2}) rotate(-90)`} textAnchor="middle" fontSize={11} fontFamily="JetBrains Mono, monospace" fill={AX.stroke}>{unit} vs baseline</text>
              <line x1={m.l} x2={w - m.r} y1={y(0)} y2={y(0)} stroke={PAL.muted} strokeDasharray="3 3" />
              {paint(lines, picked).map((l) => (
                <path key={l.year} d={gen(l.dev.slice(1)) ?? ''} fill="none" stroke={l.color} strokeWidth={l.width} strokeOpacity={l.opacity} strokeLinejoin="round"><title>{l.year}</title></path>
              ))}
            </svg>
          );
        }}
      </Sized>
      <YearLegend lines={lines} picked={picked} setPicked={setPicked} />
    </div>
  );
}

export function SeasonalRadial({ lines, unit, digits, height, label }: ChartProps) {
  const [picked, setPicked] = useState<number | null>(null);
  const h = Math.max(60, height - LEGEND_H);
  const all = finiteDevs(lines);
  const k = niceScale(quantile(all.map(Math.abs), 0.98));
  const kText = digits > 0 || k >= 1 ? String(k) : k.toFixed(1);
  return (
    <div style={{ height }}>
      <Sized height={h}>
        {(w) => {
          const size = Math.min(w, h);
          const cx = w / 2, cy = h / 2, R = Math.max(10, size / 2 - 18);
          const rr = (dev: number) => Math.min(1.8, Math.max(0.2, 1 + 0.5 * dev / k));
          const rho = (r: number) => (r / 2) * R;
          const ang = (doy: number) => (2 * Math.PI * (doy - 1)) / 366;
          const px = (doy: number, r: number) => cx + rho(r) * Math.sin(ang(doy));
          const py = (doy: number, r: number) => cy - rho(r) * Math.cos(ang(doy));
          const path = (dev: number[]) => {
            const gen = d3line<number>().defined((v) => Number.isFinite(v)).x((v, i) => px(i + 1, rr(v))).y((v, i) => py(i + 1, rr(v)));
            const d = gen(dev.slice(1)) ?? '';
            // closed only where December runs into January; never a segment across the boundary otherwise
            return d && Number.isFinite(dev[366]) && Number.isFinite(dev[1]) ? `${d}L${px(1, rr(dev[1]!))},${py(1, rr(dev[1]!))}` : d;
          };
          const ring = (r: number, text: string) => (
            <g key={r}>
              <circle cx={cx} cy={cy} r={rho(r)} fill="none" stroke={AX.grid} strokeDasharray="2 3" />
              <text x={cx + 3} y={cy - rho(r) - 2} fontSize={10} fontFamily="JetBrains Mono, monospace" fill={AX.stroke}>{text}</text>
            </g>
          );
          return (
            <svg role="img" aria-label={`${label}, radial`} data-chart data-testid="chart" width={w} height={h}>
              {MONTH_START.slice(0, 12).map((s) => <line key={s} x1={cx} y1={cy} x2={px(s, 2)} y2={py(s, 2)} stroke={AX.grid} strokeOpacity={0.5} />)}
              {ring(0.5, `-${kText}`)}
              {ring(1, '0')}
              {ring(1.5, `+${kText} ${unit}`)}
              {MONTH_START.slice(0, 12).map((s, i) => {
                const mid = (s + MONTH_START[i + 1]!) / 2;
                return <text key={s} x={px(mid, 1.9)} y={py(mid, 1.9)} textAnchor="middle" dy="0.32em" fontSize={11} fontFamily="JetBrains Mono, monospace" fill={AX.stroke}>{MONTHS[i]}</text>;
              })}
              {paint(lines, picked).map((l) => (
                <path key={l.year} d={path(l.dev)} fill="none" stroke={l.color} strokeWidth={l.width} strokeOpacity={l.opacity} strokeLinejoin="round"><title>{l.year}</title></path>
              ))}
            </svg>
          );
        }}
      </Sized>
      <YearLegend lines={lines} picked={picked} setPicked={setPicked} />
    </div>
  );
}
