import { useMemo } from 'react';
import { PAL, diverging, isoDay, sequential, signed } from '@wl/views';
import type { NightChannels } from '@wl/schema';
import { useApp } from '../lib/app';
import { useJob } from '../lib/useJob';
import { link } from '../lib/router';
import { LineChart, type LineSpec } from '../charts/analysis/lines';
import { Whisker } from '../charts/analysis/dots';
import { Heatgrid } from '../charts/analysis/heat';
import { Card, Desktop, Intro, JobNote, NotAvailable } from '../charts/analysis/frame';
import { fmt } from '../charts/analysis/inputs';
import { Fill } from '../ui/Fill';
import { FocusList, type FocusItem } from '../ui/FocusList';
import { View } from '../ui/View';

interface Regression { terms: string[]; beta: (number | null)[]; se: (number | null)[]; r2: number | null; rmse: number | null; n: number; worst: [number, number, number][] }
interface Teardown {
  sleep_score: Regression; readiness_nnls: Regression; readiness_ols: Regression;
  body_battery: { table: [string, boolean, number, number][]; mae: number | null; days: number };
  hrv_band: { window: number[]; width: number[]; mae: (number | null)[]; kind: string[]; best_sd: [number, number, number | null]; best_percentile: [number, number, number | null] };
  stress_vs_hr: { hr_bin: number[]; since_activity: string[]; median_stress: number[]; n: number[] };
}

function RegressionTable({ r, title, method, job, input, note }: { r: Regression; title: string; method: string; job: unknown; input: string; note?: string }) {
  const ok = r.terms.length > 0 && r.n > 0;
  const bs = r.beta.filter((b): b is number => b !== null && Number.isFinite(b));
  const lim = Math.max(1e-6, ...bs.map((b, i) => Math.abs(b) + (r.se[i] ?? 0)));
  return (
    <Card title={title} method={method} job={job} testid={`reg-${method}-${title}`} scroll>
      {!ok ? <NotAvailable input={input} /> : (
        <>
          <table className="t">
            <thead><tr><th>Term</th><th className="n">Coefficient</th><th className="n">SE</th><th>+/- 1 SE</th></tr></thead>
            <tbody>
              {r.terms.map((t, i) => (
                <tr key={t}><td>{t}</td><td className="n">{fmt(r.beta[i])}</td><td className="n">{fmt(r.se[i])}</td><td><Whisker b={r.beta[i] ?? NaN} se={r.se[i] ?? NaN} lo={-lim} hi={lim} /></td></tr>
              ))}
            </tbody>
          </table>
          <p className="mono muted" style={{ fontSize: 12 }}>R&sup2; = {fmt(r.r2)}, RMSE = {fmt(r.rmse)}, n = {r.n} nights. {note}</p>
          {r.worst.length > 0 && (
            <details>
              <summary>Nights the fit misses by the most</summary>
              <table className="t" style={{ maxWidth: 520 }}>
                <thead><tr><th>Night</th><th className="n">Garmin</th><th className="n">Predicted</th><th className="n">Miss</th></tr></thead>
                <tbody>{r.worst.slice(0, 8).map(([d, y, p]) => <tr key={d}><td><a href={`/night/${d}`} onClick={link(`/night/${d}`)}>{isoDay(d)}</a></td><td className="n">{fmt(y, 1)}</td><td className="n">{fmt(p, 1)}</td><td className="n">{signed(y - p, 1)}</td></tr>)}</tbody>
              </table>
            </details>
          )}
        </>
      )}
    </Card>
  );
}

const BANDS = ['0-10', '11-25', '26-50', '51-75', '76-100', 'activity', 'unmeasured'];
const bandOf = (v: number) => (!Number.isFinite(v) ? 'unmeasured' : v <= 10 ? '0-10' : v <= 25 ? '11-25' : v <= 50 ? '26-50' : v <= 75 ? '51-75' : '76-100');
const clockText = (sec: number) => new Date(sec * 1000).toISOString().slice(5, 16).replace('T', ' ');

