import { useMemo, useRef, useState } from 'react';
import type { MethodSpec, RefSpec } from '@wl/schema';
import { useApp } from '../lib/app';
import { Empty, Grade } from '../charts/small';
import { Panel } from '../ui/Panel';
import { View } from '../ui/View';

const resolution = (s: number | null) => {
  if (s === null) return 'any';
  if (s % 60 === 0 && s >= 60) return `${s / 60} min`;
  return `${s} s`;
};

function Method({ m, refs }: { m: MethodSpec; refs: Map<string, RefSpec> }) {
  return (
    <section className="method" id={m.id} data-method={m.id}>
      <h2>{m.name}<Grade g={m.validity} /> <span className="muted mono" style={{ fontSize: 11, fontWeight: 400 }}>{m.id}</span></h2>
      <p>{m.summary}</p>
      <h3>Formula</h3>
      <p className="mono" style={{ fontSize: 12.5, overflowWrap: 'anywhere' }}>{m.formula}</p>
      {m.params.length > 0 && (
        <>
          <h3>Parameters</h3>
          <table className="t" style={{ marginBottom: 10 }}>
            <thead><tr><th>Name</th><th className="n">Default</th><th className="n">Range</th><th>Unit</th><th>Meaning</th></tr></thead>
            <tbody>
              {m.params.map((p) => (
                <tr key={p.name}>
                  <td className="mono">{p.name}</td>
                  <td className="n">{p.default}</td>
                  <td className="n">{p.min} to {p.max}</td>
                  <td>{p.unit}</td>
                  <td className="muted">{p.help}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
      <h3>Inputs</h3>
      <p>
        {m.inputs.length ? m.inputs.map((i, k) => <span key={i}>{k > 0 && ', '}<span className="mono">{i}</span></span>) : 'none'}
        <span className="muted">; minimum resolution: {resolution(m.min_resolution_s)}</span>
      </p>
      <h3>References</h3>
      {m.refs.length === 0 ? <p className="muted">None: a convention of this app, not a published method.</p> : (
        <ul style={{ margin: '0 0 4px', paddingLeft: 18 }}>
          {m.refs.map((k) => {
            const r = refs.get(k);
            if (!r) return <li key={k} className="muted">{k}</li>;
            return (
              <li key={k}>
                {r.citation} ({r.year}): {r.title}{' '}
                <a href={r.url} target="_blank" rel="noreferrer" data-doi={r.doi ? 'true' : undefined}>{r.doi ? r.url.replace(/^https?:\/\/(dx\.)?doi\.org\//, 'doi:') : 'link'}</a>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

export default function Methods() {
  const { registry } = useApp();
  const [q, setQ] = useState('');
  const refs = useMemo(() => new Map((registry?.refs ?? []).map((r) => [r.key, r])), [registry]);
  const groups = useMemo(() => {
    const out = new Map<string, MethodSpec[]>();
    const needle = q.trim().toLowerCase();
    for (const m of registry?.methods ?? []) {
      if (needle && !`${m.id} ${m.name} ${m.group} ${m.summary}`.toLowerCase().includes(needle)) continue;
      (out.get(m.group) ?? out.set(m.group, []).get(m.group)!).push(m);
    }
    return [...out];
  }, [registry, q]);

  const doc = useRef<HTMLDivElement>(null);
  const go = (g: string) => (e: React.MouseEvent) => {
    e.preventDefault();
    doc.current?.querySelector(`#group-${CSS.escape(encodeURIComponent(g))}`)?.scrollIntoView({ block: 'start' });
  };

  if (!registry) return <View title="Methods"><Empty>Loading the method registry...</Empty></View>;
  return (
    <View
      title="Methods"
      desc="Every method, its formula and its sources."
      tools={<input type="search" aria-label="Filter methods" placeholder="Filter methods" value={q} onChange={(e) => setQ(e.target.value)} />}
    >
      <div className="vgrid" style={{ gridTemplateColumns: '200px minmax(0,1fr)' }}>
        <Panel scroll testid="methods-groups">
          <nav className="vnav" aria-label="Groups">
            {groups.map(([g, ms]) => <a key={g} href={`#group-${encodeURIComponent(g)}`} onClick={go(g)}>{g} <span className="muted">({ms.length})</span></a>)}
          </nav>
        </Panel>
        <Panel scroll testid="methods-doc">
          <div ref={doc} style={{ maxWidth: 980 }}>
            <p className="muted" style={{ marginTop: 0 }}>
              Every number in this app comes from one of these methods. The badge is how far to trust it on wrist data: A is solid, B is good for your own trends, C is experimental, D is shown for completeness.
              {' '}{registry.methods.length} methods.
            </p>
            {groups.length === 0 && <Empty>No method matches "{q}".</Empty>}
            {groups.map(([g, ms]) => (
              <div key={g} id={`group-${encodeURIComponent(g)}`}>
                <h2 style={{ margin: '18px 0 8px', fontSize: 17 }}>{g}</h2>
                {ms.map((m) => <Method key={m.id} m={m} refs={refs} />)}
              </div>
            ))}
          </div>
        </Panel>
      </div>
    </View>
  );
}
