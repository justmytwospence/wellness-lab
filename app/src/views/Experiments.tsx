import { useMemo, useState } from 'react';
import { LABEL_KINDS, kindInfo, type ExperimentDef } from '@wl/schema';
import { PAL, isoDay, parseDay, pct, signed, weekday, WEEKDAYS } from '@wl/views';
import { useApp } from '../lib/app';
import { useJob } from '../lib/useJob';
import { useParam } from '../lib/router';
import { QuantileDots, invNormal } from '../charts/analysis/dots';
import { Card, Desktop, Intro, JobNote, NotAvailable } from '../charts/analysis/frame';
import { METRICS, fmt, metricLabel, missingInput } from '../charts/analysis/inputs';
import { View } from '../ui/View';

interface Slot { day: number; arm: 'A' | 'B'; washout: boolean }
interface Result1 { n_a: number; n_b: number; effect: number | null; se: number | null; p_pos: number | null; p_gt_swc: number | null; p_lt_neg_swc: number | null; p_within_swc: number | null; status: string; swc: number | null }
interface Analysis { schedule: Slot[]; itt: Result1; per_protocol: Result1; quantiles: number[] }

const newId = () => (typeof crypto !== 'undefined' && 'randomUUID' in crypto ? crypto.randomUUID() : `x${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`);
const ARM = { A: PAL.hrv, B: PAL.base } as const;
const hatch = 'repeating-linear-gradient(45deg, #8892b855 0 3px, transparent 3px 6px)';

