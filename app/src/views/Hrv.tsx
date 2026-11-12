import { useMemo, useState } from 'react';
import { area as d3area, line as d3line } from 'd3-shape';
import type { Table } from '@wl/schema';
import { ADVICE_COLORS, LOAD_COLORS, PAL, isoDay, mix } from '@wl/views';
import { useJob } from '../lib/useJob';
import { useNumParam, useParam } from '../lib/router';
import { col, finite, txt } from '../lib/util';
import { Empty } from '../charts/small';
import { Plot } from '../charts/Plot';
import { Raster } from '../charts/Raster';
import { TimeChart, type Shade } from '../charts/TimeChart';
import { ChartCard, GOOD, Legend, Missing, NeedsLargerScreen, Select, Slider, Split, useNarrow, useNotes } from '../charts/lab';
import { loess, quantile, sortedFinite } from '../charts/loess';
import { AcuteNote, BaselineNote, IntroNote, ModelNote, NightNote, NightsNote, TrendsNote, type ModelNums } from '../charts/notes';

const SPANS: Record<string, [string, number]> = { '90': ['Last 90 nights', 90], '6m': ['Last 6 months', 183], '1y': ['Last year', 365], all: ['Everything', 100000] };
const RANGES: [string, string][] = [['28', '28 nights (Vesterinen, Nuuttila)'], ['60', '60 nights (HRV4Training)'], ['90', '90 nights']];
const TABS: [string, string][] = [['baseline', 'Baseline'], ['trends', 'Trends'], ['response', 'Response'], ['night', 'Inside the night'], ['model', 'Model']];
const CLASSES = ['rest', 'easy', 'moderate', 'hard'] as const;
const TREND_COLORS: Record<string, string> = { 'coping well': GOOD, 'accumulated fatigue': PAL.alert, saturation: PAL.violet, maladaptation: PAL.hr };

export const adviceColor = (a: string | null | undefined): string => {
  switch (a) {
    case 'normal': return ADVICE_COLORS.normal;
    case 'high': return ADVICE_COLORS.high;
    case 'watch': return ADVICE_COLORS.watch;
    case 'keep it easy': return ADVICE_COLORS.easy;
    case 'back off': return ADVICE_COLORS.back_off;
    default: return PAL.muted;
  }
};

interface Headline { day: string; score: number; base_score: number; base_change: number; lo: number; hi: number; swc_lo: number; swc_hi: number; advice: string | null; advice_text: string; trend: string | null; expected: number | null; surprise: number | null; nights: number }
interface Result { nightly: Table; scored: Table; acute_lag: Table; acute_pts: Table; barcode: Table; stage_hrv: Table; night_sens: Table; model_view: Table; model: ModelNums; load_cuts: [number, number]; headline: Headline | null }

/** Rows of a night table in the view span, as column getters. */
function useView(t: Table, from: number) {
  return useMemo(() => {
    const day = col(t, 'day');
    const keep: number[] = [];
    day.forEach((d, i) => d >= from && keep.push(i));
    const get = (name: string) => { const c = col(t, name); return keep.map((i) => c[i]!); };
    const gets = (name: string) => { const c = txt(t, name); return keep.map((i) => c[i] ?? null); };
    return { days: keep.map((i) => day[i]!), get, gets };
  }, [t, from]);
}

export default function Hrv() {
  const narrow = useNarrow();
  if (narrow) return <NeedsLargerScreen name="HRV" />;
  return <HrvView />;
}

