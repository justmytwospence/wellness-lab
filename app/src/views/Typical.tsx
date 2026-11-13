import { useEffect, useMemo, useRef } from 'react';
import { PAL, formatRange, parseRange, isoDay, weekday } from '@wl/views';
import { useApp } from '../lib/app';
import { useJob } from '../lib/useJob';
import { useParam } from '../lib/router';
import { Empty } from '../charts/small';
import { View, ViewEmpty } from '../ui/View';
import { Panel } from '../ui/Panel';
import { Fill } from '../ui/Fill';
import { InspectorNotes } from '../ui/Inspector';
import { DesktopOnly } from '../charts/wellness/DesktopOnly';
import { QuantileBands, SmallMultiples, type AlignMode, type Typ } from '../charts/wellness/quantiles';
import { useInputs } from '../charts/wellness/inputs';
import { useJobs } from '../charts/wellness/useJobs';

interface M { id: string; label: string; unit: string; digits: number; method: string }
const METRICS: M[] = [
  { id: 'hr', label: 'Heart rate', unit: ' bpm', digits: 0, method: 'night.hr' },
  { id: 'hrv5', label: 'HRV (5-minute RMSSD)', unit: ' ms', digits: 0, method: 'night.hrv' },
  { id: 'stress', label: 'Stress', unit: '', digits: 0, method: 'night.stress' },
  { id: 'resp', label: 'Breathing rate', unit: ' /min', digits: 1, method: 'night.resp' },
  { id: 'spo2', label: 'SpO2', unit: ' %', digits: 1, method: 'night.spo2' },
  { id: 'body_battery', label: 'Body Battery', unit: '', digits: 0, method: 'night.timing' },
  { id: 'steps', label: 'Steps per minute', unit: '', digits: 1, method: 'night.timing' },
  { id: 'motion', label: 'Motion', unit: '', digits: 2, method: 'night.timing' },
  { id: 'altitude', label: 'Altitude', unit: ' m', digits: 0, method: 'night.timing' },
];
const BIN = 10;
const MAX_WEEKS = 26;

/** Calendar weeks (Monday to Sunday) overlapping the period, clipped to it; the latest `MAX_WEEKS`. */
function weeksOf([a0, a1]: [number, number]): [number, number][] {
  const out: [number, number][] = [];
  for (let ws = a0 - weekday(a0); ws <= a1; ws += 7) out.push([Math.max(ws, a0), Math.min(ws + 6, a1)]);
  return out.slice(-MAX_WEEKS);
}

