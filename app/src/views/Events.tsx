import { useMemo } from 'react';
import { LABEL_KINDS, kindInfo, type Label, type LabelKind } from '@wl/schema';
import { PAL, diverging, isoDay, signed } from '@wl/views';
import { useApp } from '../lib/app';
import { useJob } from '../lib/useJob';
import { navigate, useNumParam, useParam } from '../lib/router';
import { LineChart, type BandSpec, type LineSpec } from '../charts/analysis/lines';
import { Heatgrid } from '../charts/analysis/heat';
import { Card, Desktop, Intro, JobNote, NotAvailable } from '../charts/analysis/frame';
import { METRICS, metricLabel, missingInput } from '../charts/analysis/inputs';

const DEFAULT_METRICS = ['hr_min30', 'ln_rmssd', 'resp_mean', 'sleep_stress'];
const COLORS = [PAL.hr, PAL.hrv, PAL.base, PAL.violet, PAL.alert, '#e8c36a'];
const rel = (v: number) => (v > 0 ? `+${v}` : `${v}`);
const finite = (v: number | null | undefined): v is number => v !== null && v !== undefined && Number.isFinite(v);

interface MetricStudy { metric: string; rel: number[]; event: (number | null)[]; control: (number | null)[]; diff: (number | null)[]; lo: (number | null)[]; hi: (number | null)[]; n: number[]; std_effect: (number | null)[]; sd: number; episodes: [string, (number | null)[]][] }
interface Study { episodes: number; too_few: boolean; controls_found: number; window: [number, number]; metrics: MetricStudy[] }

function MetricCard({ ms, study, color, labels, job }: { ms: MetricStudy; study: Study; color: string; labels: Label[]; job: unknown }) {
  const x = ms.rel;
  const showEach = study.episodes <= 8;
  const eventLines = useMemo<LineSpec[]>(() => [
    ...(showEach ? ms.episodes.map(([id, v]) => ({ label: id.slice(0, 6), y: v, color, faint: true })) : []),
    { label: 'event', y: ms.event, color, points: true },
    { label: 'control', y: ms.control, color: PAL.muted, dash: [6, 4] },
  ], [ms, color, showEach]);
  const diffLines = useMemo<LineSpec[]>(() => [{ label: 'difference', y: ms.diff, color, points: true }], [ms, color]);
  const band = useMemo<BandSpec>(() => ({ lo: ms.lo, hi: ms.hi, color, gradient: true }), [ms, color]);
  const nLines = useMemo<LineSpec[]>(() => [{ label: 'episodes', y: ms.n, color: PAL.base }], [ms]);
  const sync = `ev-${ms.metric}`;
  // the per-episode raster: each episode's change from its own pre-event mean, in the metric's spread
  const grid = useMemo(() => {
    const pre = x.map((r, i) => (r < 0 ? i : -1)).filter((i) => i >= 0);
    return ms.episodes.map(([, v]) => {
      const base = pre.map((i) => v[i]).filter(finite);
      const all = v.filter(finite);
      const m = base.length ? base.reduce((a, b) => a + b, 0) / base.length : all.length ? all.reduce((a, b) => a + b, 0) / all.length : NaN;
      return v.map((y) => (finite(y) && Number.isFinite(m) && ms.sd > 0 ? (y - m) / ms.sd : null));
    });
  }, [ms, x]);
  const names = ms.episodes.map(([id]) => {
    const l = labels.find((q) => q.id === id);
    return l ? isoDay(l.onset_day ?? l.start_day) : id.slice(0, 8);
  });
  return (
    <Card title={metricLabel(ms.metric)} method="events.study" job={job} testid={`event-${ms.metric}`}>
      <div className="grid2" style={{ gridTemplateColumns: 'repeat(auto-fit, minmax(380px, 1fr))' }}>
        <div>
          <h3>Event mean and control mean</h3>
          <LineChart x={x} lines={eventLines} vline={0} xFmt={rel} sync={sync} ariaLabel={`${metricLabel(ms.metric)}: event mean (solid) and control mean (dashed) by night relative to onset`} />
        </div>
        <div>
          <h3>Difference with 95% interval</h3>
          <LineChart x={x} lines={diffLines} band={band} hline={0} vline={0} xFmt={rel} sync={sync} ariaLabel={`${metricLabel(ms.metric)}: event minus control with a bootstrap interval`} testid="chart-diff" />
        </div>
      </div>
      <h3>Episodes per night</h3>
      <LineChart x={x} lines={nLines} bars xFmt={rel} sync={sync} height={64} ariaLabel="Number of episodes with a value on each night" testid="chart-n" />
      <h3>Each episode, change from its own pre-event level (SD)</h3>
      <Heatgrid
        rows={names} cols={x.length} cw={Math.max(14, Math.min(34, Math.floor(640 / x.length)))} ch={14}
        colTicks={x.map((r, i) => ({ r, i })).filter(({ r }) => r % 2 === 0).map(({ r, i }) => ({ i, text: rel(r) }))}
        value={(r, c) => grid[r]?.[c] ?? null}
        color={(v) => diverging(v)}
        tip={(r, c, v) => `${names[r]}, night ${rel(x[c]!)}: ${v === null ? 'no data' : `${signed(v, 1)} SD`}`}
        onCell={(r) => { const l = labels.find((q) => q.id === ms.episodes[r]?.[0]); if (l) navigate(`/night/${l.onset_day ?? l.start_day}`); }}
        ariaLabel={`Episode raster for ${metricLabel(ms.metric)}`} testid="episode-raster"
      />
    </Card>
  );
}