function HrvView() {
  const [span, setSpan] = useParam('span', '6m');
  const [nr, setNr] = useNumParam('nr', 60);
  const [kd, setKd] = useNumParam('kd', 1.5);
  const [ks, setKs] = useNumParam('ks', 0.5);
  const [tab, setTab] = useParam('tab', 'baseline');
  const notes = useNotes();
  const job = { kind: 'hrv4t' as const, range_days: nr, k_daily: kd, k_swc: ks };
  const { data, loading, error } = useJob(job);
  const r = data as unknown as Result | null;
  const spanDays = (SPANS[span] ?? SPANS['6m']!)[1];

  const body = (() => {
    if (error) return <div className="card err" role="alert">{error}</div>;
    if (!r) return <Empty>{loading ? 'Running the HRV analysis...' : 'No data yet.'}</Empty>;
    if (r.nightly.n === 0) return <ChartCard title="HRV" method="baseline.normal_range" job={job}><Missing needs="Garmin nightly HRV or five-minute HRV values (hrv5)" /></ChartCard>;
    const lastDay = Math.max(...col(r.nightly, 'day'));
    const from = lastDay - spanDays;
    return (
      <>
        {r.headline && <Latest h={r.headline} />}
        <div className="pill-row" role="tablist">
          {TABS.map(([k, t]) => <button key={k} role="tab" aria-selected={tab === k} className={tab === k ? 'on' : ''} onClick={() => setTab(k)}>{t}</button>)}
        </div>
        {tab === 'baseline' && <BaselineChart r={r} from={from} job={job} />}
        {tab === 'trends' && <TrendsCharts r={r} from={from} />}
        {tab === 'response' && <ResponseCharts r={r} />}
        {tab === 'night' && <InsideNight r={r} from={from} />}
        {tab === 'model' && <ModelChart r={r} from={from} />}
      </>
    );
  })();

  const aside = r && r.nightly.n > 0 ? <HrvNotes r={r} tab={tab} /> : <IntroNote />;
  return (
    <div className="page">
      <h1>HRV</h1>
      <p className="muted">Nightly HRV against your own normal range, trend patterns, the response to training, the inside of the night and a model of what tonight should be. Press <kbd>n</kbd> for the notes.</p>
      <div className="controls">
        <Select label="Show" value={span} options={Object.entries(SPANS).map(([k, v]) => [k, v[0]] as [string, string])} onChange={(v) => setSpan(v)} />
        <Select label="Normal range from" value={String(nr)} options={RANGES} onChange={(v) => setNr(Number(v))} />
        <Slider label="Night range (SD)" min={0.5} max={2.5} step={0.25} value={kd} onCommit={setKd} fmt={(v) => v.toFixed(2)} />
        <Slider label="Baseline band (SD)" min={0.25} max={1} step={0.05} value={ks} onCommit={setKs} fmt={(v) => v.toFixed(2)} />
      </div>
      <Split notes={notes} aside={aside}>{body}</Split>
    </div>
  );
}

function Latest({ h }: { h: Headline }) {
  return (
    <section className="card" data-testid="latest">
      <div className="row">
        <b>{h.day}</b>
        <span className="chip" style={{ background: adviceColor(h.advice) }}>{h.advice ?? 'no advice yet'}</span>
        <span className="mono">score {h.score} (range {h.lo} to {h.hi})</span>
        <span className="mono">baseline {h.base_score} (band {h.swc_lo} to {h.swc_hi})</span>
        {h.trend && <span className="muted">pattern: {h.trend}</span>}
      </div>
      {h.advice_text && <p style={{ margin: '6px 0 0' }}>{h.advice_text}</p>}
    </section>
  );
}

