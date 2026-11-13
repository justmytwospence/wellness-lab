import { useEffect, useRef, useState, type ReactNode } from 'react';
import { exportBackup, importBackup, removeEntry, estimate } from '@wl/store';
import { beginSignIn, fetchAuth, finishSignIn, freshToken, opfsFiles, pullWellness, servers, saveServers, LIBRARY_DIR, type ImportEvent, type ImportReport } from '@wl/ingest';
import { PAL, isoDay, sequential } from '@wl/views';
import { useApp } from '../lib/app';
import { useJob } from '../lib/useJob';
import { Empty } from '../charts/small';
import { Panel } from '../ui/Panel';
import { View } from '../ui/View';

/** One hairline-separated section inside a scrolling column. */
function Block({ title, sub, testid, children }: { title: ReactNode; sub?: ReactNode; testid?: string; children: ReactNode }) {
  return (
    <section className="method" data-testid={testid}>
      <h2 style={{ margin: '0 0 8px', fontSize: 14 }}>{title}{sub && <> {sub}</>}</h2>
      {children}
    </section>
  );
}

function Coverage() {
  const { data } = useJob({ kind: 'coverage' });
  const ref = useRef<HTMLCanvasElement>(null);
  const cov = data as unknown as { first_day: number; days: number; series: Record<string, number[]>; no_hr_days: number[]; devices: { n: number; num: Record<string, (number | null)[]>; txt: Record<string, (string | null)[]> } } | null;
  useEffect(() => {
    if (!cov || !ref.current) return;
    const names = Object.keys(cov.series);
    const c = ref.current;
    c.width = Math.min(cov.days, 1400);
    c.height = names.length * 12;
    const g = c.getContext('2d')!;
    names.forEach((n, r) => {
      const v = cov.series[n]!;
      for (let x = 0; x < c.width; x++) {
        const a = Math.floor((x * cov.days) / c.width), b = Math.max(a + 1, Math.floor(((x + 1) * cov.days) / c.width));
        let s = 0;
        for (let i = a; i < b; i++) s += v[i] ?? 0;
        g.fillStyle = s === 0 ? PAL.panel2 : sequential(s / (b - a));
        g.fillRect(x, r * 12, 1, 11);
      }
    });
  }, [cov]);
  if (!cov) return null;
  const names = Object.keys(cov.series);
  const d = cov.devices;
  return (
    <Block title="Coverage" sub={<span className="muted" style={{ fontSize: 12 }}>{isoDay(cov.first_day)} to {isoDay(cov.first_day + cov.days - 1)}</span>}>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="mono muted" style={{ fontSize: 10, lineHeight: '12px', flex: 'none' }}>{names.map((n) => <div key={n}>{n}</div>)}</div>
        <canvas ref={ref} style={{ width: '100%', minWidth: 0, flex: '1 1 0', height: names.length * 12, imageRendering: 'pixelated' }} />
      </div>
      <p className="muted" style={{ fontSize: 12 }}>Each row is a series, each column a stretch of days; brighter is fuller. {cov.no_hr_days.length} days have no heart rate (watch off or not synced), so they are never read as calm days.</p>
      {d.n > 0 && (<><h3>Devices</h3><table className="t"><thead><tr><th>Product</th><th>Serial</th><th>From</th><th>To</th></tr></thead><tbody>{Array.from({ length: d.n }, (_, i) => <tr key={i}><td>{d.txt.product?.[i]}</td><td className="mono">{d.txt.serial?.[i]}</td><td className="mono">{d.num.first_day?.[i] != null ? isoDay(d.num.first_day[i]!) : ''}</td><td className="mono">{d.num.last_day?.[i] != null ? isoDay(d.num.last_day[i]!) : ''}</td></tr>)}</tbody></table></>)}
    </Block>
  );
}

