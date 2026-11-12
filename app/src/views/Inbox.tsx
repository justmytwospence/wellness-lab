import { useEffect, useState } from 'react';
import { LABEL_KINDS, kindInfo, type Label, type Proposal } from '@wl/schema';
import { PAL, confidence, isoDay, why } from '@wl/views';
import { useApp } from '../lib/app';
import { link } from '../lib/router';
import { Spark, Empty } from '../charts/small';
import { LabelDialog } from '../layout/LabelDialog';
import { useJob } from '../lib/useJob';
import { col } from '../lib/util';

function Evidence({ p }: { p: Proposal }) {
  const { data } = useJob({ kind: 'baseline', metric: 'hr_min30' });
  const { data: h } = useJob({ kind: 'baseline', metric: 'ln_rmssd' });
  const pick = (t: unknown) => {
    const d = col(t as never, 'day'), x = col(t as never, 'x');
    const a = d.findIndex((v) => v >= p.start_day - 7), b = d.findIndex((v) => v > p.end_day + 7);
    return x.slice(a < 0 ? 0 : a, b < 0 ? undefined : b);
  };
  return (
    <div className="row" style={{ fontSize: 12 }}>
      {data && <span className="muted">sleeping HR <Spark values={pick(data)} width={140} color={PAL.hr} /></span>}
      {h && <span className="muted">ln HRV <Spark values={pick(h)} width={140} /></span>}
    </div>
  );
}

function Posterior({ p }: { p: Proposal }) {
  const e = Object.entries(p.posterior).sort((a, b) => (b[1] ?? 0) - (a[1] ?? 0)).slice(0, 5);
  return (
    <div style={{ minWidth: 220 }}>
      {e.map(([k, v]) => (
        <div key={k} className="row" style={{ gap: 6, fontSize: 12 }}>
          <span style={{ width: 110 }}>{kindInfo(k as never).name}</span>
          <div style={{ flex: 1, height: 6, background: PAL.grid, borderRadius: 3 }}><div style={{ width: `${(v ?? 0) * 100}%`, height: 6, background: kindInfo(k as never).color, borderRadius: 3 }} /></div>
          <span className="mono">{Math.round((v ?? 0) * 100)}%</span>
        </div>
      ))}
    </div>
  );
}