/** One day of Body Battery rebuilt from the rate table, against what the watch recorded. */
function ExampleDay({ table, day }: { table: Teardown['body_battery']['table']; day: number }) {
  const night = useJob({ kind: 'night', day });
  const ch = night.data as unknown as NightChannels | null;
  const span = ch ? [ch.onset - 14 * 3.6e6, ch.onset + 14 * 3.6e6] : null;
  const bb = useJob(span ? { kind: 'downsample', series: 'body_battery', t0: span[0]!, t1: span[1]!, px: 1200 } : null);
  const st = useJob(span ? { kind: 'downsample', series: 'stress', t0: span[0]!, t1: span[1]!, px: 1200 } : null);
  const sim = useMemo(() => {
    const bt = bb.data?.arrays.t as Float64Array | undefined, bv = bb.data?.arrays.v as Float64Array | undefined;
    const stt = st.data?.arrays.t as Float64Array | undefined, stv = st.data?.arrays.v as Float64Array | undefined;
    if (!ch || !bt || !bv || bt.length < 10) return null;
    const rate = new Map(table.map(([b, a, r]) => [`${b}|${a}`, r]));
    const stressAt = (t: number) => {
      if (!stt || !stv) return NaN;
      let lo = 0, hi = stt.length;
      while (lo < hi) { const m = (lo + hi) >> 1; if (stt[m]! < t) lo = m + 1; else hi = m; }
      const k = lo > 0 && (lo === stt.length || t - stt[lo - 1]! < stt[lo]! - t) ? lo - 1 : lo;
      return k < stt.length && Math.abs(stt[k]! - t) <= 3 * 60_000 ? stv![k]! : NaN;
    };
    const sm: number[] = [bv[0]!];
    let err = 0, n = 0;
    for (let i = 1; i < bt.length; i++) {
      const dt = (bt[i]! - bt[i - 1]!) / 3.6e6;
      const prev = sm[i - 1]!;
      if (dt <= 0 || dt > 0.12) { sm.push(prev + (bv[i]! - bv[i - 1]!)); continue; }
      const asleep = bt[i - 1]! >= ch.onset && bt[i - 1]! < ch.wake;
      sm.push(Math.max(0, Math.min(100, prev + (rate.get(`${bandOf(stressAt(bt[i - 1]!))}|${asleep}`) ?? 0) * dt)));
      err += Math.abs(sm[i]! - bv[i]!); n++;
    }
    const off = ch.utc_offset_min * 60;
    return { x: Array.from(bt, (t) => t / 1000 + off), obs: Array.from(bv), sim: sm, mae: n ? err / n : NaN };
  }, [ch, bb.data, st.data, table]);
  const lines = useMemo<LineSpec[]>(() => (sim ? [{ label: 'recorded', y: sim.obs, color: PAL.hrv }, { label: 'rebuilt from the table', y: sim.sim, color: PAL.hr, dash: [5, 4] }] : []), [sim]);
  if (night.error) return <p className="muted">No night to show an example for ({night.error}).</p>;
  if (!sim) return <p className="muted">{night.loading || bb.loading ? 'Loading the example day...' : 'No Body Battery samples around the last night.'}</p>;
  return (
    <>
      <h3>Example: the day around {isoDay(day)}, recorded and rebuilt</h3>
      <div style={{ display: 'flex', flexDirection: 'column', height: 280 }}>
        <Fill>{({ h }) => <LineChart x={sim.x} lines={lines} time xFmt={clockText} height={h} yLabel="Body Battery" ariaLabel="Body Battery recorded and rebuilt from the rate table for one day" testid="chart-bb-example" />}</Fill>
      </div>
      <p className="muted" style={{ fontSize: 12 }}>Mean error on this day: {fmt(sim.mae, 1)} points (local clock; activity periods are approximated by their stress band here).</p>
    </>
  );
}

function BodyBattery({ t, job, day }: { t: Teardown['body_battery']; job: unknown; day: number }) {
  const rows = BANDS.filter((b) => t.table.some((r) => r[0] === b));
  const at = (b: string, a: boolean) => t.table.find((r) => r[0] === b && r[1] === a);
  return (
    <Card title="Body Battery: points per hour by stress band" method="garmin.teardown" job={job} testid="card-bb" scroll>
      {rows.length === 0 ? <NotAvailable input="body_battery and stress series" /> : (
        <>
          <p className="muted" style={{ fontSize: 12 }}>Mean rate at which Body Battery changes, by the stress level of the moment and whether you were asleep (blue cells charge the battery, red cells drain it).</p>
          <Heatgrid
            rows={rows} cols={2} cw={110} ch={28} rowLabelW={90}
            colTicks={[{ i: 0, text: 'awake' }, { i: 1, text: 'asleep' }]}
            value={(r, c) => at(rows[r]!, c === 1)?.[2] ?? null}
            color={(v) => diverging(-v / 4)}
            text={(v) => signed(v, 1)}
            tip={(r, c, v) => `${rows[r]}, ${c === 1 ? 'asleep' : 'awake'}: ${v === null ? 'no samples' : `${signed(v, 2)} points/h`} (${at(rows[r]!, c === 1)?.[3] ?? 0} samples)`}
            ariaLabel="Body Battery rate by stress band and sleep" testid="bb-heatmap"
          />
          <p className="mono muted" style={{ fontSize: 12 }}>Rebuilding the whole record from this table is off by {fmt(t.mae, 2)} points per sample on average, over about {t.days} days.</p>
          <ExampleDay table={t.table} day={day} />
        </>
      )}
    </Card>
  );
}

