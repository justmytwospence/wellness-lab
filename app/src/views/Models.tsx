import { useMemo, type ReactNode } from 'react';
import { kindInfo } from '@wl/schema';
import { PAL, diverging, isoDay, signed } from '@wl/views';
import { useApp } from '../lib/app';
import { useJob } from '../lib/useJob';
import { navigate } from '../lib/router';
import { Fill, useSize } from '../ui/Fill';
import { InspectorNotes } from '../ui/Inspector';
import { View } from '../ui/View';
import { LineChart, type BandSpec, type LineSpec, type SpanSpec } from '../charts/analysis/lines';
import { HBars } from '../charts/analysis/dots';
import { Heatgrid } from '../charts/analysis/heat';
import { Card, Desktop, Intro, JobNote, NotAvailable } from '../charts/analysis/frame';
import { fmt, metricInput, metricLabel } from '../charts/analysis/inputs';

interface Fit { factor: (number | null)[]; factor_sd: (number | null)[]; loadings: number[]; noise_sd: number[]; phi: number; gamma: number[]; resid_z: (number | null)[][]; loglik: number; iterations: number }
interface Dfm { signals: string[]; inputs?: string[]; days?: number[]; fit?: Fit; error?: string }
const SIGNALS = ['hr_min30', 'ln_rmssd', 'resp_mean', 'sleep_stress', 'skin_temp', 'rest_frac'];
const dayText = (sec: number) => isoDay(Math.round(sec / 86400));

function Strain({ dfm, range }: { dfm: Dfm; range: [number, number] }) {
  const { state } = useApp();
  const fit = dfm.fit!, days = dfm.days!;
  const idx = useMemo(() => days.map((d, i) => (d >= range[0] && d <= range[1] ? i : -1)).filter((i) => i >= 0), [days, range]);
  const x = useMemo(() => idx.map((i) => days[i]! * 86400), [idx, days]);
  const lines = useMemo<LineSpec[]>(() => [{ label: 'strain', y: idx.map((i) => fit.factor[i] ?? null), color: PAL.alert }], [idx, fit]);
  const band = useMemo<BandSpec>(() => ({ lo: idx.map((i) => (fit.factor[i] ?? NaN) - (fit.factor_sd[i] ?? NaN)), hi: idx.map((i) => (fit.factor[i] ?? NaN) + (fit.factor_sd[i] ?? NaN)), color: PAL.alert }), [idx, fit]);
  const spans = useMemo<SpanSpec[]>(() => state.labels.filter((l) => !l.deleted_at && l.end_day >= range[0] && l.start_day <= range[1]).map((l) => ({ a: l.start_day * 86400, b: (l.end_day + 1) * 86400, color: kindInfo(l.kind).color })), [state.labels, range]);
  return <Fill>{({ h }) => <LineChart x={x} lines={lines} band={band} spans={spans} time hline={0} height={h} xFmt={dayText} yLabel="strain (SD)" ariaLabel="Latent strain factor with a one standard deviation band; labels along the bottom edge" testid="chart-strain" />}</Fill>;
}

/** Bars that take the width of their container, up to their natural 440. */
function Bars({ children }: { children(width: number): ReactNode }) {
  const [ref, { w }] = useSize<HTMLDivElement>();
  return <div ref={ref} style={{ width: '100%' }}>{w > 0 && children(Math.min(440, w))}</div>;
}

/** The container's full width, for content that spreads over it. */
function Wide({ children }: { children(width: number): ReactNode }) {
  const [ref, { w }] = useSize<HTMLDivElement>();
  return <div ref={ref} style={{ width: '100%' }}>{w > 0 && children(w)}</div>;
}