function HrvNotes({ r, tab }: { r: Result; tab: string }) {
  const nightly = r.nightly;
  const stats = useMemo(() => {
    const src = txt(nightly, 'rmssd_src'), a = col(nightly, 'rmssd'), g = col(nightly, 'garmin');
    const xs: number[] = [], ys: number[] = [];
    src.forEach((s, i) => s === '5-min values' && finite(a[i]) && finite(g[i]) && (xs.push(a[i]!), ys.push(g[i]!)));
    const n = xs.length;
    if (n < 3) return { n, r: NaN, mae: NaN };
    const mx = xs.reduce((s, v) => s + v, 0) / n, my = ys.reduce((s, v) => s + v, 0) / n;
    let sxy = 0, sxx = 0, syy = 0, ae = 0;
    xs.forEach((x, i) => { sxy += (x - mx) * (ys[i]! - my); sxx += (x - mx) ** 2; syy += (ys[i]! - my) ** 2; ae += Math.abs(x - ys[i]!); });
    return { n, r: sxy / Math.sqrt(sxx * syy), mae: ae / n };
  }, [nightly]);
  const lag = (k: string, night: number) => {
    const c = txt(r.acute_lag, 'klass'), nn = col(r.acute_lag, 'night'), m = col(r.acute_lag, 'mean');
    const i = c.findIndex((v, j) => v === k && nn[j] === night);
    return i >= 0 && finite(m[i]) ? m[i]! : null;
  };
  const sens = (name: string) => {
    const mm = txt(r.night_sens, 'measure'), e = col(r.night_sens, 'effect');
    const i = mm.indexOf(name);
    return i >= 0 && finite(e[i]) ? e[i]! : null;
  };
  return (
    <>
      {tab === 'baseline' && <><IntroNote /><NightsNote n={stats.n} r={stats.r} mae={stats.mae} cuts={r.load_cuts} /><BaselineNote /></>}
      {tab === 'trends' && <TrendsNote />}
      {tab === 'response' && <AcuteNote hard={lag('hard', 1)} rest={lag('rest', 1)} />}
      {tab === 'night' && <NightNote whole={sens('whole night')} first4={sens('first 4 h')} />}
      {tab === 'model' && <ModelNote m={r.model.nights > 0 ? r.model : null} />}
    </>
  );
}

// ---- Baseline -------------------------------------------------------------------------------------------------

function BaselineChart({ r, from, job }: { r: Result; from: number; job: unknown }) {
  const v = useView(r.scored, from);
  const c = useMemo(() => {
    const ms = (n: string) => v.get(n).map(Math.exp);
    const lines = [
      { y: ms('lo'), stroke: 'transparent' }, { y: ms('hi'), stroke: 'transparent' },
      { y: ms('swc_lo'), stroke: 'transparent' }, { y: ms('swc_hi'), stroke: 'transparent' },
      { y: v.get('g_lo'), stroke: PAL.muted, width: 1, dash: [2, 3] }, { y: v.get('g_hi'), stroke: PAL.muted, width: 1, dash: [2, 3] },
      { y: ms('base7'), stroke: PAL.text, width: 2 },
    ];
    const advice = v.gets('advice');
    return { lines, advice, rmssd: v.get('rmssd'), score: v.get('score'), bscore: v.get('base_score'), garmin: v.get('garmin'), gstat: v.gets('g_status'), load: v.get('load') };
  }, [v]);
  const dots = useMemo(() => ({ y: c.rmssd, color: (i: number) => adviceColor(c.advice[i]), r: () => 3.4 }), [c]);
  const bands = useMemo(() => [{ a: 0, b: 1, fill: `${PAL.hrv}1a` }, { a: 2, b: 3, fill: `${PAL.base}38` }], []);
  if (!v.days.length) return <Missing needs="nights with HRV in this span" />;
  return (
    <ChartCard title="Nightly HRV against your normal range" method="baseline.normal_range" job={job}
      subtitle="Dots: nights. Line: 7-night baseline. Light band: range for a night; blue band: range for the baseline; dotted: Garmin's balanced band">
      <Legend items={[{ color: ADVICE_COLORS.normal, label: 'normal' }, { color: ADVICE_COLORS.high, label: 'high' }, { color: ADVICE_COLORS.watch, label: 'watch' }, { color: ADVICE_COLORS.easy, label: 'keep it easy' }, { color: ADVICE_COLORS.back_off, label: 'back off' }]} />
      <TimeChart label="Nightly HRV against the normal range" days={v.days} lines={c.lines} bands={bands} dots={dots} log height={330} yTitle="rMSSD (ms, log scale)"
        readout={(i) => `${isoDay(v.days[i]!)}  rMSSD ${finite(c.rmssd[i]) ? c.rmssd[i]!.toFixed(0) : '-'} ms  score ${finite(c.score[i]) ? c.score[i]!.toFixed(1) : '-'}  baseline ${finite(c.bscore[i]) ? c.bscore[i]!.toFixed(1) : '-'}  ${c.advice[i] ?? ''}  Garmin ${finite(c.garmin[i]) ? c.garmin[i] : '-'} ${(c.gstat[i] ?? '').toLowerCase()}  load ${finite(c.load[i]) ? c.load[i]!.toFixed(0) : '-'}`} />
    </ChartCard>
  );
}

