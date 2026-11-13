import { useEffect, useMemo, useRef, useState } from 'react';
import uPlot from 'uplot';
import { STAGES, kindInfo, type Band, type Chan, type NightChannels } from '@wl/schema';
import { PAL, STRESS_BANDS, clock, isoDay, prettyDay, signed, parseDay } from '@wl/views';
import { useApp } from '../lib/app';
import { useJob } from '../lib/useJob';
import { link, navigate, useLoc, useParam } from '../lib/router';
import { UPlot, axisStyle } from '../charts/UPlot';
import { Empty, Grade } from '../charts/small';
import { MethodButton } from '../layout/MethodPanel';
import { LabelDialog } from '../layout/LabelDialog';
import { View } from '../ui/View';
import { Panel } from '../ui/Panel';
import { Fill } from '../ui/Fill';

const nz = (a: (number | null)[]): number[] => a.map((v) => (v === null ? NaN : v));

/** Seconds on a "local clock as UTC" axis. */
const sec = (ms: number, off: number) => (ms + off * 60_000) / 1000;

interface LaneCfg { key: string; name: string; unit: string; color: string; kind: 'line' | 'points' | 'bars' | 'steps'; band?: string; min?: number; max?: number }
const LANES: LaneCfg[] = [
  { key: 'hr', name: 'Heart rate', unit: 'bpm', color: PAL.hr, kind: 'line', band: 'hr' },
  { key: 'hrv5', name: 'HRV (5 min RMSSD)', unit: 'ms', color: PAL.hrv, kind: 'points', band: 'hrv5' },
  { key: 'stress', name: 'Stress', unit: '', color: PAL.violet, kind: 'bars', band: 'stress', min: 0, max: 100 },
  { key: 'resp', name: 'Breathing', unit: '/min', color: PAL.base, kind: 'line', band: 'resp' },
  { key: 'spo2', name: 'SpO2', unit: '%', color: '#5ad1ff', kind: 'points' },
  { key: 'body_battery', name: 'Body Battery', unit: '', color: '#9ad17a', kind: 'line', min: 0, max: 100 },
  { key: 'steps', name: 'Steps', unit: '', color: PAL.muted, kind: 'bars' },
  { key: 'altitude', name: 'Altitude', unit: 'm', color: PAL.violet, kind: 'line' },
];

function Lane({ cfg, night, other, xr, onBrush, labels, showX, height }: { height: number; showX: boolean; cfg: LaneCfg; night: NightChannels; other?: { night: NightChannels; shift: number }; xr: [number, number]; onBrush(r: [number, number] | null): void; labels: { a: number; b: number; color: string }[] }) {
  const ch: Chan | undefined = night.series[cfg.key] ?? (cfg.key === 'steps' ? night.series.motion : undefined);
  const off = night.utc_offset_min;
  const band = cfg.band ? (night.bands[cfg.band] as Band | undefined) : undefined;
  const { data, options } = useMemo(() => {
    if (!ch) return { data: null, options: null };
    const t = ch.t.map((x) => sec(x, off));
    const v = nz(ch.v as (number | null)[]);
    const bandT = band ? band.minute.map((m) => sec(night.onset + m * 60_000, off)) : [];
    const oc = other ? other.night.series[cfg.key] : undefined;
    const ot = oc ? oc.t.map((x) => sec(x + other!.shift, off)) : [];
    const ov = oc ? nz(oc.v as (number | null)[]) : [];
    const xs = [...new Set([...t, ...bandT, ...ot])].sort((a, b) => a - b);
    const idx = new Map(xs.map((x, i) => [x, i]));
    const place = (src: number[], vals: number[]) => {
      const o: (number | null)[] = new Array(xs.length).fill(null);
      src.forEach((s2, i) => { o[idx.get(s2)!] = Number.isFinite(vals[i]!) ? vals[i]! : null; });
      return o;
    };
    const series: uPlot.Series[] = [{}];
    const cols: (number | null)[][] = [];
    if (band) {
      cols.push(place(bandT, nz(band.p10 as (number | null)[])), place(bandT, nz(band.p90 as (number | null)[])));
      series.push({ label: 'p10', stroke: 'transparent', width: 0, points: { show: false } }, { label: 'p90', stroke: 'transparent', width: 0, points: { show: false } });
    }
    cols.push(place(t, v));
    series.push(cfg.kind === 'points' ? { label: cfg.name, stroke: cfg.color, width: 1.5, points: { show: true, size: 4, fill: cfg.color } } : cfg.kind === 'bars' ? { label: cfg.name, fill: cfg.color + 'aa', stroke: cfg.color, paths: uPlot.paths.bars!({ size: [0.9, 6] }), points: { show: false } } : { label: cfg.name, stroke: cfg.color, width: 1.4, points: { show: false } });
    if (oc) {
      cols.push(place(ot, ov));
      series.push({ label: 'other night', stroke: PAL.muted, width: 1, dash: [4, 3], points: { show: false } });
    }
    const spanGaps = cfg.kind === 'line';
    series.forEach((se, k) => { if (k > 0) se.spanGaps = spanGaps || k <= (band ? 2 : 0); });
    const opts: Omit<uPlot.Options, 'width' | 'height'> = {
      series,
      legend: { show: false },
      scales: { x: { time: true, min: xr[0], max: xr[1] }, y: { range: cfg.min !== undefined ? [cfg.min, cfg.max!] : undefined } },
      axes: [{ ...axisStyle, show: showX, size: showX ? 30 : 0 }, { ...axisStyle, size: 44 }],
      bands: band ? [{ series: [2, 1], fill: PAL.base + '22' }] : [],
      cursor: { drag: { x: true, y: false, setScale: false }, points: { size: 6 } },
      padding: [6, 8, 0, 0],
      hooks: {
        setSelect: [(u: uPlot) => { if (u.select.width > 4) { onBrush([u.posToVal(u.select.left, 'x'), u.posToVal(u.select.left + u.select.width, 'x')]); u.setSelect({ left: 0, width: 0, top: 0, height: 0 }, false); } }],
        drawAxes: [(u: uPlot) => { const c = u.ctx; for (const l of labels) { const x0 = u.valToPos(l.a, 'x', true), x1 = u.valToPos(l.b, 'x', true); c.fillStyle = l.color + '26'; c.fillRect(x0, u.bbox.top, Math.max(2, x1 - x0), u.bbox.height); } }],
      },
    };
    return { data: [xs, ...cols] as uPlot.AlignedData, options: opts };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ch, band, night, xr[0], xr[1], other, labels, showX]);
  if (!ch || !data || !options) return null;
  return (
    <div className="lane" data-testid={`lane-${cfg.key}`}>
      <span className="name">{cfg.name} {cfg.unit && <span className="muted">({cfg.unit})</span>}</span>
      <UPlot options={options} data={data} height={height} sync="night" />
    </div>
  );
}

