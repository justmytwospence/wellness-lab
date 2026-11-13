import { useEffect, useState } from 'react';
import { LABEL_KINDS, kindInfo, type Label, type LabelKind } from '@wl/schema';
import { isoDay, parseDay } from '@wl/views';
import { useApp } from '../lib/app';

/** Create or edit a label: kind, days, onset, severity, note and whether it is left out of baselines. */
export function LabelDialog({ initial, onClose, proposalId }: { initial: Partial<Label> & { start_day: number; end_day: number }; onClose(): void; proposalId?: string }) {
  const { saveLabel, updateLabel, answer } = useApp();
  const [kind, setKind] = useState<LabelKind>(initial.kind ?? 'illness');
  const [start, setStart] = useState(isoDay(initial.start_day));
  const [end, setEnd] = useState(isoDay(initial.end_day));
  const [onset, setOnset] = useState(initial.onset_day ? isoDay(initial.onset_day) : '');
  const [sev, setSev] = useState(initial.severity ?? 0);
  const [note, setNote] = useState(initial.note ?? '');
  const [excl, setExcl] = useState(initial.exclude_baseline ?? kindInfo(initial.kind ?? 'illness').exclude);
  const [touched, setTouched] = useState(initial.exclude_baseline !== undefined);
  useEffect(() => {
    const on = (e: KeyboardEvent) => e.key === 'Escape' && onClose();
    window.addEventListener('keydown', on);
    return () => window.removeEventListener('keydown', on);
  }, [onClose]);
  const save = async () => {
    const l = { ...initial, kind, start_day: parseDay(start), end_day: Math.max(parseDay(start), parseDay(end)), onset_day: onset ? parseDay(onset) : null, severity: sev || null, note, exclude_baseline: excl } as Label;
    if (proposalId) await answer(proposalId, 'confirm', l);
    else if (initial.id) await updateLabel(l);
    else await saveLabel(l);
    onClose();
  };
  return (
    <div className="overlay" onClick={onClose}>
      <div className="dialog" role="dialog" aria-label="Label" onClick={(e) => e.stopPropagation()}>
        <h1>{initial.id ? 'Edit label' : 'Label these nights'}</h1>
        <div className="row" style={{ margin: '10px 0' }}>
          <label>Kind <select value={kind} onChange={(e) => { const k = e.target.value as LabelKind; setKind(k); if (!touched) setExcl(kindInfo(k).exclude); }}>{LABEL_KINDS.map((k) => <option key={k.kind} value={k.kind}>{k.name}</option>)}</select></label>
          <label>From <input type="date" value={start} onChange={(e) => setStart(e.target.value)} /></label>
          <label>To <input type="date" value={end} onChange={(e) => setEnd(e.target.value)} /></label>
        </div>
        <div className="row" style={{ margin: '10px 0' }}>
          <label>First symptoms <input type="date" value={onset} onChange={(e) => setOnset(e.target.value)} /></label>
          <label>Severity <select value={sev} onChange={(e) => setSev(+e.target.value)}><option value={0}>-</option><option value={1}>Mild</option><option value={2}>Moderate</option><option value={3}>Severe</option></select></label>
        </div>
        <label style={{ display: 'block', margin: '10px 0' }}><input type="checkbox" checked={excl} onChange={(e) => { setExcl(e.target.checked); setTouched(true); }} /> Leave these nights out of my baselines</label>
        <textarea value={note} onChange={(e) => setNote(e.target.value)} placeholder="Note" rows={3} style={{ width: '100%' }} />
        <div className="row" style={{ marginTop: 12, justifyContent: 'flex-end' }}><button onClick={onClose}>Cancel</button><button className="primary" onClick={() => void save()}>Save</button></div>
      </div>
    </div>
  );
}