// ---- Trends ---------------------------------------------------------------------------------------------------

function TrendsCharts({ r, from }: { r: Result; from: number }) {
  const v = useView(r.scored, from);
  const t = useMemo(() => {
    const trend = v.gets('trend');
    const shade: Shade[] = [];
    trend.forEach((k, i) => {
      if (!k || k === 'stable' || k === 'warming up') return;
      const last = shade[shade.length - 1];
      const fill = `${TREND_COLORS[k] ?? PAL.muted}47`;
      if (last && last.fill === fill && last.d1 + 1 === v.days[i]) last.d1 = v.days[i]!;
      else shade.push({ d0: v.days[i]!, d1: v.days[i]!, fill });
    });
    const mk = (y: number[], stroke: string) => [{ y, stroke, width: 1.8 }];
    return {
      trend, shade,
      base: mk(v.get('base7').map(Math.exp), PAL.hrv), hr: mk(v.get('hr7'), PAL.hr), cv: mk(v.get('cv7s'), PAL.violet), norm: mk(v.get('norm7'), PAL.base),
    };
  }, [v]);
  if (!v.days.length) return <Missing needs="nights with HRV in this span" />;
  const noHr = t.hr[0]!.y.every((x) => !Number.isFinite(x));
  const panel = (name: string, lines: typeof t.base, fmt: (x: number) => string, h = 150) => (
    <TimeChart label={name} days={v.days} lines={lines} shade={t.shade} height={h} idle={name} sync="hrv-trends"
      readout={(i) => `${isoDay(v.days[i]!)}  ${name} ${finite(lines[0]!.y[i]) ? fmt(lines[0]!.y[i]!) : '-'}  ${t.trend[i] ?? ''}`} />
  );
  return (
    <ChartCard title="Four weekly signals and the pattern they make" method="hrv.trends" job={{ kind: 'hrv4t' }}
      subtitle="Shaded: nights where the 14-night slopes form one of HRV4Training's patterns">
      <Legend items={Object.entries(TREND_COLORS).map(([label, color]) => ({ label, color }))} />
      {panel('HRV baseline (ms)', t.base, (x) => x.toFixed(0), 170)}
      {noHr ? <Missing needs="sleeping heart rate (sleeps.avg_hr)" /> : (
        <>
          {panel('Sleeping HR (bpm)', t.hr, (x) => x.toFixed(1))}
          {panel('CV of ln rMSSD (%)', t.cv, (x) => x.toFixed(2))}
          {panel('rMSSD / mean RR', t.norm, (x) => x.toFixed(3))}
        </>
      )}
    </ChartCard>
  );
}

// ---- Response -------------------------------------------------------------------------------------------------