function Hypnogram({ night, xr, onBrush }: { night: NightChannels; xr: [number, number]; onBrush(r: [number, number] | null): void }) {
  const off = night.utc_offset_min;
  const st = night.stages;
  const s = (st.num.start ?? []) as number[], e = (st.num.end ?? []) as number[], k = (st.num.stage ?? []) as number[];
  const t0 = sec(night.onset, off) - 3600, t1 = sec(night.wake, off) + 3600;
  const W = 1000;
  const x = (t: number) => ((t - t0) / (t1 - t0)) * W;
  const row = (c: number) => ({ 3: 0, 2: 1, 1: 2, 0: 3, 4: 4 } as Record<number, number>)[c] ?? 4;
  const drag = useRef<number | null>(null);
  const [sel, setSel] = useState<[number, number] | null>(null);
  const px = (ev: React.MouseEvent) => { const r = (ev.currentTarget as SVGElement).getBoundingClientRect(); return t0 + ((ev.clientX - r.left) / r.width) * (t1 - t0); };
  return (
    <div className="lane" data-testid="lane-hypnogram">
      <span className="name">Sleep stages</span>
      <svg viewBox={`0 0 ${W} 40`} preserveAspectRatio="none" width="100%" height={40} style={{ cursor: 'ew-resize', display: 'block', background: PAL.panel2, borderRadius: 4 }}
        onMouseDown={(ev) => { drag.current = px(ev); }}
        onMouseMove={(ev) => { if (drag.current !== null) setSel([drag.current, px(ev)]); }}
        onMouseUp={(ev) => { if (drag.current !== null) { const a = drag.current, b = px(ev); drag.current = null; setSel(null); if (Math.abs(b - a) > (t1 - t0) * 0.01) onBrush([Math.min(a, b), Math.max(a, b)]); } }}
        onDoubleClick={() => onBrush(null)}>
        {s.map((a, i) => <rect key={i} x={x(sec(a, off))} width={Math.max(1, x(sec(e[i]!, off)) - x(sec(a, off)))} y={row(k[i]!) * 8} height={7} fill={STAGES.find((z) => z.code === k[i])?.color ?? PAL.muted} />)}
        <rect x={x(xr[0])} width={Math.max(2, x(xr[1]) - x(xr[0]))} y={0} height={40} fill="none" stroke={PAL.hrv} strokeOpacity={0.6} />
        {sel && <rect x={x(Math.min(...sel))} width={Math.abs(x(sel[1]) - x(sel[0]))} y={0} height={40} fill={PAL.base} opacity={0.3} />}
      </svg>
    </div>
  );
}