function Residuals({ dfm, range, width }: { dfm: Dfm; range: [number, number]; width: number }) {
  const { state } = useApp();
  const fit = dfm.fit!, days = dfm.days!;
  const idx = useMemo(() => days.map((d, i) => (d >= range[0] && d <= range[1] ? i : -1)).filter((i) => i >= 0), [days, range]);
  const win = state.settings.baseline_window;
  // a night is faded where fewer than 14 nights with a value came before it in the baseline window
  const thin = useMemo(() => dfm.signals.map((_, s) => idx.map((i) => { let n = 0; for (let k = Math.max(0, i - win); k < i; k++) if (fit.resid_z[s]?.[k] != null && Number.isFinite(fit.resid_z[s]![k]!)) n++; return n < 14; })), [dfm.signals, idx, fit, win]);
  const ticks = useMemo(() => idx.map((i, c) => ({ i: c, d: isoDay(days[i]!) })).filter((t, c, a) => c === 0 || t.d.slice(0, 7) !== a[c - 1]!.d.slice(0, 7)).map((t) => ({ i: t.i, text: t.d.slice(0, 7) })), [idx, days]);
  // the cells share the panel's width (fractional widths are fine in SVG)
  const cw = Math.max(1, Math.min(14, (width - 190 - 4) / Math.max(1, idx.length)));
  return (
    <Heatgrid
      rows={dfm.signals.map(metricLabel)} cols={idx.length} cw={cw} ch={22} rowLabelW={190}
      colTicks={ticks.filter((_, k) => k % (cw * 30 < 60 ? 3 : 1) === 0)}
      value={(r, c) => fit.resid_z[r]?.[idx[c]!] ?? null}
      color={(v) => diverging(v)}
      alpha={(r, c) => (thin[r]?.[c] ? 0.35 : 1)}
      tip={(r, c, v) => `${metricLabel(dfm.signals[r]!)}, ${isoDay(days[idx[c]!]!)}: ${v === null ? 'no data' : `${signed(v, 1)} SD`}${thin[r]?.[c] ? ' (short baseline)' : ''}`}
      onCell={(_, c) => navigate(`/night/${days[idx[c]!]}`)}
      ariaLabel="Residual z-scores by signal and night" testid="residual-heatmap"
    />
  );
}