function ResponseCharts({ r }: { r: Result }) {
  const lag = r.acute_lag;
  const rows = useMemo(() => {
    const k = txt(lag, 'klass'), n = col(lag, 'night'), m = col(lag, 'mean'), lo = col(lag, 'lo'), hi = col(lag, 'hi'), cnt = col(lag, 'n');
    return k.map((c, i) => ({ c: c ?? '', night: n[i]!, mean: m[i]! * 100, lo: lo[i]! * 100, hi: hi[i]! * 100, n: cnt[i]! })).filter((x) => finite(x.mean));
  }, [lag]);
  const pts = useMemo(() => {
    const day = col(r.acute_pts, 'day'), dev = col(r.acute_pts, 'dev'), load = col(r.acute_pts, 'load'), cls = txt(r.acute_pts, 'load_class');
    const out: { day: number; load: number; dev: number; cls: string }[] = [];
    day.forEach((d, i) => finite(dev[i]) && load[i]! > 0 && out.push({ day: d, load: load[i]!, dev: dev[i]! * 100, cls: cls[i] ?? 'rest' }));
    return out;
  }, [r.acute_pts]);
  const fit = useMemo(() => loess(pts.map((p) => p.load), pts.map((p) => p.dev), 0.5), [pts]);
  if (!rows.length) return <ChartCard title="Nights after training" method="hrv.response" job={{ kind: 'hrv4t' }}><Missing needs="activities with a training load (activities.load)" /></ChartCard>;
  const ys = rows.flatMap((x) => [x.lo, x.hi]).filter(finite);
  const y0 = Math.min(0, ...ys) - 1, y1 = Math.max(0, ...ys) + 1;
  const maxLoad = Math.max(10, ...pts.map((p) => p.load));
  const legend = CLASSES.map((c) => ({ label: c, color: LOAD_COLORS[c] }));
  return (
    <div className="cols">
      <ChartCard title="Nights after training" method="hrv.response" job={{ kind: 'hrv4t' }} subtitle="Mean and 95 % interval, night 1 = the night right after">
        <Legend items={legend} />
        <Plot height={290} x={{ domain: [0.5, 3.5], ticks: [1, 2, 3].map((v) => ({ v, label: `night ${v}` })), noGrid: true }} y={{ domain: [y0, y1], label: 'vs baseline (%)' }} label="Nights after training by load class">
          {({ sx, sy }) => (
            <>
              <line x1={0} x2={sx(3.5)} y1={sy(0)} y2={sy(0)} stroke={PAL.muted} strokeDasharray="3 3" />
              {rows.map((x) => {
                const cx = sx(x.night + (CLASSES.indexOf(x.c as never) - 1.5) * 0.16);
                const color = LOAD_COLORS[x.c as keyof typeof LOAD_COLORS] ?? PAL.muted;
                return (
                  <g key={`${x.c}${x.night}`}>
                    <line x1={cx} x2={cx} y1={sy(x.lo)} y2={sy(x.hi)} stroke={color} strokeWidth={2} />
                    <circle cx={cx} cy={sy(x.mean)} r={4.5} fill={color}><title>{`${x.c}, night ${x.night}: ${x.mean >= 0 ? '+' : ''}${x.mean.toFixed(1)} % (n = ${x.n})`}</title></circle>
                  </g>
                );
              })}
            </>
          )}
        </Plot>
      </ChartCard>
      <ChartCard title="Dose and response" method="hrv.response" job={{ kind: 'hrv4t' }} subtitle="Every night after a training day; line: local regression">
        <Legend items={legend} />
        {pts.length < 5 ? <Missing needs="nights after training days" /> : (
          <Plot height={290} x={{ domain: [0, maxLoad], scale: 'sqrt', label: 'Training load the day before' }} y={{ domain: [-40, 40], clamp: true, label: 'Night vs baseline (%)' }} label="Training load against the next night's HRV">
            {({ sx, sy }) => (
              <>
                <line x1={0} x2={sx(maxLoad)} y1={sy(0)} y2={sy(0)} stroke={PAL.muted} strokeDasharray="3 3" />
                {pts.map((p) => <circle key={p.day} cx={sx(p.load)} cy={sy(p.dev)} r={2.4} fill={LOAD_COLORS[p.cls as keyof typeof LOAD_COLORS] ?? PAL.muted} opacity={0.4}><title>{`${isoDay(p.day)}: load ${p.load.toFixed(0)}, ${p.dev >= 0 ? '+' : ''}${p.dev.toFixed(1)} %`}</title></circle>)}
                <path d={d3line<[number, number]>().x((p) => sx(p[0])).y((p) => sy(p[1]))(fit) ?? ''} fill="none" style={{ stroke: 'var(--text)' }} strokeWidth={2.2} />
              </>
            )}
          </Plot>
        )}
      </ChartCard>
    </div>
  );
}

// ---- Inside the night -----------------------------------------------------------------------------------------

const hexRgb = (c: string): [number, number, number] => { const n = parseInt(c.slice(1), 16); return [(n >> 16) & 255, (n >> 8) & 255, n & 255]; };