const GROUPS: { title: string; grade?: 'A' | 'B' | 'C' | 'D'; rows: [string, string, (v: number) => string][] }[] = [
  { title: 'Timing', grade: 'A', rows: [['onset_clock', 'Fell asleep', (v) => clock(v)], ['wake_clock', 'Woke', (v) => clock(v)], ['mid_clock', 'Midsleep', (v) => clock(v)], ['tib_h', 'Time in bed (h)', (v) => v.toFixed(2)], ['tst_h', 'Sleep (h)', (v) => v.toFixed(2)]] },
  { title: 'Heart', grade: 'B', rows: [['hr_min30', 'Lowest 30 min (bpm)', (v) => v.toFixed(0)], ['hr_mean', 'Mean (bpm)', (v) => v.toFixed(1)], ['hr_nadir', 'Nadir (bpm)', (v) => v.toFixed(0)], ['dip_pct', 'Dip (%)', (v) => v.toFixed(1)], ['hr_early_late', 'Early minus late (bpm)', (v) => signed(v, 1)]] },
  { title: 'HRV', grade: 'B', rows: [['rmssd_mean', 'RMSSD (ms)', (v) => v.toFixed(0)], ['rmssd_4h', 'First 4 h (ms)', (v) => v.toFixed(0)], ['rmssd_last2h', 'Last 2 h (ms)', (v) => v.toFixed(0)], ['rmssd_norm', 'Normalised', (v) => v.toFixed(2)], ['garmin_hrv', 'Garmin night (ms)', (v) => v.toFixed(0)]] },
  { title: 'Stress and Body Battery', grade: 'C', rows: [['sleep_stress', 'Sleep stress', (v) => v.toFixed(0)], ['bb_wake', 'Body Battery at wake', (v) => v.toFixed(0)], ['bb_change', 'Charged', (v) => signed(v, 0)]] },
  { title: 'Breathing and SpO2', grade: 'C', rows: [['resp_mean', 'Breathing (/min)', (v) => v.toFixed(1)], ['resp_sd', 'Spread', (v) => v.toFixed(2)], ['spo2_mean', 'SpO2 mean (%)', (v) => v.toFixed(1)], ['odi3', 'ODI3 (/h)', (v) => v.toFixed(1)]] },
  { title: 'Stages', grade: 'D', rows: [['deep_min', 'Deep (min)', (v) => v.toFixed(0)], ['rem_min', 'REM (min)', (v) => v.toFixed(0)], ['light_min', 'Light (min)', (v) => v.toFixed(0)], ['awake_min', 'Awake (min)', (v) => v.toFixed(0)], ['transitions_h', 'Transitions per hour', (v) => v.toFixed(1)]] },
  { title: 'Context', rows: [['load_prev', 'Training load, day before', (v) => v.toFixed(0)], ['last_act_end_h', 'Hours since last activity', (v) => v.toFixed(1)], ['tz_change', 'Time zone change (min)', (v) => v.toFixed(0)], ['sleep_score', 'Garmin sleep score', (v) => v.toFixed(0)]] },
];