function ImportPanel() {
  const app = useApp();
  const [busy, setBusy] = useState(false);
  const [phase, setPhase] = useState('');
  const [pct, setPct] = useState(0);
  const [report, setReport] = useState<ImportReport | null>(null);
  const [error, setError] = useState('');
  const run = (file: File) => {
    setBusy(true); setError(''); setReport(null); setPct(0);
    const w = new Worker(new URL('../../../packages/ingest/src/import.worker.ts', import.meta.url), { type: 'module' });
    w.onmessage = async (e: MessageEvent<ImportEvent>) => {
      const m = e.data;
      if (m.type === 'phase') setPhase(m.message);
      else if (m.type === 'progress') setPct(m.total ? m.done / m.total : 0);
      else if (m.type === 'done') { setReport(m.report); setBusy(false); w.terminate(); await app.refreshData(); }
      else if (m.type === 'error') { setError(m.error); setBusy(false); w.terminate(); }
    };
    w.postMessage({ file });
  };
  return (
    <Block title="Import a Garmin account export">
      <p className="muted">Request your data at garmin.com (Account, Data Management, Export Your Data) and drop the zip here. It is read in this browser; nothing is uploaded anywhere.</p>
      <label style={{ display: 'block', textAlign: 'center', border: '1px dashed var(--line)', borderRadius: 8, padding: 16, cursor: 'pointer' }} onDragOver={(e) => e.preventDefault()} onDrop={(e) => { e.preventDefault(); const f = e.dataTransfer.files[0]; if (f) run(f); }}>
        {busy ? `${phase} ${Math.round(pct * 100)}%` : 'Drop the export zip here, or click to choose it'}
        <input type="file" accept=".zip" hidden disabled={busy} data-testid="import-file" onChange={(e) => { const f = e.target.files?.[0]; if (f) run(f); }} />
      </label>
      {busy && <progress value={pct} max={1} style={{ width: '100%' }} />}
      {error && <p className="err">{error}</p>}
      {report && (
        <div data-testid="import-report">
          <h3>Imported</h3>
          <p>{report.days ? `${isoDay(report.days[0])} to ${isoDay(report.days[1])}` : 'No days'}; {report.activities_with_rr} activities with beat intervals; {report.skipped} files skipped.</p>
          <table className="t"><tbody>{Object.entries(report.fit_files).map(([k, v]) => <tr key={k}><td>device files: {k}</td><td className="n">{v}</td></tr>)}{Object.entries(report.series_rows).map(([k, v]) => <tr key={k}><td>series: {k}</td><td className="n">{v.toLocaleString()}</td></tr>)}</tbody></table>
          {report.errors.length > 0 && <details><summary>{report.errors.length} warnings</summary><pre className="mono" style={{ fontSize: 11 }}>{report.errors.join('\n')}</pre></details>}
        </div>
      )}
    </Block>
  );
}