function BandGrid({ g, job }: { g: Teardown['hrv_band']; job: unknown }) {
  const kinds = ['sd', 'percentile'] as const;
  return (
    <Card title="Garmin's HRV Status band: which window and width reproduce it" method="garmin.teardown" job={job} testid="card-hrv-band" scroll>
      {g.mae.length === 0 || g.mae.every((m) => m === null) ? <NotAvailable input="days.hrv_bal_lo and days.hrv_bal_hi (Garmin HRV status)" /> : (
        <>
          <p className="muted" style={{ fontSize: 12 }}>Mean error (ln ms) between Garmin's balanced range and the range recomputed from your own nightly values, for each window length and band width. Darker is closer.</p>
          <div style={{ display: 'grid', gap: 16, gridTemplateColumns: 'repeat(auto-fit, minmax(480px, 1fr))' }}>
            {kinds.map((k) => {
              const idx = g.kind.map((x, i) => (x === k ? i : -1)).filter((i) => i >= 0);
              const wins = [...new Set(idx.map((i) => g.window[i]!))];
              const widths = [...new Set(idx.map((i) => g.width[i]!))];
              const at = (w: number, c: number) => { const i = idx.find((j) => g.window[j] === w && g.width[j] === c); return i === undefined ? null : g.mae[i] ?? null; };
              const vals = idx.map((i) => g.mae[i]).filter((v): v is number => v !== null && Number.isFinite(v));
              const [lo, hi] = [Math.min(...vals), Math.max(...vals)];
              const best = (k === 'sd' ? g.best_sd : g.best_percentile);
              const mark: [number, number] | null = best[2] !== null ? [wins.indexOf(best[0]), widths.findIndex((w) => Math.abs(w - best[1]) < 1e-9)] : null;
              return (
                <div key={k}>
                  <h3>{k === 'sd' ? 'Mean plus or minus c SD' : 'Percentile band (p to 1 - p)'}</h3>
                  <Heatgrid
                    rows={wins.map((w) => `${w} nights`)} cols={widths.length} cw={k === 'sd' ? 28 : 56} ch={22} rowLabelW={80}
                    colTicks={widths.map((w, i) => ({ i, text: k === 'sd' && i % 2 ? '' : w.toFixed(2) })) }
                    value={(r, c) => at(wins[r]!, widths[c]!)}
                    color={(v) => sequential(1 - (v - lo) / (hi - lo || 1))}
                    tip={(r, c, v) => `${wins[r]}-night window, ${k === 'sd' ? 'c' : 'p'} = ${widths[c]!.toFixed(2)}: ${v === null ? 'no fit' : `error ${v.toFixed(3)}`}`}
                    mark={mark && mark[0] >= 0 && mark[1] >= 0 ? mark : null}
                    ariaLabel={`HRV band grid, ${k}`} testid={`band-grid-${k}`}
                  />
                </div>
              );
            })}
          </div>
          <p data-testid="band-best" style={{ fontSize: 13 }}>
            Best fit: {g.best_sd[2] !== null ? <>mean plus or minus <b>{fmt(g.best_sd[1])} SD</b> over the last <b>{g.best_sd[0]} nights</b> (error {fmt(g.best_sd[2], 3)})</> : 'none'}
            {g.best_percentile[2] !== null && <>; as percentiles, <b>{Math.round(g.best_percentile[1] * 100)}th to {Math.round(100 - g.best_percentile[1] * 100)}th</b> over <b>{g.best_percentile[0]} nights</b> (error {fmt(g.best_percentile[2], 3)})</>}.
          </p>
        </>
      )}
    </Card>
  );
}

