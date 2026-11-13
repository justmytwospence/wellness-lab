import { useMemo, useState } from 'react';
import { adviceOf, kindInfo, type Table } from '@wl/schema';
import { PAL, clock, duration, num, sequential, signed, why, confidence, whatIf, isoDay, weekday } from '@wl/views';
import { useApp } from '../lib/app';
import { useJob } from '../lib/useJob';
import { col, finite } from '../lib/util';
import { link, navigate, useParam } from '../lib/router';
import { BandBar, Empty, Spark } from '../charts/small';
import { MethodButton } from '../layout/MethodPanel';
import { View, ViewEmpty } from '../ui/View';
import { Panel } from '../ui/Panel';
import { Fill } from '../ui/Fill';

type Metric = { id: string; label: string; unit: string; fmt: (v: number) => string; method: string };
const METRICS: Metric[] = [
  { id: 'hr_min30', label: 'Lowest heart rate', unit: 'bpm', fmt: (v) => v.toFixed(0), method: 'night.hr' },
  { id: 'ln_rmssd', label: 'Overnight HRV', unit: 'ms', fmt: (v) => Math.exp(v).toFixed(0), method: 'night.hrv' },
  { id: 'resp_mean', label: 'Breathing rate', unit: '/min', fmt: (v) => v.toFixed(1), method: 'night.resp' },
  { id: 'sleep_stress', label: 'Sleep stress', unit: '', fmt: (v) => v.toFixed(0), method: 'night.stress' },
  { id: 'skin_temp', label: 'Skin temperature', unit: '\u00b0C', fmt: (v) => v.toFixed(2), method: 'night.hr' },
  { id: 'tst_h', label: 'Sleep time', unit: '', fmt: duration, method: 'night.stages' },
  { id: 'mid_clock', label: 'Midsleep', unit: '', fmt: (v) => clock(v), method: 'night.timing' },
  { id: 'sleep_score', label: 'Garmin sleep score', unit: '', fmt: (v) => v.toFixed(0), method: 'garmin.teardown' },
  { id: 'bb_change', label: 'Body Battery charge', unit: '', fmt: (v) => signed(v, 0), method: 'garmin.teardown' },
];

function MetricRow({ m, day }: { m: Metric; day: number }) {
  const { data } = useJob({ kind: 'baseline', metric: m.id });
  if (!data) return null;
  const d = col(data as unknown as Table, 'day');
  const i = d.lastIndexOf(day);
  if (i < 0) return null;
  const c = (n: string) => col(data as unknown as Table, n)[i]!;
  const x = c('x');
  if (!finite(x)) return null;
  return (
    <tr>
      <td>{m.label} <MethodButton id={m.method} job={{ kind: 'baseline', metric: m.id }} /></td>
      <td className="n">{m.fmt(x)} <span className="muted">{m.unit}</span></td>
      <td className="n muted">{finite(c('center')) ? m.fmt(c('center')) : ''}</td>
      <td className="n">{finite(c('z')) ? signed(c('z'), 1) : ''}</td>
      <td><BandBar v={x} lo={c('lo')} hi={c('hi')} /></td>
    </tr>
  );
}

function Heatmap({ nights, metric, w, h }: { nights: Table; metric: string; w: number; h: number }) {
  const days = col(nights, 'day');
  const vals = col(nights, metric);
  const last = days[days.length - 1] ?? 0;
  const start = last - 52 * 7 - weekday(last);
  const f = vals.filter(finite).sort((a, b) => a - b);
  const q = (p: number) => f[Math.floor(p * (f.length - 1))] ?? 0;
  const lo = q(0.03), hi = q(0.97);
  const by = new Map(days.map((d, i) => [d, vals[i]!]));
  const cell = Math.max(6, Math.floor(Math.min((w - 28) / 53, (h - 4) / 7)));
  const gap = cell >= 10 ? 2 : 1;
  const cells = [];
  for (let d = start; d <= last; d++) {
    const wk = Math.floor((d - start) / 7), r = weekday(d);
    const v = by.get(d);
    cells.push(<rect key={d} x={28 + wk * cell} y={r * cell} width={cell - gap} height={cell - gap} rx={2} fill={v !== undefined && finite(v) ? sequential((v - lo) / (hi - lo || 1)) : PAL.panel2} onClick={() => v !== undefined && navigate(`/night/${d}`)} style={{ cursor: v !== undefined ? 'pointer' : 'default' }}><title>{isoDay(d)}: {finite(v ?? NaN) ? v!.toFixed(2) : 'no data'}</title></rect>);
  }
  const lab = (r: number, t: string) => <text key={t} x={0} y={r * cell + cell - gap - 1} fontSize={10} fill={PAL.muted}>{t}</text>;
  return <svg width={28 + 53 * cell} height={7 * cell} role="img" aria-label={`Calendar of ${metric}`}>{[lab(0, 'M'), lab(2, 'W'), lab(4, 'F')]}{cells}</svg>;
}