function InsideNight({ r, from }: { r: Result; from: number }) {
  const [hover, setHover] = useState<string>('');
  const bc = r.barcode;
  const g = useMemo(() => {
    const day = col(bc, 'day'), h = col(bc, 'h'), v = col(bc, 'rmssd');
    const rows = [...new Set(day.filter((d) => d >= from))].sort((a, b) => a - b);
    if (!rows.length) return null;
    const width = 60, rowOf = new Map(rows.map((d, i) => [d, i]));
    const grid = new Float64Array(rows.length * width).fill(NaN);
    const vals: number[] = [];
    const bins = new Map<number, number[]>();
    day.forEach((d, i) => {
      if (d < from || !finite(v[i])) return;
      const k = Math.min(width - 1, Math.round(h[i]! * 6));
      grid[rowOf.get(d)! * width + k] = v[i]!;
      vals.push(v[i]!);
      (bins.get(k) ?? bins.set(k, []).get(k)!).push(v[i]!);
    });
    const s = sortedFinite(vals);
    const [lo, md, hi] = [quantile(s, 0.03), quantile(s, 0.5), quantile(s, 0.97)];
    const [c0, c1, c2] = [hexRgb('#1c2b55'), hexRgb(PAL.hrv), hexRgb('#fff3b0')];
    const rgba = new Uint8Array(grid.length * 4);
    const bg = hexRgb(PAL.panel2);
    grid.forEach((x, i) => {
      let c = bg;
      if (Number.isFinite(x)) {
        const lx = Math.log(Math.min(hi, Math.max(lo, x)));
        const t = lx <= Math.log(md) ? (lx - Math.log(lo)) / (Math.log(md) - Math.log(lo) || 1) : (lx - Math.log(md)) / (Math.log(hi) - Math.log(md) || 1);
        c = hexRgb(lx <= Math.log(md) ? mix(rgb(c0), rgb(c1), t) : mix(rgb(c1), rgb(c2), t));
      }
      rgba.set([c[0], c[1], c[2], 255], i * 4);
    });
    const prof = [...bins.entries()].filter(([, a]) => a.length >= 10).sort((a, b) => a[0] - b[0]).map(([k, a]) => {
      const z = sortedFinite(a);
      return { h: k / 6, med: quantile(z, 0.5), q1: quantile(z, 0.25), q3: quantile(z, 0.75) };
    });
    return { rows, width, rgba, prof, lo, md, hi, grid };
  }, [bc, from]);
  if (!g) return <ChartCard title="Every night, five minutes at a time" method="hrv.inside_night" job={{ kind: 'hrv4t' }}><Missing needs="five-minute HRV values (hrv5) with sleep onsets" /></ChartCard>;
  const stages = (() => {
    const s = txt(r.stage_hrv, 'stage'), rel = col(r.stage_hrv, 'rel'), q1 = col(r.stage_hrv, 'q1'), q3 = col(r.stage_hrv, 'q3'), n = col(r.stage_hrv, 'n');
    return ['deep', 'light', 'rem', 'awake'].map((k) => { const i = s.indexOf(k); return i < 0 ? null : { k, rel: rel[i]!, q1: q1[i]!, q3: q3[i]!, n: n[i]! }; }).filter((x): x is NonNullable<typeof x> => !!x);
  })();
  const sens = (() => {
    const m = txt(r.night_sens, 'measure'), e = col(r.night_sens, 'effect');
    return ['whole night', 'first 4 h', 'Garmin'].map((k) => ({ k, e: e[m.indexOf(k)] ?? NaN })).filter((x) => finite(x.e));
  })();
  const STAGE_COLORS: Record<string, string> = { deep: PAL.base, light: PAL.hrv, rem: PAL.violet, awake: PAL.hr };
  const profY: [number, number] = g.prof.length ? [Math.min(...g.prof.map((p) => p.q1)) * 0.95, Math.max(...g.prof.map((p) => p.q3)) * 1.05] : [0, 1];
  const sMin = Math.min(0, ...stages.map((s) => s.q1)) - 2, sMax = Math.max(0, ...stages.map((s) => s.q3)) + 2;
  const sensMax = Math.max(0.1, ...sens.map((s) => s.e)) * 1.15, sensMin = Math.min(0, ...sens.map((s) => s.e));
  const job = { kind: 'hrv4t' };
  return (
    <div className="cols" style={{ gridTemplateColumns: 'repeat(auto-fit, minmax(380px, 1fr))' }}>
      <ChartCard title="Every night, five minutes at a time" method="hrv.inside_night" job={job} subtitle="One row per night (newest at the bottom), colour: rMSSD">
        <div className="mono muted" style={{ fontSize: 12, minHeight: 18 }}>{hover || `rMSSD from ${g.lo.toFixed(0)} (dark) through ${g.md.toFixed(0)} (teal) to ${g.hi.toFixed(0)} ms (pale)`}</div>
        <Raster rgba={g.rgba} width={g.width} height={g.rows.length} rowPx={g.rows.length > 250 ? 2 : 3}
          onHover={(row, c) => { const v = g.grid[row * g.width + c]; setHover(`${isoDay(g.rows[row]!)}  ${(c / 6).toFixed(1)} h in  ${Number.isFinite(v) ? `${v!.toFixed(0)} ms` : 'no value'}`); }} />
        <div className="mono muted" style={{ display: 'flex', justifyContent: 'space-between', fontSize: 11 }}>{[0, 2, 4, 6, 8, 10].map((h) => <span key={h}>{h} h</span>)}</div>
        <div className="muted" style={{ fontSize: 11, textAlign: 'center' }}>Hours since falling asleep</div>
      </ChartCard>
      <div className="stack">
        <ChartCard title="The typical night" method="hrv.inside_night" job={job} subtitle="Median and middle half; amber: the first-4-hours window">
          {g.prof.length < 2 ? <Missing needs="at least 10 nights of five-minute values" /> : (
            <Plot height={170} x={{ domain: [0, 10], label: 'Hours since falling asleep' }} y={{ domain: profY, label: 'rMSSD (ms)' }} label="Typical night: median and middle half of rMSSD">
              {({ sx, sy }) => (
                <>
                  <rect x={sx(0.5)} width={sx(4.5) - sx(0.5)} y={0} height={sy(profY[0])} fill={PAL.hr} opacity={0.1} />
                  <path d={d3area<(typeof g.prof)[number]>().x((p) => sx(p.h)).y0((p) => sy(p.q1)).y1((p) => sy(p.q3))(g.prof) ?? ''} fill={PAL.hrv} opacity={0.25} />
                  <path d={d3line<(typeof g.prof)[number]>().x((p) => sx(p.h)).y((p) => sy(p.med))(g.prof) ?? ''} fill="none" stroke={PAL.hrv} strokeWidth={2} />
                </>
              )}
            </Plot>
          )}
        </ChartCard>
        <ChartCard title="By sleep stage" method="hrv.inside_night" job={job} subtitle="Garmin's wrist-estimated stages; five-minute values, median and middle half">
          {!stages.length ? <Missing needs="sleep stages (stages table)" /> : (
            <Plot height={150} margin={{ l: 56 }} x={{ domain: [sMin, sMax], label: "vs the night's mean (%)" }} y={{ domain: [-0.5, stages.length - 0.5], ticks: stages.map((s, i) => ({ v: stages.length - 1 - i, label: s.k })), noGrid: true }} label="HRV by sleep stage">
              {({ sx, sy }) => (
                <>
                  <line x1={sx(0)} x2={sx(0)} y1={0} y2={sy(-0.5)} stroke={PAL.muted} strokeDasharray="3 3" />
                  {stages.map((s, i) => {
                    const y = sy(stages.length - 1 - i);
                    return (
                      <g key={s.k}>
                        <line x1={sx(s.q1)} x2={sx(s.q3)} y1={y} y2={y} stroke={STAGE_COLORS[s.k]} strokeWidth={2} />
                        <circle cx={sx(s.rel)} cy={y} r={5.5} fill={STAGE_COLORS[s.k]}><title>{`${s.k}: ${s.rel >= 0 ? '+' : ''}${s.rel.toFixed(1)} % (n = ${s.n})`}</title></circle>
                      </g>
                    );
                  })}
                </>
              )}
            </Plot>
          )}
        </ChartCard>
        <ChartCard title="Which number sees training" method="hrv.inside_night" job={job} subtitle="Bigger is more sensitive">
          {!sens.length ? <Missing needs="hard and rest training days" /> : (
            <Plot height={110} margin={{ l: 96 }} x={{ domain: [sensMin, sensMax], label: 'Rest night minus hard night (SD)' }} y={{ domain: [-0.5, sens.length - 0.5], ticks: sens.map((s, i) => ({ v: sens.length - 1 - i, label: s.k })), noGrid: true }} label="Sensitivity of three HRV measures to training">
              {({ sx, sy }) => sens.map((s, i) => {
                const y = sy(sens.length - 1 - i), bh = Math.abs(sy(0.3) - sy(-0.3));
                return <rect key={s.k} x={Math.min(sx(0), sx(s.e))} width={Math.abs(sx(s.e) - sx(0))} y={y - bh / 2} height={bh} rx={3} fill={PAL.hr}><title>{`${s.k}: ${s.e.toFixed(2)} SD`}</title></rect>;
              })}
            </Plot>
          )}
        </ChartCard>
      </div>
    </div>
  );
}
const rgb = (c: [number, number, number]) => `#${c.map((v) => v.toString(16).padStart(2, '0')).join('')}`;