function Body() {
  const { nights, state, range } = useApp();
  const ok = !!nights && nights.n > 30;
  const dfmJob = useMemo(() => (ok ? { kind: 'dfm' as const } : null), [ok]);
  const { data, loading, error } = useJob(dfmJob);
  const hrvJob = useMemo(() => (ok ? { kind: 'hrv4t' as const, range_days: state.settings.hrv_range_days, k_daily: state.settings.k_daily, k_swc: state.settings.k_swc } : null), [ok, state.settings.hrv_range_days, state.settings.k_daily, state.settings.k_swc]);
  const hrv = useJob(hrvJob);
  const dfm = data as unknown as Dfm | null;
  const m = (hrv.data as unknown as { model?: Record<string, number | string> } | null)?.model;
  const missing = SIGNALS.filter((s) => dfm && !dfm.signals.includes(s));
  const lr = Number(m?.lr);
  const hrvCard = (
    <Card title="HRV and training (state-space model)" method="hrv.state_space" job={hrvJob} testid="card-hrv-model">
      {m ? (
        <>
          <table className="t" style={{ maxWidth: 560 }}>
            <tbody>
              <tr><td>Training response (beta)</td><td className="n" data-testid="model-pct">{signed(Number(m.pct_per_100), 1)}% HRV per 100 load</td></tr>
              <tr><td>Half-life of the response</td><td className="n">{fmt(Number(m.half_life_days), 1)} days</td></tr>
              <tr><td>Likelihood-ratio test against no response</td><td className="n">LR = {fmt(lr, 1)} on 2 df, p = {Number.isFinite(lr) ? (Math.exp(-Math.max(0, lr) / 2) < 0.001 ? '< 0.001' : Math.exp(-Math.max(0, lr) / 2).toFixed(3)) : '\u2013'}</td></tr>
              <tr><td>Noise and level drift (sd, ln RMSSD)</td><td className="n">{fmt(Number(m.sigma_eps), 3)} / {fmt(Number(m.sigma_eta), 3)}</td></tr>
              <tr><td>Nights used</td><td className="n">{String(m.nights)}</td></tr>
            </tbody>
          </table>
          <p className="muted" style={{ fontSize: 12 }}>A negative response means hard training lowers the next nights' HRV and fades with the half-life above.</p>
        </>
      ) : hrv.loading ? <p className="muted">Fitting...</p> : <div className="empty">Not enough HRV nights to fit the model.</div>}
    </Card>
  );
  // the right column scrolls as a whole; each card keeps its natural height
  const cell = (key: string, card: ReactNode) => <div key={key} style={{ flex: 'none', background: 'var(--bg)' }}>{card}</div>;

  const fit = dfm?.fit && dfm.days ? { dfm, fit: dfm.fit, days: dfm.days } : null;
  const left = fit ? (
    <div className="vgrid" style={{ gridTemplateRows: 'minmax(0,1.1fr) minmax(0,1fr)' }}>
      <Card title="Strain factor" method="model.strain" job={dfmJob} testid="card-strain">
        <InspectorNotes><h2>Strain factor</h2><p>High means your nocturnal heart rate, breathing and stress are up and HRV is down together, beyond what your own baseline explains. Coloured marks along the bottom edge are your labels.</p></InspectorNotes>
        <Strain dfm={fit.dfm} range={range} />
        {missing.length > 0 && <p className="muted" style={{ fontSize: 12, margin: '4px 0 0' }}>Left out: {missing.map((s) => <span key={s}>{metricLabel(s)} (<NotAvailable inline input={metricInput(s)} />) </span>)}</p>}
      </Card>
      <Card title="Residuals: what the factor does not explain" method="model.strain" job={dfmJob} testid="card-residuals" scroll>
        <p className="muted" style={{ fontSize: 12 }}>Each cell is the night's value minus loading times factor, in units of that signal's noise. Faded cells have fewer than 14 baseline nights behind them.</p>
        <Wide>{(w) => <Residuals dfm={fit.dfm} range={range} width={w} />}</Wide>
      </Card>
    </div>
  ) : dfm?.error ? (
    <Card title="Strain factor" method="model.strain" job={dfmJob}><div className="empty" data-testid="dfm-error">Not enough nights to fit the model ({dfm.error}).</div></Card>
  ) : (
    <div className="panel">
      <JobNote loading={loading} error={error} />
      {!nights || nights.n === 0 ? <div className="empty">No nights loaded yet.</div> : null}
    </div>
  );

  return (
    <View title="Models" desc="One hidden strain factor behind your nights, and what drives it.">
      <Intro>Two models of how your body responds. The strain factor is one hidden number behind your overnight heart rate, HRV, breathing and stress, pushed by training, travel, alcohol and illness. The HRV model asks how much of tonight's HRV is the training you did.</Intro>
      <div className="vgrid" style={{ gridTemplateColumns: 'minmax(0,1fr) 380px' }}>
        {left}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 1, background: 'var(--line)', overflow: 'auto', minHeight: 0 }}>
          {fit && cell('loadings', (
            <Card title="Loadings" method="model.strain" job={dfmJob} testid="card-loadings">
              <p className="muted" style={{ fontSize: 12 }}>How strongly each signal follows the factor (positive: rises with strain).</p>
              <Bars>{(w) => <HBars items={fit.dfm.signals.map((s, i) => ({ label: metricLabel(s), value: fit.fit.loadings[i] ?? NaN }))} width={w} />}</Bars>
              <p className="mono muted" style={{ fontSize: 12 }}>phi = {fmt(fit.fit.phi)} (carry-over from one night to the next)</p>
            </Card>
          ))}
          {fit && cell('gamma', (
            <Card title="What pushes the factor" method="model.strain" job={dfmJob} testid="card-gamma">
              <p className="muted" style={{ fontSize: 12 }}>Effect of each input on the next morning's strain (gamma).</p>
              <Bars>{(w) => <HBars items={(fit.dfm.inputs ?? []).map((s, i) => ({ label: s, value: fit.fit.gamma[i] ?? NaN }))} width={w} color={PAL.hr} />}</Bars>
              <p className="mono muted" style={{ fontSize: 12 }}>{fit.days.length} nights, log-likelihood {fmt(fit.fit.loglik, 1)}, {fit.fit.iterations} iterations</p>
            </Card>
          ))}
          {cell('hrv', hrvCard)}
        </div>
      </div>
    </View>
  );
}

export default function Models() {
  return <Desktop><Body /></Desktop>;
}