function Body() {
  const app = useApp();
  const { state, nights } = app;
  const live = useMemo(() => state.labels.filter((l) => !l.deleted_at), [state.labels]);
  const counts = useMemo(() => {
    const m = new Map<LabelKind, number>();
    for (const l of live) m.set(l.kind, (m.get(l.kind) ?? 0) + 1);
    return m;
  }, [live]);
  const best = (counts.get('illness') ? 'illness' : [...counts.entries()].sort((a, b) => b[1] - a[1])[0]?.[0] ?? 'illness') as LabelKind;
  const [kindP, setKind] = useParam('kind', '');
  const kind = (LABEL_KINDS.some((k) => k.kind === kindP) ? kindP : best) as LabelKind;
  const [mP, setM] = useParam('m', DEFAULT_METRICS.join(','));
  const [idsP, setIds] = useParam('ids', '');
  const [pre, setPre] = useNumParam('pre', 7);
  const [post, setPost] = useNumParam('post', 14);
  const [sameWd, setSameWd] = useParam('wd', '1');
  const [sameLoad, setSameLoad] = useParam('load', '1');
  const metrics = useMemo(() => mP.split(',').filter((m) => METRICS.some((x) => x.id === m)).slice(0, 6), [mP]);
  const ids = useMemo(() => idsP.split(',').filter(Boolean), [idsP]);
  const ofKind = useMemo(() => live.filter((l) => l.kind === kind).sort((a, b) => a.start_day - b.start_day), [live, kind]);
  const usable = metrics.filter((m) => !missingInput(nights, m));
  const missing = metrics.filter((m) => missingInput(nights, m));
  const job = useMemo(
    () => (nights && nights.n > 30 && usable.length && ofKind.length
      ? { kind: 'event_study' as const, label_kind: kind, ...(ids.length ? { label_ids: ids } : {}), metrics: usable, window: [-Math.max(0, pre), Math.max(1, post)] as [number, number], controls: { same_weekday: sameWd === '1', same_load_class: sameLoad === '1' }, n_boot: 400 }
      : null),
    [nights, usable.join(','), ofKind.length, kind, ids.join(','), pre, post, sameWd, sameLoad], // eslint-disable-line react-hooks/exhaustive-deps
  );
  const { data, loading, error } = useJob(job);
  const study = data as unknown as Study | null;
  const toggle = (m: string) => setM((metrics.includes(m) ? metrics.filter((x) => x !== m) : [...metrics, m].slice(0, 6)).join(',') || null);

  return (
    <div className="page">
      <h1>Events</h1>
      <Intro>How your nights move around an event, against matched nights of your own with no event nearby (same weekday, same season, same training load the day before). The interval is a bootstrap over events.</Intro>
      <div className="card">
        <div className="row" style={{ marginBottom: 8 }}>
          <label>Kind <select value={kind} onChange={(e) => { setKind(e.target.value); setIds(null); }} aria-label="Label kind">
            {LABEL_KINDS.map((k) => <option key={k.kind} value={k.kind}>{k.name} ({counts.get(k.kind) ?? 0})</option>)}
          </select></label>
          <label>Before <input type="number" min={0} max={30} value={pre} onChange={(e) => setPre(Math.min(30, Math.max(0, +e.target.value || 0)))} style={{ width: 60 }} aria-label="Nights before" /></label>
          <label>After <input type="number" min={1} max={60} value={post} onChange={(e) => setPost(Math.min(60, Math.max(1, +e.target.value || 1)))} style={{ width: 60 }} aria-label="Nights after" /></label>
          <label><input type="checkbox" checked={sameWd === '1'} onChange={(e) => setSameWd(e.target.checked ? '1' : '0')} /> same weekday</label>
          <label><input type="checkbox" checked={sameLoad === '1'} onChange={(e) => setSameLoad(e.target.checked ? '1' : '0')} /> same load the day before</label>
        </div>
        <div className="row" role="group" aria-label="Metrics" style={{ gap: 6 }}>
          {METRICS.map((m) => {
            const on = metrics.includes(m.id);
            return <button key={m.id} className={on ? 'primary' : ''} aria-pressed={on} onClick={() => toggle(m.id)} disabled={!on && metrics.length >= 6} style={{ fontSize: 12, padding: '2px 8px' }}>{m.label}</button>;
          })}
        </div>
        <p className="muted" style={{ fontSize: 12, margin: '6px 0 0' }}>Up to 6 metrics.</p>
        {ofKind.length > 0 && (
          <details style={{ marginTop: 6 }}>
            <summary>{ids.length ? `${ids.length} of ${ofKind.length} episodes chosen` : `All ${ofKind.length} episodes`}</summary>
            <div className="row" style={{ gap: 12, marginTop: 6 }}>
              {ofKind.map((l) => (
                <label key={l.id} className="mono" style={{ fontSize: 12 }}>
                  <input type="checkbox" checked={ids.includes(l.id)} onChange={(e) => setIds((e.target.checked ? [...ids, l.id] : ids.filter((i) => i !== l.id)).join(',') || null)} /> {isoDay(l.start_day)}
                </label>
              ))}
              {ids.length > 0 && <button onClick={() => setIds(null)}>All</button>}
            </div>
          </details>
        )}
      </div>
      {!nights || nights.n === 0 ? <div className="empty">No nights loaded yet.</div>
        : ofKind.length === 0 ? <div className="empty" data-testid="no-events">No {kindInfo(kind).name.toLowerCase()} labels yet. Label some nights in the Inbox, or pick another kind.</div>
        : (
          <>
            <JobNote loading={loading} error={error} />
            {missing.map((m) => <Card key={m} title={metricLabel(m)} method="events.study"><NotAvailable input={missingInput(nights, m)!} /></Card>)}
            {study && study.too_few && <div className="card" role="status" data-testid="too-few" style={{ borderColor: PAL.hr }}>Too few events ({study.episodes}) for a reliable average; showing each episode.</div>}
            {study && !study.too_few && study.controls_found === 0 && <div className="card" role="status" style={{ borderColor: PAL.hr }}>No matched control nights were found; relax the weekday or load matching to see the difference.</div>}
            {study && <p className="muted mono" style={{ fontSize: 12 }} data-testid="event-summary">{study.episodes} episodes, {study.controls_found} control nights, nights {rel(study.window[0])} to {rel(study.window[1])} around onset.</p>}
            {study?.metrics.map((ms) => <MetricCard key={ms.metric} ms={ms} study={study} color={COLORS[Math.max(0, usable.indexOf(ms.metric)) % COLORS.length]!} labels={ofKind} job={job} />)}
          </>
        )}
    </div>
  );
}

export default function Events() {
  return <Desktop><Body /></Desktop>;
}
