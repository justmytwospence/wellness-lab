import { Info } from 'lucide-react';
import { useApp } from '../lib/app';
import { Grade } from '../charts/small';

/** The info button on every chart opens this: the why, the registry entry, and the job that produced the numbers. */
export function MethodButton({ id, job }: { id: string; job?: unknown }) {
  const { ui } = useApp();
  return (
    <button className="icon ghost" aria-label={`About ${id}`} title="How this is computed" onClick={() => { ui.setMethod(id); ui.setInspector(true); if (job) sessionStorage.setItem('wl.method.job', JSON.stringify(job)); else sessionStorage.removeItem('wl.method.job'); }} style={{ width: 24, height: 24 }}><Info size={14} /></button>
  );
}

export function MethodDetails() {
  const { ui, registry } = useApp();
  if (!ui.method) return null;
  const m = registry?.methods.find((x) => x.id === ui.method);
  const job = sessionStorage.getItem('wl.method.job');
  return (
    <div>
      <h1>{m?.name ?? ui.method}{m && <Grade g={m.validity} />}</h1>
      {m ? (
        <>
          <p>{m.summary}</p>
          <h3>Formula</h3>
          <p className="mono" style={{ fontSize: 12 }}>{m.formula}</p>
          {m.params.length > 0 && (<><h3>Parameters</h3><table className="t"><tbody>{m.params.map((p) => (<tr key={p.name}><td className="mono">{p.name}</td><td className="n">{p.default} {p.unit}</td><td className="muted">{p.help}</td></tr>))}</tbody></table></>)}
          <h3>Inputs</h3>
          <p>{m.inputs.join(', ') || 'none'}{m.min_resolution_s ? `; needs data at least every ${m.min_resolution_s} s` : ''}</p>
          <h3>References</h3>
          <ul>{m.refs.map((k) => { const r = registry?.refs.find((x) => x.key === k); return r ? <li key={k}><a href={r.url} target="_blank" rel="noreferrer">{r.citation}</a>: {r.title}</li> : null; })}</ul>
        </>
      ) : (<p className="muted">No registry entry for {ui.method}.</p>)}
      {job && (<><h3>Engine job</h3><pre className="mono" style={{ fontSize: 11, whiteSpace: 'pre-wrap' }}>{job}</pre><button onClick={() => navigator.clipboard.writeText(job)}>Copy job</button></>)}
    </div>
  );
}