/** The schedule as weeks of seven nights: arm A or B, washout hatched, an adherence checkbox per night when `adherence` is given. */
function Schedule({ slots, adherence, onToggle, today }: { slots: Slot[]; adherence?: Record<number, boolean>; onToggle?: (day: number, v: boolean) => void; today: number }) {
  if (!slots.length) return null;
  const by = new Map(slots.map((s) => [s.day, s]));
  const first = Math.min(...by.keys()), last = Math.max(...by.keys());
  const start = first - weekday(first);
  const days: number[] = [];
  for (let d = start; d <= last; d++) days.push(d);
  while (days.length % 7) days.push(days[days.length - 1]! + 1);
  return (
    <div data-testid="schedule" role="group" aria-label="Schedule calendar">
      <div className="row muted" style={{ fontSize: 12, marginBottom: 6, gap: 14 }}>
        <span><span style={{ color: ARM.A }}>&#9632;</span> A: the change</span>
        <span><span style={{ color: ARM.B }}>&#9632;</span> B: as usual</span>
        <span><span style={{ display: 'inline-block', width: 12, height: 12, background: hatch, verticalAlign: -2 }} /> washout (not analysed)</span>
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(7, minmax(64px, 1fr))', gap: 3, maxWidth: 720 }}>
        {WEEKDAYS.map((w) => <div key={w} className="muted" style={{ fontSize: 11, textAlign: 'center' }}>{w}</div>)}
        {days.map((d) => {
          const s = by.get(d);
          if (!s) return <div key={d} />;
          const adherent = adherence ? adherence[d] !== false : true;
          return (
            <div key={d} data-testid="sched-cell" data-arm={s.arm} data-washout={s.washout} title={`${isoDay(d)}: ${s.arm === 'A' ? 'the change' : 'as usual'}${s.washout ? ', washout' : ''}`}
              style={{ border: `1px solid ${ARM[s.arm]}`, borderRadius: 6, padding: '3px 5px', background: s.washout ? hatch : `${ARM[s.arm]}22`, opacity: !adherent ? 0.5 : 1, outline: d === today ? `2px solid ${PAL.text}` : undefined }}>
              <div className="row mono" style={{ justifyContent: 'space-between', fontSize: 11, gap: 2 }}>
                <span>{isoDay(d).slice(5)}</span><b style={{ color: ARM[s.arm] }}>{s.arm}</b>
              </div>
              {adherence && onToggle && (
                <label style={{ fontSize: 11, display: 'flex', gap: 4, alignItems: 'center' }} className="muted">
                  <input type="checkbox" checked={adherent} onChange={(e) => onToggle(d, e.target.checked)} aria-label={`Followed the plan on ${isoDay(d)}`} /> followed
                </label>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

const sentence = (r: Result1, def: ExperimentDef): string => {
  if (r.n_a < 3 || r.n_b < 3) return `Not enough nights yet: ${r.n_a} on the change (A) and ${r.n_b} as usual (B). Each side needs at least 3 analysed nights.`;
  const good = def.higher_is_better ? 'higher' : 'lower';
  return `${metricLabel(def.outcome)} averaged ${signed(r.effect ?? NaN, 3)} (SE ${fmt(r.se)}) on A compared with B. Chance of a worthwhile improvement (${good} by more than ${fmt(r.swc)}): ${pct(r.p_gt_swc ?? NaN)}; chance of a worthwhile worsening: ${pct(r.p_lt_neg_swc ?? NaN)}; chance the effect is smaller than that either way: ${pct(r.p_within_swc ?? NaN)}.`;
};
const CHIP: Record<string, string> = { 'likely improves': PAL.hrv, 'likely worsens': PAL.alert, 'no meaningful effect': PAL.base, inconclusive: PAL.hr };

function Result({ def }: { def: ExperimentDef }) {
  const { state, nights } = useApp();
  const adherence = state.adherence[def.id] ?? {};
  const job = useMemo(() => ({ kind: 'experiment' as const, def, adherence }), [def, adherence]);
  const { data, loading, error } = useJob(nights && nights.n > 30 ? job : null);
  const a = data as unknown as Analysis | null;
  const [mode, setMode] = useState<'itt' | 'pp'>('itt');
  const r = a ? (mode === 'itt' ? a.itt : a.per_protocol) : null;
  const q = useMemo(() => {
    if (!a || !r) return [];
    if (mode === 'itt') return a.quantiles;
    return r.se && r.se > 0 && r.effect !== null ? Array.from({ length: 20 }, (_, k) => r.effect! + r.se! * invNormal((k + 0.5) / 20)) : [];
  }, [a, r, mode]);
  const lastDay = nights ? Math.max(...(nights.num.day ?? [0]).map((d) => d ?? 0)) : 0;
  const waiting = a ? a.schedule.filter((s) => !s.washout && s.day > lastDay).length : 0;
  return (
    <Card title={`Result: ${def.name}`} method="experiments.n_of_1" job={job} testid="card-result" scroll
      actions={<div className="row" role="radiogroup" aria-label="Analysis"><label><input type="radio" checked={mode === 'itt'} onChange={() => setMode('itt')} /> as planned (ITT)</label><label><input type="radio" checked={mode === 'pp'} onChange={() => setMode('pp')} /> as followed (per protocol)</label></div>}>
      <JobNote loading={loading} error={error} />
      {r && (
        <>
          <p data-testid="status-sentence" style={{ fontSize: 14 }}><span className="chip" style={{ background: CHIP[r.status] ?? PAL.hr }}>{r.status}</span> {sentence(r, def)}</p>
          {q.length > 0 ? (
            <>
              <QuantileDots q={q} swc={r.swc ?? 0} />
              <p className="muted" style={{ fontSize: 12 }}>Twenty dots, each a one in twenty chance of the true effect of A against B, in units of {metricLabel(def.outcome)}. Teal dots are worth having, red worth avoiding, blue are within the smallest worthwhile change (shaded).</p>
            </>
          ) : <p className="muted">The dotplot appears once both sides have at least 3 nights.</p>}
          <table className="t" style={{ maxWidth: 640 }}>
            <thead><tr><th>Analysis</th><th className="n">n A / n B</th><th className="n">Effect</th><th className="n">SE</th><th>Status</th></tr></thead>
            <tbody>
              {([['As planned (ITT)', a!.itt], ['As followed (per protocol)', a!.per_protocol]] as const).map(([n, x]) => (
                <tr key={n}><td>{n}</td><td className="n">{x.n_a} / {x.n_b}</td><td className="n">{x.n_a < 3 || x.n_b < 3 ? '\u2013' : signed(x.effect ?? NaN, 3)}</td><td className="n">{x.n_a < 3 || x.n_b < 3 ? '\u2013' : fmt(x.se)}</td><td>{x.status}</td></tr>
              ))}
            </tbody>
          </table>
          {waiting > 0 && <p className="muted" style={{ fontSize: 12 }}>{waiting} analysed nights are still to come.</p>}
        </>
      )}
    </Card>
  );
}

function Wizard({ onDone }: { onDone(id: string): void }) {
  const app = useApp();
  const { state, nights, lastDay } = app;
  const available = METRICS.filter((m) => !missingInput(nights, m.id));
  const [name, setName] = useState(`Experiment ${state.experiments.length + 1}`);
  const [kind, setKind] = useState<string>('caffeine_late');
  const [tag, setTag] = useState('');
  const [outcome, setOutcome] = useState('ln_rmssd');
  const [hib, setHib] = useState(true);
  const [period, setPeriod] = useState(7);
  const [washout, setWashout] = useState(2);
  const [blocks, setBlocks] = useState(4);
  const [start, setStart] = useState(isoDay(Math.max(lastDay, app.today) + 1));
  const [randomized, setRandomized] = useState(true);
  const [seed] = useState(() => Math.floor(Math.random() * 1_000_000) + 1);
  const [id] = useState(newId);
  const intervention = kind === '__tag' ? tag.trim() : kind;
  const startDay = parseDay(start);
  const def = useMemo<ExperimentDef>(
    () => ({ id, name: name.trim(), intervention, outcome, period_nights: period, washout: Math.min(washout, Math.max(0, period - 1)), blocks, randomized, start_day: Number.isFinite(startDay) ? startDay : lastDay + 1, seed, higher_is_better: hib }),
    [id, name, intervention, outcome, period, washout, blocks, randomized, startDay, lastDay, seed, hib],
  );
  const valid = def.name.length > 0 && def.intervention.length > 0 && Number.isFinite(startDay) && available.some((m) => m.id === outcome);
  const preview = useJob(nights && nights.n > 30 && valid ? { kind: 'experiment' as const, def, adherence: {} } : null);
  const sched = (preview.data as unknown as Analysis | null)?.schedule ?? [];
  const num = (v: string, lo: number, hi: number) => Math.min(hi, Math.max(lo, Math.round(+v) || lo));
  return (
    <Card title="New experiment" method="experiments.n_of_1" testid="wizard" scroll>
      <p className="muted" style={{ fontSize: 12 }}>Alternate blocks of the change (A) and as usual (B), in random order within each block, and compare the outcome. The first nights of each period are a washout and are not analysed.</p>
      <div className="row" style={{ alignItems: 'flex-end', gap: 14 }}>
        <label>Name <input value={name} onChange={(e) => setName(e.target.value)} aria-label="Experiment name" style={{ width: 220 }} /></label>
        <label>Intervention <select value={kind} onChange={(e) => setKind(e.target.value)} aria-label="Intervention">
          {LABEL_KINDS.map((k) => <option key={k.kind} value={k.kind}>{k.name}</option>)}
          <option value="__tag">A tag...</option>
        </select></label>
        {kind === '__tag' && <label>Tag <input value={tag} onChange={(e) => setTag(e.target.value)} aria-label="Tag" style={{ width: 140 }} /></label>}
        <label>Outcome <select value={outcome} onChange={(e) => setOutcome(e.target.value)} aria-label="Outcome metric">
          {available.map((m) => <option key={m.id} value={m.id}>{m.label}</option>)}
        </select></label>
        <label><input type="checkbox" checked={hib} onChange={(e) => setHib(e.target.checked)} /> higher is better</label>
      </div>
      <div className="row" style={{ alignItems: 'flex-end', gap: 14, marginTop: 8 }}>
        <label>Period (nights) <input type="number" value={period} min={2} max={28} onChange={(e) => setPeriod(num(e.target.value, 2, 28))} style={{ width: 64 }} aria-label="Period nights" /></label>
        <label>Washout <input type="number" value={washout} min={0} max={10} onChange={(e) => setWashout(num(e.target.value, 0, 10))} style={{ width: 64 }} aria-label="Washout nights" /></label>
        <label>Blocks <input type="number" value={blocks} min={1} max={8} onChange={(e) => setBlocks(num(e.target.value, 1, 8))} style={{ width: 64 }} aria-label="Blocks" /></label>
        <label>Start <input type="date" value={start} onChange={(e) => setStart(e.target.value)} aria-label="Start date" /></label>
        <label><input type="checkbox" checked={randomized} onChange={(e) => setRandomized(e.target.checked)} /> random order within blocks</label>
        <button className="primary" disabled={!valid} onClick={() => { void app.saveExperiment(def).then(() => { app.toast(`Saved ${def.name}`); onDone(def.id); }); }}>Save experiment</button>
      </div>
      <p className="muted" style={{ fontSize: 12 }}>{blocks * 2 * period} nights in all, {isoDay(def.start_day)} to {isoDay(def.start_day + blocks * 2 * period - 1)}.</p>
      {sched.length > 0 && <Schedule slots={sched} today={app.today} />}
    </Card>
  );
}

function Body() {
  const app = useApp();
  const { state, nights } = app;
  const [expP, setExp] = useParam('exp', '');
  const [open, setOpen] = useState(false);
  // a tick shows at once; the saved state (and the result under it) catches up after the engine has re-run
  const [optimistic, setOptimistic] = useState<Record<string, boolean>>({});
  const sel = state.experiments.find((e) => e.id === expP) ?? state.experiments[0];
  const job = useMemo(() => (sel && nights && nights.n > 30 ? { kind: 'experiment' as const, def: sel, adherence: state.adherence[sel.id] ?? {} } : null), [sel, nights, state.adherence]);
  const sched = useJob(job);
  const slots = (sched.data as unknown as Analysis | null)?.schedule ?? [];
  const missing = sel ? missingInput(nights, sel.outcome) : null;
  const describe = (e: ExperimentDef) => (LABEL_KINDS.some((k) => k.kind === e.intervention) ? kindInfo(e.intervention as never).name : e.intervention);
  const scheduleCard = sel && !missing && (
    <Card title={`Schedule: ${sel.name}`} method="experiments.n_of_1" job={job} testid="card-schedule" scroll>
      <JobNote loading={sched.loading} error={sched.error} />
      {slots.length > 0 ? <Schedule slots={slots} adherence={{ ...(state.adherence[sel.id] ?? {}), ...Object.fromEntries(Object.entries(optimistic).filter(([k]) => k.startsWith(`${sel.id}:`)).map(([k, v]) => [Number(k.split(':')[1]), v])) }} onToggle={(d, v) => { const key = `${sel.id}:${d}`; setOptimistic((o) => ({ ...o, [key]: v })); void app.setAdherence(sel.id, d, v).finally(() => setOptimistic((o) => { const n = { ...o }; delete n[key]; return n; })); }} today={app.today} /> : <p className="muted">Planning...</p>}
    </Card>
  );
  return (
    <View title="Experiments" desc="A/B experiments on yourself, planned in blocks.">
      <Intro>Test one change on yourself: alternate blocks with and without it, and let the numbers say whether it moved your sleep. Each experiment is planned here, you tick off the nights you followed the plan, and the result updates as nights arrive.</Intro>
      <div className="vgrid" style={{ gridTemplateColumns: '340px minmax(0, 1fr)' }}>
        <Card title="Your experiments" actions={<button className="primary" onClick={() => setOpen(!open)}>{open ? 'Close' : 'New experiment'}</button>} testid="experiment-list" scroll>
          {state.experiments.length === 0 ? <p className="muted" data-testid="no-experiments">No experiments yet.</p> : (
            <table className="t">
              <thead><tr><th>Name</th><th>Change</th><th>Outcome</th><th>Plan</th><th /></tr></thead>
              <tbody>
                {state.experiments.map((e) => (
                  <tr key={e.id} data-testid="experiment-row" style={{ background: sel?.id === e.id ? 'var(--panel2)' : undefined }}>
                    <td><button style={{ all: 'unset', cursor: 'pointer', color: PAL.base }} onClick={() => setExp(e.id)}>{e.name}</button></td>
                    <td>{describe(e)}</td>
                    <td>{metricLabel(e.outcome)}</td>
                    <td className="muted">{e.blocks} blocks of 2 x {e.period_nights} nights from {isoDay(e.start_day)}</td>
                    <td><button className="danger" onClick={() => { void app.deleteExperiment(e.id); }} aria-label={`Delete ${e.name}`}>Delete</button></td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </Card>
        {open ? <Wizard onDone={(id) => { setExp(id); setOpen(false); }} />
          : !sel ? <div className="empty" style={{ background: 'var(--bg)', alignSelf: 'center' }}>Choose an experiment or start a new one.</div>
          : missing ? <Card title={sel.name}><NotAvailable input={missing} /></Card>
          : (
            <div className="vgrid" style={{ height: 'auto', gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1fr)' }}>
              {scheduleCard}
              <Result def={sel} />
            </div>
          )}
      </div>
    </View>
  );
}

export default function Experiments() {
  return <Desktop><Body /></Desktop>;
}