function ConnectPanel() {
  const app = useApp();
  const [url, setUrl] = useState(servers()[0]?.url ?? '');
  const [msg, setMsg] = useState('');
  useEffect(() => {
    if (location.pathname === '/oauth/callback') {
      finishSignIn().then((u) => { if (u) { history.replaceState(null, '', '/data'); setMsg('Signed in. Pulling...'); void pull(u); } }).catch((e) => setMsg(String(e.message ?? e)));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const pull = async (u: string) => {
    const s = servers().find((x) => x.url === u);
    try {
      const r = await pullWellness({ base: u, files: opfsFiles, token: async () => { if (!s?.auth) return null; const t = await freshToken(s); if (t === 'signin') throw new Error('sign-in needed'); return t; }, emit: (p) => setMsg(`Pulling ${p.done}/${p.total}`) });
      setMsg(`Pulled ${r.changed.length} files.`);
      await app.refreshData();
    } catch (e) { setMsg(String((e as Error).message)); }
  };
  const connect = async () => {
    const base = url.replace(/\/$/, '');
    const meta = await fetchAuth(base);
    if (typeof meta === 'string') return setMsg(meta);
    saveServers([...servers().filter((x) => x.url !== base), { url: base }]);
    await beginSignIn(base, meta);
  };
  return (
    <Block title="Connect to your garmin-mcp">
      <p className="muted">Point at its wellness export, for example https://garmin.example.com/export/wellness/v1. Your browser signs in and pulls the files directly.</p>
      <div className="row"><input style={{ flex: 1, minWidth: 260 }} value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://.../export/wellness/v1" aria-label="garmin-mcp export address" /><button onClick={() => void connect()} disabled={!url}>Sign in</button><button onClick={() => void pull(url.replace(/\/$/, ''))} disabled={!url}>Pull now</button></div>
      {msg && <p className="muted">{msg}</p>}
    </Block>
  );
}

export default function Data() {
  const app = useApp();
  const [info, setInfo] = useState<StorageEstimate | null>(null);
  useEffect(() => { void estimate().then(setInfo).catch(() => {}); }, [app.version]);
  const m = app.manifest;
  const [sync, setSync] = useState<{ last_ok?: string; error?: string; files?: number } | null>(null);
  useEffect(() => { if (app.mode === 'server') void fetch('/api/info').then((r) => r.json()).then(setSync).catch(() => {}); }, [app.mode, app.version]);
  const backup = async () => {
    const list = (m?.files ?? []).map((f) => ['lab', 'wellness-v1', ...f.path.split('/')]);
    list.push(['lab', 'wellness-v1', 'manifest.json'], ['lab', 'state.json']);
    const blob = await exportBackup({ list: async () => list, read: (p) => opfsFiles.read(p) });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = `wellness-lab-backup-${new Date().toISOString().slice(0, 10)}.zip`;
    a.click();
  };
  const wipe = async () => {
    if (!confirm('Delete all data stored in this browser? Labels and settings are deleted too.')) return;
    await removeEntry(['lab']);
    location.reload();
  };
  return (
    <View title="Data" desc="Where your data comes from and what it covers.">
      <div className="vgrid" style={{ gridTemplateColumns: 'minmax(0,1fr) minmax(0,1fr)' }}>
        <Panel scroll testid="data-left">
          <Block title="Source" testid="source">
          <p>{app.mode === 'server' ? 'Server mode: the server syncs with garmin-mcp and keeps your labels; this browser caches the files.' : 'Local mode: everything is in this browser (OPFS). Nothing leaves your device.'}</p>
          <p className="muted">{app.demo ? 'Showing synthetic demo data.' : m ? `${m.files.length} files, from ${String(m.source.kind)}, written by ${m.importer} on ${m.generated_at.slice(0, 10)}.` : 'No data loaded.'}{info?.usage ? ` Storage used: ${(info.usage / 1e6).toFixed(0)} MB.` : ''}</p>
          {app.mode === 'server' && <div className="row"><button onClick={() => void app.syncNow()}>Sync now</button><span className="muted">{sync?.last_ok ? `last sync ${sync.last_ok}` : ''}{sync?.error ? ` (error: ${sync.error})` : ''}</span></div>}
          <div className="row" style={{ marginTop: 8 }}><button onClick={() => void app.loadDemo()}>Load demo data</button>{m && <button onClick={() => void backup()}>Download backup</button>}
            <label className="row">Restore <input type="file" accept=".zip" onChange={async (e) => { const f = e.target.files?.[0]; if (f) { await importBackup(f, (p, d) => opfsFiles.write(p, d)); await app.refreshData(); } }} /></label>
            <button className="danger" onClick={() => void wipe()}>Delete all local data</button></div>
          </Block>
          {app.mode === 'local' && <><ImportPanel /><ConnectPanel /></>}
          <p className="muted" style={{ fontSize: 12 }}>Files are in OPFS under {LIBRARY_DIR.join('/')}.</p>
        </Panel>
        <Panel scroll testid="data-right">
          {app.nights && app.nights.n > 0 ? <Coverage /> : <Empty>Coverage appears once there is data.</Empty>}
        </Panel>
      </div>
    </View>
  );
}
