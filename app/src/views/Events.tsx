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
import { View } from '../ui/View';
import { Fill } from '../ui/Fill';
import { FocusList } from '../ui/FocusList';

const DEFAULT_METRIC = 'hr_min30';
const COLORS = [PAL.hr, PAL.hrv, PAL.base, PAL.violet, PAL.alert, '#e8c36a'];
const cell = { background: 'var(--bg)' } as const;
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
      <div className="vgrid" style={{ flex: '1 1 0', height: 'auto', gridTemplateRows: 'minmax(0, 1fr) auto minmax(0, 0.8fr)' }}>
        <div className="vgrid" style={{ height: 'auto', gridTemplateColumns: '1fr 1fr' }}>
          <div className="panel" style={{ ...cell, padding: '8px 12px 8px 0' }}>
            <h3>Event mean and control mean</h3>
            <Fill>{({ h }) => <LineChart x={x} lines={eventLines} vline={0} xFmt={rel} sync={sync} height={h} ariaLabel={`${metricLabel(ms.metric)}: event mean (solid) and control mean (dashed) by night relative to onset`} />}</Fill>
          </div>
          <div className="panel" style={{ ...cell, padding: '8px 0 8px 12px' }}>
            <h3>Difference with 95% interval</h3>
            <Fill>{({ h }) => <LineChart x={x} lines={diffLines} band={band} hline={0} vline={0} xFmt={rel} sync={sync} height={h} ariaLabel={`${metricLabel(ms.metric)}: event minus control with a bootstrap interval`} testid="chart-diff" />}</Fill>
          </div>
        </div>
        <div className="panel" style={{ ...cell, padding: '8px 0' }}>
          <h3>Episodes per night</h3>
          <LineChart x={x} lines={nLines} bars xFmt={rel} sync={sync} height={64} ariaLabel="Number of episodes with a value on each night" testid="chart-n" />
        </div>
        <div className="panel" style={{ ...cell, padding: '8px 0 0' }}>
          <h3>Each episode, change from its own pre-event level (SD)</h3>
          <div style={{ flex: '1 1 0', minHeight: 0, overflow: 'auto' }}>
            <Heatgrid
              rows={names} cols={x.length} cw={Math.max(14, Math.min(34, Math.floor(640 / x.length)))} ch={14}
              colTicks={x.map((r, i) => ({ r, i })).filter(({ r }) => r % 2 === 0).map(({ r, i }) => ({ i, text: rel(r) }))}
              value={(r, c) => grid[r]?.[c] ?? null}
              color={(v) => diverging(v)}
              tip={(r, c, v) => `${names[r]}, night ${rel(x[c]!)}: ${v === null ? 'no data' : `${signed(v, 1)} SD`}`}
              onCell={(r) => { const l = labels.find((q) => q.id === ms.episodes[r]?.[0]); if (l) navigate(`/night/${l.onset_day ?? l.start_day}`); }}
              ariaLabel={`Episode raster for ${metricLabel(ms.metric)}`} testid="episode-raster"
            />
          </div>
        </div>
      </div>
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
  // illness when there are a few, otherwise the kind with the most labels
  const best = ((counts.get('illness') ?? 0) >= 3 ? 'illness' : [...counts.entries()].sort((a, b) => b[1] - a[1])[0]?.[0] ?? 'illness') as LabelKind;
  const [kindP, setKind] = useParam('kind', '');
  const kind = (LABEL_KINDS.some((k) => k.kind === kindP) ? kindP : best) as LabelKind;
  const [idsP, setIds] = useParam('ids', '');
  const [pre, setPre] = useNumParam('pre', 7);
  const [post, setPost] = useNumParam('post', 14);
  const [sameWd, setSameWd] = useParam('wd', '1');
  const [sameLoad, setSameLoad] = useParam('load', '1');
  const [cP] = useParam('c', DEFAULT_METRIC);
  const focused = METRICS.some((m) => m.id === cP) ? cP : DEFAULT_METRIC;
  const ids = useMemo(() => idsP.split(',').filter(Boolean), [idsP]);
  const ofKind = useMemo(() => live.filter((l) => l.kind === kind).sort((a, b) => a.start_day - b.start_day), [live, kind]);
  const missing = missingInput(nights, focused);
  const job = useMemo(
    () => (nights && nights.n > 30 && !missing && ofKind.length
      ? { kind: 'event_study' as const, label_kind: kind, ...(ids.length ? { label_ids: ids } : {}), metrics: [focused], window: [-Math.max(0, pre), Math.max(1, post)] as [number, number], controls: { same_weekday: sameWd === '1', same_load_class: sameLoad === '1' }, n_boot: 400 }
      : null),
    [nights, focused, missing, ofKind.length, kind, ids.join(','), pre, post, sameWd, sameLoad], // eslint-disable-line react-hooks/exhaustive-deps
  );
  const { data, loading, error } = useJob(job);
  const study = data as unknown as Study | null;
  const items = useMemo(() => METRICS.map((m) => ({ id: m.id, title: m.label, na: missingInput(nights, m.id) })), [nights]);

  const tools = (
    <>
      <label>Kind <select value={kind} onChange={(e) => { setKind(e.target.value); setIds(null); }} aria-label="Label kind">
        {LABEL_KINDS.map((k) => <option key={k.kind} value={k.kind}>{k.name} ({counts.get(k.kind) ?? 0})</option>)}
      </select></label>
      <label>Before <input type="number" min={0} max={30} value={pre} onChange={(e) => setPre(Math.min(30, Math.max(0, +e.target.value || 0)))} style={{ width: 60 }} aria-label="Nights before" /></label>
      <label>After <input type="number" min={1} max={60} value={post} onChange={(e) => setPost(Math.min(60, Math.max(1, +e.target.value || 1)))} style={{ width: 60 }} aria-label="Nights after" /></label>
      <label><input type="checkbox" checked={sameWd === '1'} onChange={(e) => setSameWd(e.target.checked ? '1' : '0')} /> same weekday</label>
      <label><input type="checkbox" checked={sameLoad === '1'} onChange={(e) => setSameLoad(e.target.checked ? '1' : '0')} /> same load the day before</label>
      {ofKind.length > 0 && (
        <details style={{ position: 'relative' }}>
          <summary>Episodes ({ids.length ? `${ids.length} of ${ofKind.length}` : ofKind.length})</summary>
          <div className="row" style={{ position: 'absolute', top: '100%', right: 0, zIndex: 20, width: 360, maxHeight: 280, overflow: 'auto', marginTop: 6, padding: 10, gap: 10, background: 'var(--panel)', border: '1px solid var(--line)', borderRadius: 8, boxShadow: '0 8px 24px #0006' }}>
            {ofKind.map((l) => (
              <label key={l.id} className="mono" style={{ fontSize: 12 }}>
                <input type="checkbox" checked={ids.includes(l.id)} onChange={(e) => setIds((e.target.checked ? [...ids, l.id] : ids.filter((i) => i !== l.id)).join(',') || null)} /> {isoDay(l.start_day)}
              </label>
            ))}
            {ids.length > 0 && <button onClick={() => setIds(null)}>All</button>}
          </div>
        </details>
      )}
    </>
  );
  const ready = nights && nights.n > 0 && ofKind.length > 0;
  const sd = { font: '400 12px var(--font)' } as const;
  const strip = ready && (loading || error || study) ? (
    <div className="strip" style={{ minHeight: 24, paddingTop: 6, paddingBottom: 6 }}>
      {(loading || error) && <div style={sd}><JobNote loading={loading} error={error} /></div>}
      {study && study.too_few && <div role="status" data-testid="too-few" style={{ ...sd, color: PAL.hr }}>Too few events ({study.episodes}) for a reliable average; showing each episode.</div>}
      {study && !study.too_few && study.controls_found === 0 && <div role="status" style={{ ...sd, color: PAL.hr }}>No matched control nights were found; relax the weekday or load matching to see the difference.</div>}
      {study && <div className="mono" data-testid="event-summary" style={{ ...sd, color: 'var(--muted)' }}>{study.episodes} episodes, {study.controls_found} control nights, nights {rel(study.window[0])} to {rel(study.window[1])} around onset.</div>}
    </div>
  ) : undefined;

  return (
    <View title="Events" desc="How your nights move around an event, against matched control nights." tools={tools} strip={strip}>
      <Intro>How your nights move around an event, against matched nights of your own with no event nearby (same weekday, same season, same training load the day before). The interval is a bootstrap over events.</Intro>
      {!nights || nights.n === 0 ? <div className="empty">No nights loaded yet.</div>
        : ofKind.length === 0 ? <div className="empty" data-testid="no-events">No {kindInfo(kind).name.toLowerCase()} labels yet. Label some nights in the Inbox, or pick another kind.</div>
        : (
          <FocusList items={items} render={(it) => {
            if (it.na) return <Card title={it.title} method="events.study"><NotAvailable input={it.na} /></Card>;
            const ms = study?.metrics.find((m) => m.metric === it.id);
            if (!study || !ms) return <Card title={it.title} method="events.study"><JobNote loading={loading} error={error} /></Card>;
            return <MetricCard ms={ms} study={study} color={COLORS[Math.max(0, METRICS.findIndex((m) => m.id === it.id)) % COLORS.length]!} labels={ofKind} job={job} />;
          }} />
        )}
    </View>
  );
}

export default function Events() {
  return <Desktop><Body /></Desktop>;
}