export default function Inbox() {
  const app = useApp();
  const { state, answer, deleteLabel } = app;
  const [cur, setCur] = useState(0);
  const [edit, setEdit] = useState<Proposal | null>(null);
  const [labelEdit, setLabelEdit] = useState<Partial<Label> & { start_day: number; end_day: number } | null>(null);
  const [kindFilter, setKindFilter] = useState('');
  const open = state.proposals.filter((p) => p.status === 'open' && p.asked && !p.auto).sort((a, b) => b.priority - a.priority);
  const inferred = state.labels.filter((l) => !l.deleted_at && l.source === 'inferred');
  const answered = state.proposals.filter((p) => p.status !== 'open' && p.status !== 'expired').sort((a, b) => b.start_day - a.start_day);
  useEffect(() => {
    const on = (e: KeyboardEvent) => {
      if (/^(INPUT|TEXTAREA|SELECT)$/.test((e.target as HTMLElement).tagName) || e.metaKey || e.ctrlKey) return;
      const p = open[cur];
      if (e.key === 'j') setCur((c) => Math.min(open.length - 1, c + 1));
      else if (e.key === 'k') setCur((c) => Math.max(0, c - 1));
      else if (!p) return;
      else if (e.key === 'c') void answer(p.id, 'confirm');
      else if (e.key === 'd') void answer(p.id, 'deny');
      else if (e.key === 'u') void answer(p.id, 'unsure');
      else if (e.key === 's') void answer(p.id, 'snooze');
      else if (e.key === 'e') setEdit(p);
    };
    window.addEventListener('keydown', on);
    return () => window.removeEventListener('keydown', on);
  });
  const labels = state.labels.filter((l) => !l.deleted_at && (!kindFilter || l.kind === kindFilter)).sort((a, b) => b.start_day - a.start_day);
  const confirmed = state.proposals.filter((p) => p.status === 'confirmed').length, denied = state.proposals.filter((p) => p.status === 'denied').length;
  return (
    <div className="page">
      <h1>Inbox</h1>
      <p className="muted">Things the app suspects from your nights. Confirm or deny, and it learns what your own illnesses and nights out look like. At most {state.settings.question_budget_per_week} questions a week.</p>
      <div className="grid2" style={{ gridTemplateColumns: 'minmax(0, 1.5fr) minmax(300px, 1fr)' }}>
        <div>
          <h3>Asking</h3>
          {open.length === 0 && <div className="card"><Empty>Nothing to ask right now.</Empty></div>}
          {open.map((p, i) => (
            <div key={p.id} className="card" data-testid="proposal" style={{ outline: i === cur ? `1px solid ${PAL.hrv}` : 'none' }} onClick={() => setCur(i)}>
              <div className="row" style={{ justifyContent: 'space-between' }}>
                <h2 style={{ color: kindInfo(p.kind).color }}>{kindInfo(p.kind).name}? <span className="mono muted" style={{ fontSize: 13 }}>{isoDay(p.start_day)}{p.end_day > p.start_day ? ` to ${isoDay(p.end_day)}` : ''}</span></h2>
                <span className="chip" style={{ background: PAL.base }}>{confidence(p.posterior[p.kind] ?? 0)}</span>
              </div>
              <p>{why(p.evidence)}</p>
              <div className="row" style={{ justifyContent: 'space-between', alignItems: 'flex-start' }}><Evidence p={p} /><Posterior p={p} /></div>
              <div className="row" style={{ marginTop: 10 }}>
                <button className="primary" onClick={() => void answer(p.id, 'confirm')}>Confirm (c)</button>
                <button onClick={() => void answer(p.id, 'deny')}>Deny (d)</button>
                <button onClick={() => void answer(p.id, 'unsure')}>Not sure (u)</button>
                <button onClick={() => void answer(p.id, 'snooze')}>Snooze 7 days (s)</button>
                <button onClick={() => setEdit(p)}>Edit (e)</button>
                <a href={`/night/${p.start_day}`} onClick={link(`/night/${p.start_day}`)}>Open night</a>
              </div>
            </div>
          ))}
          {inferred.length > 0 && (
            <>
              <h3>Labelled automatically</h3>
              <div className="card">{inferred.map((l) => (
                <div key={l.id} className="row" style={{ padding: '3px 0' }}>
                  <b style={{ color: kindInfo(l.kind).color }}>{kindInfo(l.kind).name}</b><span className="mono">{isoDay(l.start_day)}{l.end_day > l.start_day ? ` to ${isoDay(l.end_day)}` : ''}</span>
                  <span className="muted" style={{ flex: 1 }}>{l.note}</span><button onClick={() => void deleteLabel(l.id)}>Wrong, remove</button>
                </div>))}</div>
            </>
          )}
          <details><summary>Answered ({answered.length})</summary>
            <div className="card">{answered.map((p) => <div key={p.id} className="row" style={{ padding: '3px 0' }}><span style={{ color: kindInfo(p.kind).color }}>{kindInfo(p.kind).name}</span><span className="mono">{isoDay(p.start_day)}</span><span className="muted">{p.status}</span></div>)}</div>
          </details>
        </div>
        <div>
          <div className="row" style={{ justifyContent: 'space-between' }}><h3>Labels</h3><button onClick={() => setLabelEdit({ start_day: app.lastDay, end_day: app.lastDay })}>New label</button></div>
          <select value={kindFilter} onChange={(e) => setKindFilter(e.target.value)} aria-label="Filter labels"><option value="">All kinds</option>{LABEL_KINDS.map((k) => <option key={k.kind} value={k.kind}>{k.name}</option>)}</select>
          <div className="card" style={{ marginTop: 8 }} data-testid="labels">
            {labels.length === 0 && <Empty>No labels yet.</Empty>}
            {labels.map((l) => (
              <div key={l.id} className="row" style={{ padding: '3px 0', borderBottom: `1px solid ${PAL.grid}` }}>
                <b style={{ color: kindInfo(l.kind).color }}>{kindInfo(l.kind).name}</b><span className="mono" style={{ fontSize: 12 }}>{isoDay(l.start_day)}{l.end_day > l.start_day ? ` to ${isoDay(l.end_day)}` : ''}</span>
                <span className="muted" style={{ fontSize: 11 }}>{l.source}</span><span style={{ flex: 1 }} />
                <button onClick={() => setLabelEdit(l)}>Edit</button><button onClick={() => void deleteLabel(l.id)}>Delete</button>
              </div>))}
          </div>
        </div>
      </div>
      <p className="muted" style={{ fontSize: 12 }}>Detector {state.proposals[0]?.model_version ?? 'classify-1'}: {confirmed} confirmed and {denied} denied episodes so far.</p>
      {edit && <LabelDialog proposalId={edit.id} initial={{ kind: edit.kind, start_day: edit.start_day, end_day: edit.end_day }} onClose={() => setEdit(null)} />}
      {labelEdit && <LabelDialog initial={labelEdit} onClose={() => setLabelEdit(null)} />}
    </div>
  );
}