// ---- Model ----------------------------------------------------------------------------------------------------

function ModelChart({ r, from }: { r: Result; from: number }) {
  const v = useView(r.model_view, from);
  const m = useMemo(() => {
    const pred = v.get('pred'), sd = v.get('pred_sd'), z = v.get('z');
    return {
      lines: [
        { y: pred.map((p, i) => Math.exp(p - 1.96 * sd[i]!)), stroke: 'transparent' }, { y: pred.map((p, i) => Math.exp(p + 1.96 * sd[i]!)), stroke: 'transparent' },
        { y: pred.map(Math.exp), stroke: PAL.violet, width: 1, dash: [3, 2] }, { y: v.get('level').map(Math.exp), stroke: PAL.text, width: 2 },
      ],
      rmssd: v.get('rmssd'), z, pred, load: v.get('load'),
    };
  }, [v]);
  const dots = useMemo(() => ({ y: m.rmssd, color: (i: number) => (Math.abs(m.z[i]!) > 2 ? PAL.alert : PAL.hrv), r: (i: number) => (Math.abs(m.z[i]!) > 2 ? 4.2 : 3) }), [m]);
  const bands = useMemo(() => [{ a: 0, b: 1, fill: `${PAL.violet}2e` }], []);
  if (!v.days.length || r.model.nights < 20) return <ChartCard title="What each night should have been" method="hrv.state_space" job={{ kind: 'hrv4t' }}><Missing needs="at least 20 nights of HRV" /></ChartCard>;
  return (
    <ChartCard title="What each night should have been" method="hrv.state_space" job={{ kind: 'hrv4t' }}
      subtitle="White: the level; violet: prediction with yesterday's load and its 95 % band; red: nights more than 2 SD off">
      <TimeChart label="Model expected HRV with 95 % band" days={v.days} lines={m.lines} bands={bands} dots={dots} log height={320} yTitle="rMSSD (ms, log scale)"
        readout={(i) => `${isoDay(v.days[i]!)}  rMSSD ${finite(m.rmssd[i]) ? m.rmssd[i]!.toFixed(0) : '-'}  expected ${finite(m.pred[i]) ? Math.exp(m.pred[i]!).toFixed(0) : '-'}  surprise ${finite(m.z[i]) ? (m.z[i]! >= 0 ? '+' : '') + m.z[i]!.toFixed(2) : '-'}  load ${finite(m.load[i]) ? m.load[i]!.toFixed(0) : '-'}`} />
    </ChartCard>
  );
}