export default function Night() {
  const app = useApp();
  const loc = useLoc();
  const day = Number(loc.path.split('/')[2]) || app.lastDay;
  const { data: night, error, loading } = useJob({ kind: 'night', day });
  const [cmp, setCmp] = useParam('cmp', 'none');
  const other = cmp === 'prev' ? day - 1 : cmp === 'week' ? day - 7 : null;
  const { data: oth } = useJob(other !== null ? { kind: 'night', day: other } : null);
  const [zoom, setZoom] = useState<[number, number] | null>(null);
  const [dialog, setDialog] = useState(false);
  const nightsTbl = app.nights;
  const nz2 = nightsTbl?.num.day as number[] | undefined;
  const days = nz2 ?? [];
  const i = days.indexOf(day);
  const step = (d: number) => { const j = Math.max(0, Math.min(days.length - 1, (i < 0 ? days.length - 1 : i) + d)); if (days[j] !== undefined) navigate(`/night/${days[j]}`); };
  useEffect(() => setZoom(null), [day]);
  useEffect(() => {
    const on = (e: KeyboardEvent) => {
      if (/^(INPUT|TEXTAREA|SELECT)$/.test((e.target as HTMLElement).tagName)) return;
      if (e.key === '[') step(-1);
      else if (e.key === ']') step(1);
      else if (e.key === 'f') setZoom(null);
      else if (e.key === 'l') setDialog(true);
    };
    window.addEventListener('keydown', on);
    return () => window.removeEventListener('keydown', on);
  });
  const nightD = night as unknown as NightChannels | undefined;
  const full: [number, number] | null = nightD ? [sec(nightD.onset, nightD.utc_offset_min) - 3600, sec(nightD.wake, nightD.utc_offset_min) + 3600] : null;
  const xr = zoom ?? full ?? [0, 1];
  const labels = useMemo(() => app.state.labels.filter((l) => !l.deleted_at && nightD && l.start_day <= day && l.end_day >= day).map((l) => ({ a: sec(nightD!.onset, nightD!.utc_offset_min) - 3600, b: sec(nightD!.wake, nightD!.utc_offset_min) + 3600, color: kindInfo(l.kind).color })), [app.state.labels, nightD, day]);
  const props = app.state.proposals.filter((p) => p.status === 'open' && p.start_day <= day && p.end_day >= day);
  const otherObj = useMemo(() => (nightD && oth ? { night: oth as unknown as NightChannels, shift: nightD.onset - (oth as unknown as NightChannels).onset } : undefined), [nightD, oth]);
  const laneCfgs = nightD ? LANES.filter((c) => nightD.series[c.key] || (c.key === 'steps' && nightD.series.motion)) : [];
  return (
    <View
      testid="night-page"
      title={prettyDay(day)}
      tools={(
        <>
          <button onClick={() => step(-1)} aria-label="Previous night">[</button>
          <button onClick={() => step(1)} aria-label="Next night">]</button>
          <input type="date" value={isoDay(day)} onChange={(e) => e.target.value && navigate(`/night/${parseDay(e.target.value)}`)} aria-label="Night" />
          <label>Compare with <select value={cmp} onChange={(e) => setCmp(e.target.value)}><option value="none">nothing</option><option value="prev">previous night</option><option value="week">same weekday last week</option></select></label>
          <button onClick={() => setDialog(true)}>Label (l)</button>
          <MethodButton id="night.hr" job={{ kind: 'night', day }} />
        </>
      )}
    >
      {error ? <Empty>No sleep recorded for this night. <a href="/nights" onClick={link('/nights')}>Back to nights</a></Empty> : !nightD ? <Empty>{loading ? 'Loading...' : 'No data'}</Empty> : (
        <div className="vgrid" style={{ gridTemplateColumns: 'minmax(0, 1fr) 340px' }}>
          <Panel>
            <Hypnogram night={nightD} xr={xr} onBrush={setZoom} />
            <div style={{ display: 'flex', flexDirection: 'column', flex: '1 1 0', minHeight: 0, gap: 2, marginTop: 4 }}>
              {laneCfgs.map((c, k) => {
                const showX = k === laneCfgs.length - 1;
                return (
                  <Fill key={c.key} style={{ flex: showX ? '1.35 1 0' : '1 1 0', minHeight: 44 }}>
                    {({ h }) => <Lane showX={showX} height={h} cfg={c} night={nightD} xr={xr} onBrush={setZoom} labels={labels} other={c.key === 'hr' || c.key === 'hrv5' ? otherObj : undefined} />}
                  </Fill>
                );
              })}
            </div>
            <div className="row muted" style={{ fontSize: 12, marginTop: 6 }}>
              <span>Drag on a lane or the stages to zoom, f to fit, double-click the stages to reset.</span>
              {STAGES.slice(0, 4).map((s) => <span key={s.code}><span style={{ color: s.color }}>{'\u25a0'}</span> {s.name}</span>)}
              {STRESS_BANDS.length > 0 && <span>Shaded: your usual night (10th to 90th percentile of the last 30)</span>}
            </div>
            {labels.length > 0 && <p style={{ marginTop: 8, marginBottom: 0 }}>{app.state.labels.filter((l) => !l.deleted_at && l.start_day <= day && l.end_day >= day).map((l) => <span key={l.id} className="chip" style={{ background: kindInfo(l.kind).color, marginRight: 6 }}>{kindInfo(l.kind).name}</span>)}</p>}
            {props.length > 0 && <p className="muted" style={{ marginBottom: 0 }}>Suggested: {props.map((p) => kindInfo(p.kind).name).join(', ')} (see <a href="/inbox" onClick={link('/inbox')}>Inbox</a>)</p>}
          </Panel>
          <Panel scroll>
            {GROUPS.map((g) => (
              <div key={g.title} style={{ marginBottom: 10 }}>
                <h3>{g.title}{g.grade && <Grade g={g.grade} />}</h3>
                <table className="t"><tbody>{g.rows.map(([k, label, f]) => { const v = nightD.row[k]; if (v === null || v === undefined || !Number.isFinite(v)) return null; return <tr key={k}><td>{label}</td><td className="n">{f(v)}</td></tr>; })}</tbody></table>
              </div>
            ))}
          </Panel>
        </div>
      )}
      {dialog && <LabelDialog initial={{ start_day: day, end_day: day }} onClose={() => setDialog(false)} />}
    </View>
  );
}