function StressHr({ s, job }: { s: Teardown['stress_vs_hr']; job: unknown }) {
  const cols = ['0-1 h', '1-3 h', '3-6 h', '6+ h'];
  const bins = [...new Set(s.hr_bin)].sort((a, b) => a - b);
  const at = (b: number, c: string) => { const i = s.hr_bin.findIndex((x, k) => x === b && s.since_activity[k] === c); return i < 0 ? null : i; };
  const vals = s.median_stress.filter(Number.isFinite);
  const [lo, hi] = [Math.min(...vals), Math.max(...vals)];
  return (
    <Card title="Stress against heart rate" method="garmin.teardown" job={job} testid="card-stress-hr" scroll>
      {bins.length === 0 ? <NotAvailable input="stress and hr series with activities" /> : (
        <>
          <p className="muted" style={{ fontSize: 12 }}>Median Garmin stress (0 to 100) by how far your heart rate is above your resting rate, and how long ago your last activity ended. Garmin's stress is derived from heart-rate variability, so it should rise with heart rate and fall back after exercise.</p>
          <Heatgrid
            rows={bins.map((b) => `resting ${b >= 0 ? '+' : '\u2212'}${Math.abs(b)} bpm`)} cols={cols.length} cw={90} ch={24} rowLabelW={130}
            colTicks={cols.map((c, i) => ({ i, text: c }))}
            value={(r, c) => { const i = at(bins[r]!, cols[c]!); return i === null ? null : s.median_stress[i]!; }}
            color={(v) => sequential((v - lo) / (hi - lo || 1))}
            text={(v) => v.toFixed(0)}
            tip={(r, c, v) => { const i = at(bins[r]!, cols[c]!); return `${cols[c]} after activity, resting ${bins[r]! >= 0 ? '+' : ''}${bins[r]} bpm: ${v === null ? 'too few samples' : `median stress ${v.toFixed(0)}`}${i === null ? '' : ` (n = ${s.n[i]})`}`; }}
            ariaLabel="Median stress by heart-rate bin and time since activity" testid="stress-hr-heatmap"
          />
        </>
      )}
    </Card>
  );
}

function Body() {
  const { nights, lastDay } = useApp();
  const job = useMemo(() => (nights && nights.n > 30 ? { kind: 'teardown' as const } : null), [nights]);
  const { data, loading, error } = useJob(job);
  const t = data as unknown as Teardown | null;
  const items: FocusItem[] = [
    { id: 'sleep', title: 'Sleep score from its sub-scores' },
    { id: 'ready-nnls', title: 'Readiness: non-negative weights' },
    { id: 'ready-ols', title: 'Readiness: ordinary least squares' },
    { id: 'bb', title: 'Body Battery' },
    { id: 'band', title: 'HRV Status band' },
    { id: 'stress', title: 'Stress against heart rate' },
  ];
  return (
    <View title="Garmin" desc="What Garmin's scores are made of." tools={<JobNote loading={loading} error={error} />}>
      <Intro>What Garmin's own inputs predict, and where its numbers depart from them. Garmin publishes the pieces behind its sleep score and readiness; this fits them back to the score, rebuilds Body Battery from stress and sleep, and finds the HRV Status band it must be using.</Intro>
      <FocusList items={items} label="Garmin charts" render={(it) => {
        if (!nights || nights.n === 0) return <div className="empty">No nights loaded yet.</div>;
        if (!t) return <div className="empty">{loading ? 'Working...' : ''}</div>;
        switch (it.id) {
          case 'ready-nnls': return <RegressionTable r={t.readiness_nnls} title="Readiness: non-negative weights" method="garmin.teardown" job={job} input="days.tr_*_pct (Garmin training readiness factors)" note="Non-negative least squares, so no standard errors." />;
          case 'ready-ols': return <RegressionTable r={t.readiness_ols} title="Readiness: ordinary least squares" method="garmin.teardown" job={job} input="days.tr_*_pct (Garmin training readiness factors)" />;
          case 'bb': return <BodyBattery t={t.body_battery} job={job} day={lastDay} />;
          case 'band': return <BandGrid g={t.hrv_band} job={job} />;
          case 'stress': return <StressHr s={t.stress_vs_hr} job={job} />;
          default: return <RegressionTable r={t.sleep_score} title="Sleep score from its sub-scores" method="garmin.teardown" job={job} input="days.sleep_*_score (Garmin sleep sub-scores)" />;
        }
      }} />
    </View>
  );
}

export default function Garmin() {
  return <Desktop><Body /></Desktop>;
}
