import { useMemo } from 'react';
import type { Table } from '@wl/schema';
import { PAL, clock, num } from '@wl/views';
import { useApp } from '../lib/app';
import { useJob } from '../lib/useJob';
import { col } from '../lib/util';
import { useNumParam, useParam } from '../lib/router';
import { Empty } from '../charts/small';
import { ChartCard, Experimental } from '../charts/wellness/ChartCard';
import { DesktopOnly } from '../charts/wellness/DesktopOnly';
import { Actogram } from '../charts/wellness/actogram';
import { TimeSeries, type TsSeries } from '../charts/wellness/ts';
import { densify, isNum, rolling } from '../charts/wellness/stats';
import { useInputs } from '../charts/wellness/inputs';

const hm = (v: number) => clock(v * 60);
const TEAL = PAL.hrv, AMBER = PAL.hr, BLUE = PAL.base, RED = PAL.alert, VIOLET = PAL.violet;

type Cols = Record<string, number[]>;
interface Circ { nonparametric: Table; cosinor: Table; sri: Table; chronotype: Table; sleep_debt: Table; hr_rhythm: Table; dlmo: Table }

function CircadianView() {
  const app = useApp();
  const { nights, range } = app;
  const [from, to] = range;
  const inputs = useInputs();
  const [period, setPeriod] = useNumParam('period', 24);
  const [overlay, setOverlay] = useParam('sleep', '1');
  const p = Math.min(24.5, Math.max(23.5, Math.round(period * 10) / 10));
  const have = !!nights && nights.n > 0;

  const act = useJob(have ? { kind: 'actogram', from, to, period_h: p } : null);
  const circ = useJob(have ? { kind: 'circadian', from, to } : null);
  const c = (circ.data ?? null) as unknown as Circ | null;

  /** a table's columns as a dense daily grid over the range (gaps stay gaps) */
  const dense = useMemo(() => (t: Table | undefined, names: string[]): { x: number[]; v: Cols } | null => {
    if (!t || !t.n) return null;
    const d = densify(col(t, 'day'), Object.fromEntries(names.map((n) => [n, t.num[n] ?? []])), from, to);
    return d.x.length ? { x: d.x, v: d.cols } : null;
  }, [from, to]);
  /** the same, but only the rows the table has (for weekly or three-daily fits) */
  const sparse = useMemo(() => (t: Table | undefined, names: string[]): { x: number[]; v: Cols } | null => {
    if (!t || !t.n) return null;
    const day = col(t, 'day');
    const keep = day.map((d, i) => (d >= from && d <= to ? i : -1)).filter((i) => i >= 0);
    if (!keep.length) return null;
    return { x: keep.map((i) => day[i]!), v: Object.fromEntries(names.map((n) => { const a = col(t, n); return [n, keep.map((i) => a[i]!)]; })) };
  }, [from, to]);

  if (!have) return <div className="page"><h1>Circadian</h1><Empty>No data yet. Import or load the demo from the Data page.</Empty></div>;

  const gridNeeds = (...names: string[]) => (inputs.ready ? (inputs.missing(...names) ?? null) : null);
  const noRows = (t: unknown, ...names: string[]): string | null => (t ? (gridNeeds(...names) ?? 'at least 14 days with data in this range') : null);

  const np = dense(c?.nonparametric, ['is', 'iv', 'ra', 'l5', 'm10', 'l5_onset', 'm10_onset']);
  const cos = dense(c?.cosinor, ['act_amp', 'act_acro', 'hr_amp', 'hr_acro']);
  const sri = dense(c?.sri, ['sri']);
  const chr = dense(c?.chronotype, ['sjl', 'sjl_sc', 'msf_sc', 'msw', 'msf', 'cpd']);
  const debt = dense(c?.sleep_debt, ['debt', 'debt14', 'need']);
  const hr = sparse(c?.hr_rhythm, ['peak', 'trough', 'amp', 'per_log_step', 'sleep_effect']);
  const dl = dense(c?.dlmo, ['dlmo']);
  const last = (d: { v: Cols } | null, k: string) => { const a = d?.v[k]?.filter(isNum) ?? []; return a.length ? a[a.length - 1]! : NaN; };

  const dots = (label: string, v: number[], color: string): TsSeries => ({ label, values: v, color, mode: 'dots', size: 4 });
  const line = (label: string, v: number[], color: string, dash?: number[]): TsSeries => ({ label, values: v, color, width: 1.8, dash });
  const loading = circ.loading && !c;
  const err = circ.error;

  const need = {
    np: c ? (np ? null : noRows(c.nonparametric, 'steps', 'hr')) : null,
    cos: c ? (cos ? null : noRows(c.cosinor, 'steps', 'hr')) : null,
    sri: c ? (sri ? null : noRows(c.sri)) : null,
    chr: c ? (chr ? null : 'at least 5 workday and 2 free-day nights in a 4-week window') : null,
    debt: c ? (debt ? null : 'at least 14 nights with sleep data') : null,
    hr: c ? (hr ? null : noRows(c.hr_rhythm, 'hr', 'steps')) : null,
    dl: c ? (dl ? null : noRows(c.dlmo, 'steps')) : null,
  };

  const meta = act.data as unknown as { width: number; height: number; days: number[]; period_h: number } | null;
  const rgba = act.data?.arrays.rgba as Uint8Array | undefined;
  const asleep = act.data?.arrays.asleep as Uint8Array | undefined;
  const actNeeds = inputs.ready && inputs.missing('steps') ? 'steps' : !meta || meta.height === 0 ? (act.data ? 'at least 2 days with activity data in this range' : null) : null;

  const stat = (label: string, v: string) => <div key={label}><div className="muted" style={{ fontSize: 11 }}>{label}</div><div className="mono" style={{ fontSize: 16 }}>{v}</div></div>;
  const lastOnset = last(np, 'l5_onset'), lastM10 = last(np, 'm10_onset');

  return (
    <div className="page">
      <div className="row" style={{ justifyContent: 'space-between' }}>
        <h1>Circadian</h1>
      </div>
      <p className="muted">How regular and well-timed your days are, from steps, heart rate and sleep. Rolling measures use the latest 14 days (7 for the fits) ending on each date.</p>

      <div className="card" style={{ display: 'flex', gap: 28, flexWrap: 'wrap' }} data-testid="circadian-summary">
        {stat('Interdaily stability', num(last(np, 'is'), 2))}
        {stat('Intradaily variability', num(last(np, 'iv'), 2))}
        {stat('Relative amplitude', num(last(np, 'ra'), 2))}
        {stat('L5 starts', Number.isFinite(lastOnset) ? hm(lastOnset) : '\u2013')}
        {stat('M10 starts', Number.isFinite(lastM10) ? hm(lastM10) : '\u2013')}
        {stat('Sleep regularity', num(last(sri, 'sri'), 0))}
        {stat('Social jet lag', Number.isFinite(last(chr, 'sjl')) ? `${last(chr, 'sjl').toFixed(1)} h` : '\u2013')}
        {stat('Sleep debt (14 nights)', Number.isFinite(last(debt, 'debt14')) ? `${last(debt, 'debt14').toFixed(1)} h` : '\u2013')}
      </div>

      <ChartCard
        title="Actogram"
        sub={`Each row is two ${p.toFixed(1)}-hour periods side by side, so a rhythm that drifts reads as a slanted band. Bright: more steps. Blue tint: asleep.`}
        method="circadian.nonparametric"
        job={{ kind: 'actogram', from, to, period_h: p }}
        needs={actNeeds}
        loading={act.loading && !act.data}
        error={act.error}
        actions={
          <>
            <label className="row" style={{ gap: 6 }}>Period <input type="range" min={23.5} max={24.5} step={0.1} value={p} onChange={(e) => setPeriod(Math.round(+e.target.value * 10) / 10)} aria-label="Actogram period in hours" /><span className="mono">{p.toFixed(1)} h</span></label>
            <label className="row" style={{ gap: 4 }}><input type="checkbox" checked={overlay !== '0'} onChange={(e) => setOverlay(e.target.checked ? '1' : '0')} /> Sleep</label>
          </>
        }
      >
        {meta && rgba && meta.height > 0 && <Actogram rgba={rgba} asleep={asleep} width={meta.width} height={meta.height} days={meta.days} periodH={p} overlay={overlay !== '0'} />}
      </ChartCard>

      <h2 style={{ marginTop: 18 }}>Rest and activity</h2>
      <div className="grid2">
        <ChartCard title="Stability and amplitude" sub="IS: how alike the days are (1 is identical). RA: contrast between the busiest 10 hours and quietest 5." method="circadian.nonparametric" job={{ kind: 'circadian', from, to }} needs={need.np} loading={loading} error={err}>
          {np && <TimeSeries x={np.x} series={[line('IS', np.v.is!, TEAL), line('RA', np.v.ra!, AMBER)]} height={170} yRange={[0, 1]} sync="np" yFmt={(v) => v.toFixed(1)} />}
        </ChartCard>
        <ChartCard title="Fragmentation" sub="IV: how much activity jumps hour to hour (higher is more fragmented)" method="circadian.nonparametric" job={{ kind: 'circadian', from, to }} needs={need.np} loading={loading} error={err}>
          {np && <TimeSeries x={np.x} series={[line('IV', np.v.iv!, VIOLET)]} height={170} sync="np" yFmt={(v) => v.toFixed(2)} />}
        </ChartCard>
        <ChartCard title="L5 and M10 level" sub="Steps per hour in the quietest 5 hours (L5) and busiest 10 hours (M10) of the average day" method="circadian.nonparametric" job={{ kind: 'circadian', from, to }} needs={need.np} loading={loading} error={err}>
          {np && <TimeSeries x={np.x} series={[line('M10', np.v.m10!, AMBER), line('L5', np.v.l5!, BLUE)]} height={170} sync="np" yFmt={(v) => v.toFixed(0)} />}
        </ChartCard>
        <ChartCard title="L5 and M10 clock time" sub="When those windows start" method="circadian.nonparametric" job={{ kind: 'circadian', from, to }} needs={need.np} loading={loading} error={err}>
          {np && <TimeSeries x={np.x} series={[dots('M10 starts', np.v.m10_onset!, AMBER), dots('L5 starts', np.v.l5_onset!, BLUE)]} height={170} yRange={[0, 24]} ySplit={4} sync="np" yFmt={hm} />}
        </ChartCard>
      </div>

      <h2 style={{ marginTop: 18 }}>Sleep regularity and timing</h2>
      <div className="grid2">
        <ChartCard title="Sleep Regularity Index" sub="The chance of being in the same sleep state at the same clock time on consecutive days, over 14 days (100 is perfectly regular)" method="circadian.sri" job={{ kind: 'circadian', from, to }} needs={need.sri} loading={loading} error={err}>
          {sri && <TimeSeries x={sri.x} series={[line('SRI', sri.v.sri!, TEAL)]} height={170} yRange={[0, 100]} yFmt={(v) => v.toFixed(0)} />}
        </ChartCard>
        <ChartCard title="Social jet lag" sub="Midsleep on free days minus workdays, over a rolling 4 weeks; MSFsc is the free-day midpoint corrected for sleeping in" method="circadian.chronotype" job={{ kind: 'circadian', from, to }} needs={need.chr} loading={loading} error={err}>
          {chr && (
            <>
              <TimeSeries x={chr.x} series={[line('Social jet lag', chr.v.sjl!, AMBER), line('Corrected (sleep-debt adjusted)', chr.v.sjl_sc!, BLUE, [4, 3])]} height={110} sync="chr" yFmt={(v) => `${v.toFixed(1)} h`} />
              <TimeSeries x={chr.x} series={[line('MSFsc', chr.v.msf_sc!, VIOLET)]} height={110} sync="chr" yFmt={hm} />
            </>
          )}
        </ChartCard>
        <ChartCard title="Composite phase deviation" sub="How far each night's midpoint sat from your last four weeks, combined with the change since the night before (hours)" method="circadian.chronotype" job={{ kind: 'circadian', from, to }} needs={need.chr} loading={loading} error={err}>
          {chr && <TimeSeries x={chr.x} series={[dots('CPD', chr.v.cpd!, RED), line('7-day mean', rolling(chr.v.cpd!, 7, 3), TEAL)]} height={170} yRange={[0, null]} yFmt={(v) => v.toFixed(1)} />}
        </ChartCard>
        <ChartCard title="Sleep debt" sub="Hours short of your sleep need: weighted toward recent nights (top) and the plain sum of the last 14 nights (bottom)" method="circadian.sleep_debt" job={{ kind: 'circadian', from, to }} needs={need.debt} loading={loading} error={err}>
          {debt && (
            <>
              <TimeSeries x={debt.x} series={[line('h per night', debt.v.debt!, AMBER)]} height={110} sync="debt" yFmt={(v) => v.toFixed(1)} />
              <TimeSeries x={debt.x} series={[line('14-night sum (h)', debt.v.debt14!, RED)]} height={110} sync="debt" yFmt={(v) => v.toFixed(0)} />
            </>
          )}
        </ChartCard>
      </div>

      <h2 style={{ marginTop: 18 }}>Daily rhythm</h2>
      <div className="grid2">
        <ChartCard title="Cosinor acrophase" sub="The clock hour a 24-hour cosine fitted to the last 7 days peaks, for activity and heart rate" method="circadian.cosinor" job={{ kind: 'circadian', from, to }} needs={need.cos} loading={loading} error={err}>
          {cos && <TimeSeries x={cos.x} series={[dots('Activity peak', cos.v.act_acro!, AMBER), dots('Heart rate peak', cos.v.hr_acro!, RED)]} height={170} yRange={[0, 24]} ySplit={4} sync="cos" yFmt={hm} />}
        </ChartCard>
        <ChartCard title="Cosinor amplitude" sub="Half the swing of the fitted cosine: steps per hour (top), beats per minute (bottom)" method="circadian.cosinor" job={{ kind: 'circadian', from, to }} needs={need.cos} loading={loading} error={err}>
          {cos && (
            <>
              <TimeSeries x={cos.x} series={[line('Activity', cos.v.act_amp!, AMBER)]} height={110} sync="cos" yFmt={(v) => v.toFixed(0)} />
              <TimeSeries x={cos.x} series={[line('Heart rate', cos.v.hr_amp!, RED)]} height={110} sync="cos" yFmt={(v) => v.toFixed(1)} />
            </>
          )}
        </ChartCard>
        <ChartCard title="Heart rate rhythm: peak and trough" sub="Weekly fit of a cosine plus the effects of movement and sleep; the clock hours it is highest and lowest" method="circadian.hr_rhythm" job={{ kind: 'circadian', from, to }} needs={need.hr} loading={loading} error={err}>
          {hr && <TimeSeries x={hr.x} series={[dots('Peak', hr.v.peak!, RED), dots('Trough', hr.v.trough!, BLUE)]} height={170} yRange={[0, 24]} ySplit={4} sync="hr" yFmt={hm} />}
        </ChartCard>
        <ChartCard title="Heart rate rhythm: size" sub="Amplitude of the daily cosine (bpm), and the effects of movement (bpm per log step) and of being asleep (bpm)" method="circadian.hr_rhythm" job={{ kind: 'circadian', from, to }} needs={need.hr} loading={loading} error={err}>
          {hr && (
            <>
              <TimeSeries x={hr.x} series={[line('Amplitude (bpm)', hr.v.amp!, RED)]} height={100} sync="hr" yFmt={(v) => v.toFixed(1)} />
              <TimeSeries x={hr.x} series={[line('bpm per log step', hr.v.per_log_step!, AMBER)]} height={100} sync="hr" yFmt={(v) => v.toFixed(1)} />
              <TimeSeries x={hr.x} series={[line('Sleep effect (bpm)', hr.v.sleep_effect!, BLUE)]} height={100} sync="hr" yFmt={(v) => v.toFixed(1)} />
            </>
          )}
        </ChartCard>
      </div>

      <ChartCard
        title="Estimated melatonin onset (Forger model)"
        sub="A model of the body clock driven by a light level guessed from your sleep and movement. It is an estimate of the clock hour the evening melatonin rise would begin, not a measurement."
        method="circadian.dlmo"
        job={{ kind: 'circadian', from, to }}
        needs={need.dl}
        loading={loading}
        error={err}
        badge={<Experimental />}
      >
        {dl && <TimeSeries x={dl.x} series={[dots('Modelled DLMO', dl.v.dlmo!, VIOLET), line('7-day mean', rolling(dl.v.dlmo!, 7, 3), TEAL)]} height={170} yRange={[0, 24]} ySplit={4} yFmt={hm} />}
      </ChartCard>
    </div>
  );
}

export default function Circadian() {
  return <DesktopOnly title="Circadian"><CircadianView /></DesktopOnly>;
}