function TypicalView() {
  const app = useApp();
  const { nights, range, lastDay, firstDay } = app;
  const inputs = useInputs();
  const [metric, setMetric] = useParam('metric', 'hr');
  const [align, setAlign] = useParam('align', 'onset');
  const [aText, setAText] = useParam('pa', '');
  const [bText, setBText] = useParam('pb', '');
  const m = METRICS.find((x) => x.id === metric) ?? METRICS[0]!;
  const mode = (align === 'clock' ? 'clock' : 'onset') as AlignMode;
  const span = mode === 'clock' ? 16 * 60 : 12 * 60;

  // `a` toggles the alignment; ignore it as the second key of a `g a` go-to
  const lastG = useRef(0);
  useEffect(() => {
    const on = (e: KeyboardEvent) => {
      if (/^(INPUT|TEXTAREA|SELECT)$/.test((e.target as HTMLElement).tagName) || e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key === 'g') lastG.current = Date.now();
      else if (e.key === 'a' && Date.now() - lastG.current > 1300) setAlign(mode === 'onset' ? 'clock' : 'onset');
    };
    window.addEventListener('keydown', on);
    return () => window.removeEventListener('keydown', on);
  }, [mode, setAlign]);

  const a = useMemo(() => (aText ? parseRange(aText, lastDay, firstDay) : null) ?? range, [aText, lastDay, firstDay, range]);
  const aBad = !!aText && !parseRange(aText, lastDay, firstDay);
  const b = useMemo(() => (bText ? parseRange(bText, lastDay, firstDay) : null), [bText, lastDay, firstDay]);
  const bBad = !!bText && !b;

  const have = !!nights && nights.n > 0;
  const job = { kind: 'typical' as const, metric: m.id, align: mode, a, ...(b ? { b } : {}), bin_min: BIN };
  const { data, loading, error } = useJob(have ? job : null);
  const ta = data?.a as Typ | undefined;
  const tb = (data?.b ?? null) as Typ | null;

  const weeks = useMemo(() => weeksOf(a), [a]);
  const wk = useJobs(have && ta && ta.minute.length ? weeks.map(([f, t]) => ({ kind: 'typical' as const, metric: m.id, align: mode, a: [f, t] as [number, number], bin_min: BIN })) : null);
  const domain = useMemo<[number, number]>(() => {
    const v = wk.data.flatMap((d) => (d?.a ? [...(d.a as Typ).p25, ...(d.a as Typ).p75] : [])).filter(Number.isFinite) as number[];
    if (!v.length) return [0, 1];
    const lo = Math.min(...v), hi = Math.max(...v), pad = (hi - lo || 1) * 0.1;
    return [lo - pad, hi + pad];
  }, [wk.data]);

  if (!have) return <ViewEmpty title="Typical">No data yet. Import or load the demo from the Data page.</ViewEmpty>;
  const empty = !!ta && ta.minute.length === 0;
  const needs = empty && inputs.ready && !inputs.has(m.id) ? m.id : null;
  const fmt = (v: number) => v.toFixed(m.digits);
  const periodLabel = (r: [number, number]) => formatRange(r);

  const showWeeks = !!ta && ta.minute.length > 0;
  return (
    <View
      title="Typical"
      desc="The shape of your nights: median and spread for each 10-minute bin."
      tools={(
        <>
          <label>Signal <select value={metric} onChange={(e) => setMetric(e.target.value)} aria-label="Signal">{METRICS.map((x) => <option key={x.id} value={x.id}>{x.label}</option>)}</select></label>
          <label>Align <select value={mode} onChange={(e) => setAlign(e.target.value)} aria-label="Alignment"><option value="onset">on falling asleep</option><option value="clock">on the clock</option></select> <kbd>a</kbd></label>
          <label>Period A <input value={aText} placeholder={app.rangeText} onChange={(e) => setAText(e.target.value)} aria-label="Period A" style={{ width: 200, borderColor: aBad ? PAL.alert : undefined }} /></label>
          <label>Period B <input value={bText} placeholder="optional, e.g. 2024 or last 30 days" onChange={(e) => setBText(e.target.value)} aria-label="Period B" style={{ width: 240, borderColor: bBad ? PAL.alert : undefined }} /></label>
          <span className="muted" style={{ fontSize: 12 }}>
            <span style={{ color: PAL.hrv }}>A {periodLabel(a)}</span>{b && <span style={{ color: PAL.hr }}>, B {periodLabel(b)}</span>}
            {(aBad || bBad) && <span className="err"> (could not read the period; using the default)</span>}
          </span>
        </>
      )}
    >
      <InspectorNotes><h2>About this view</h2><p>The shape of a night: for each 10-minute bin, the middle half (dark band) and the middle 90% (light band) of your nights, around the median. Bins need at least 5 nights.</p></InspectorNotes>
      <div className="vgrid" style={{ gridTemplateRows: showWeeks ? 'minmax(0, 1.7fr) minmax(0, 1fr)' : 'minmax(0, 1fr)' }}>
        <Panel title={`Typical night: ${m.label}`} sub={mode === 'clock' ? 'Clock time, from 20:00' : 'Time since falling asleep'} method={m.method} job={job} needs={needs} loading={loading && !ta} error={error}>
          {ta && ta.minute.length > 0 && (
            <Fill>{({ h }) => <QuantileBands a={ta} b={tb && tb.minute.length ? tb : null} align={mode} bin={BIN} span={span} unit={m.unit} fmt={fmt} labelA="A" labelB="B" height={Math.max(120, h - 22)} />}</Fill>
          )}
          {ta && ta.minute.length === 0 && !needs && <Empty>Fewer than 5 nights in {periodLabel(a)} have this signal.</Empty>}
          {b && tb && tb.minute.length === 0 && <p className="muted">Period B has fewer than 5 nights with this signal.</p>}
        </Panel>
        {showWeeks && (
          <Panel scroll title="Week by week" sub={`The median curve of each calendar week in period A, on one scale${weeks.length === MAX_WEEKS ? ` (the latest ${MAX_WEEKS} weeks)` : ''}. Shaded: middle half of the week's nights.`} method={m.method} job={{ kind: 'typical', metric: m.id, align: mode, per: 'week' }} loading={wk.loading && !wk.data.length}>
            <SmallMultiples items={weeks.map(([f], i) => ({ label: isoDay(f), t: (wk.data[i]?.a ?? null) as Typ | null }))} domain={domain} span={span} bin={BIN} />
          </Panel>
        )}
      </div>
    </View>
  );
}

export default function Typical() {
  return <DesktopOnly title="Typical"><TypicalView /></DesktopOnly>;
}