export default function Today() {
  const app = useApp();
  const { nights, lastDay, state, answer, loadDemo } = app;
  const { data: hrv } = useJob(nights && nights.n > 30 ? { kind: 'hrv4t', range_days: state.settings.hrv_range_days, k_daily: state.settings.k_daily, k_swc: state.settings.k_swc } : null);
  const [load, setLoad] = useState(150);
  const [metric, setMetric] = useParam('heat', 'ln_rmssd');
  const model = hrv?.model;
  const w = useMemo(() => (model ? whatIf(model, load) : null), [model, load]);
  if (!nights || nights.n === 0) {
    return (
      <ViewEmpty title="Today">
        {app.loading ? 'Loading your data...' : !app.storageOk ? 'This page is not in a secure context, so the browser cannot store your data. Open it over HTTPS (or on localhost).' : (<>No data yet. <a href="/data" onClick={link('/data')}>Import a Garmin export or connect garmin-mcp</a>, or <button onClick={() => void loadDemo()}>load demo data</button>.</>)}
      </ViewEmpty>
    );
  }
  const h = hrv?.headline as Record<string, any> | undefined;
  const adv = h ? adviceOf(h.advice as string) : undefined;
  const open = state.proposals.filter((p) => p.status === 'open' && p.asked).slice(0, 3);
  const spark = hrv?.spark as Table | undefined;
  return (
    <View
      title={<>Last night <span className="muted mono">{isoDay(lastDay)}</span></>}
      tools={<a href={`/night/${lastDay}`} onClick={link(`/night/${lastDay}`)}>Open the night</a>}
    >
      <div className="vgrid" style={{ gridTemplateColumns: 'minmax(0, 1.1fr) minmax(0, 1fr)', gridTemplateRows: 'minmax(0, 1fr) minmax(0, 1fr)' }}>
        <Panel title="Against your own baseline" scroll>
          <table className="t"><thead><tr><th>Signal</th><th className="n">Last night</th><th className="n">Usual</th><th className="n">z</th><th>Band</th></tr></thead>
            <tbody>{METRICS.map((m) => <MetricRow key={m.id} m={m} day={lastDay} />)}</tbody></table>
        </Panel>
        <Panel title="HRV advice" method="baseline.normal_range" job={{ kind: 'hrv4t' }}>
          {h && adv ? (
            <>
              <div className="row"><span style={{ fontSize: 34, fontWeight: 700 }} className="mono">{num(h.score, 1)}</span><span className="chip" style={{ background: adv.color }}>{adv.label}</span></div>
              <p>{adv.text}</p>
              <div className="row muted" style={{ fontSize: 12 }}>
                <span>7-night baseline {num(h.base_score, 1)} ({signed(h.base_change ?? NaN, 2)})</span><span>CV {num(h.cv7, 2)}</span><span>Pattern: {h.trend ?? 'none'}</span>
                <span>Model expected {num(h.expected, 1)}, surprise {signed(h.surprise ?? NaN, 1)} SD</span>
              </div>
              {spark ? <Fill style={{ minHeight: 40, marginTop: 8 }}>{({ w: sw, h: sh }) => <Spark values={col(spark, 'score')} width={sw} height={Math.max(40, sh)} />}</Fill> : <div style={{ flex: 1 }} />}
              {w && model && (
                <div style={{ marginTop: 10 }}>
                  <label>What if I train <span className="mono">{load}</span> load today? <input type="range" min={0} max={400} step={5} value={load} onChange={(e) => setLoad(+e.target.value)} aria-label="Planned load" /></label>
                  <p className="muted">Tonight's HRV would be about <b className="mono">{w.rmssd.toFixed(0)} ms</b> (80% range {w.lo80.toFixed(0)} to {w.hi80.toFixed(0)}), score {w.score.toFixed(1)}.</p>
                </div>
              )}
            </>
          ) : <Empty>Not enough HRV nights yet.</Empty>}
        </Panel>
        <Panel title="Is this right?" scroll actions={<a href="/inbox" onClick={link('/inbox')}>All suggestions</a>}>
          {open.length === 0 ? <div className="empty">Nothing to confirm right now.</div> : open.map((p) => (
            <div key={p.id} className="row" style={{ padding: '6px 0', borderTop: `1px solid ${PAL.grid}` }}>
              <b style={{ color: kindInfo(p.kind).color }}>{kindInfo(p.kind).name}</b>
              <span className="mono">{isoDay(p.start_day)}{p.end_day > p.start_day ? ` to ${isoDay(p.end_day)}` : ''}</span>
              <span className="chip" style={{ background: PAL.base }}>{confidence(p.posterior[p.kind] ?? 0)}</span>
              <span className="muted" style={{ flex: 1 }}>{why(p.evidence)}</span>
              <button className="primary" onClick={() => void answer(p.id, 'confirm')}>Confirm</button>
              <button onClick={() => void answer(p.id, 'deny')}>Deny</button>
            </div>
          ))}
        </Panel>
        <Panel
          title="The last year"
         
          actions={(
            <select value={metric} onChange={(e) => setMetric(e.target.value)} aria-label="Heatmap metric">
              {['ln_rmssd', 'hr_min30', 'resp_mean', 'sleep_stress', 'tst_h', 'sleep_score', 'dip_pct', 'rmssd_norm'].map((m) => <option key={m} value={m}>{m}</option>)}
            </select>
          )}
        >
          <Fill>{({ w: fw, h: fh }) => <Heatmap nights={nights} metric={metric} w={fw} h={fh} />}</Fill>
        </Panel>
      </div>
    </View>
  );
}
