import { X } from 'lucide-react';
import { useApp } from '../lib/app';
import { Grade } from '../charts/small';

/** The info button on every chart opens this: the why, the registry entry, and the job that produced the numbers. */
export function MethodButton({ id, job }: { id: string; job?: unknown }) {
  const { ui } = useApp();
  return (
    <button aria-label={`About ${id}`} title="How this is computed" onClick={() => { ui.setMethod(id); if (job) sessionStorage.setItem('wl.method.job', JSON.stringify(job)); else sessionStorage.removeItem('wl.method.job'); }} style={{ padding: '0 7px', borderRadius: 12, fontSize: 12 }}>i</button>
  );
}

export function MethodPanel() {
  const { ui, registry } = useApp();
  if (!ui.method) return null;
  const m = registry?.methods.find((x) => x.id === ui.method);
  const job = sessionStorage.getItem('wl.method.job');
  return (
    <aside className="side" role="dialog" aria-label="Method">
      <div className="row" style={{ justifyContent: 'space-between' }}>
        <h1>{m?.name ?? ui.method}{m && <Grade g={m.validity} />}</h1>
        <button onClick={() => ui.setMethod(null)} aria-label="Close"><X size={14} /></button>
      </div>
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
    </aside>
  );
}
