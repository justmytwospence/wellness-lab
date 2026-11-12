import { scaleLinear } from 'd3-scale';
import { PAL, isoDay, mix } from '@wl/views';
import { AX, DayAxis, Sized } from './svg';
import { isNum } from './stats';

const ramp = (stops: string[], t: number): string => {
  const x = Math.min(1, Math.max(0, t)) * (stops.length - 1);
  const i = Math.min(stops.length - 2, Math.floor(x));
  return mix(stops[i]!, stops[i + 1]!, x - i);
};
export const VIRIDIS = ['#440154', '#3b528b', '#21908d', '#5dc863', '#fde725'];
export const viridis = (t: number): string => ramp(VIRIDIS, t);
/** Red, yellow, green for a 0..1 "how much is helping". */
export const rdylgn = (t: number): string => ramp(['#d73027', '#fee08b', '#1a9850'], t);

export const clockAxis = (v: number): string => `${String(((Math.round(v) % 24) + 24) % 24).padStart(2, '0')}:00`;

/** Bed time as hours that run past midnight without wrapping: 23:30 is -0.5, 00:30 is 0.5. */
const hhmm = (h: number): string => {
  const m = Math.round((((h % 24) + 24) % 24) * 60) % 1440;
  return `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`;
};

export const bedHour = (clockHour: number): number => ((clockHour + 6) % 24) - 6;

export interface NightRow { day: number; bed: number; wake: number; score: number; hours: number }

/** The "every night" chart: one vertical bar per night from falling asleep to waking, colored by sleep score; y wraps at midnight from -4 h to +11 h. */
export function SleepTiming({ rows, d0, d1, height = 300 }: { rows: NightRow[]; d0: number; d1: number; height?: number }) {
  return (
    <div data-testid="chart">
      <Sized height={height}>
        {(w) => {
          const m = { l: 44, r: 8, t: 8, b: 26 };
          const x = scaleLinear().domain([d0, Math.max(d1, d0 + 1)]).range([m.l, w - m.r]);
          const y = scaleLinear().domain([-4, 11]).range([m.t, height - m.b]);
          const lw = Math.max(1, Math.min(4, ((w - m.l - m.r) / Math.max(1, d1 - d0 + 1)) * 0.7));
          return (
            <svg width={w} height={height} role="img" aria-label="Every night, bed to wake">
              <g fontFamily="JetBrains Mono, monospace" fontSize={11} fill={AX.stroke}>
                {[-4, -2, 0, 2, 4, 6, 8, 10].map((t) => <g key={t} transform={`translate(0,${y(t)})`}><line x1={m.l} x2={w - m.r} stroke={AX.grid} /><text x={m.l - 6} textAnchor="end" dy="0.32em">{clockAxis(t)}</text></g>)}
              </g>
              {rows.map((r) => (
                <line key={r.day} x1={x(r.day)} x2={x(r.day)} y1={y(r.bed)} y2={y(r.wake)} stroke={isNum(r.score) ? viridis((r.score - 30) / 70) : PAL.muted} strokeWidth={lw} strokeOpacity={0.85}>
                  <title>{`${isoDay(r.day)}: asleep ${hhmm(r.bed)}, awake ${hhmm(r.wake)}, ${r.hours.toFixed(1)} h${isNum(r.score) ? `, score ${r.score.toFixed(0)}` : ''}`}</title>
                </line>
              ))}
              <DayAxis d0={d0} d1={Math.max(d1, d0 + 1)} x0={m.l} x1={w - m.r} y={height - m.b} />
            </svg>
          );
        }}
      </Sized>
      <div className="row muted" style={{ fontSize: 12 }}>
        Sleep score 30
        <span style={{ display: 'inline-block', width: 140, height: 8, borderRadius: 4, background: `linear-gradient(90deg, ${VIRIDIS.join(',')})` }} />
        100
      </div>
    </div>
  );
}
