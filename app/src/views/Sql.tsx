import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import schema from '@wl/schema/src/schema';
import type { Table } from '@wl/schema';
import { useApp } from '../lib/app';
import { link, useParam } from '../lib/router';
import type { duckdb } from '../lib/duck';

type Cell = string | number | null;
interface Result { cols: string[]; rows: Cell[][]; total: number; ms: number }
interface ViewInfo { name: string; cols: { name: string; type: string }[] }
interface Saved { name: string; sql: string }

export const MAX_ROWS = 10_000;
const ROW_H = 24;
const VIEW_H = 440;
const KEY = 'wl.sql';

const DAY0 = "DATE '1970-01-01'";
export const STARTERS: Saved[] = [
  { name: 'Daily resting heart rate', sql: `SELECT ${DAY0} + day AS date, rhr, rhr7\nFROM days\nWHERE rhr IS NOT NULL\nORDER BY day;` },
  { name: 'Nights with HRV and load', sql: `-- one row per night; load_prev is the training load of the day before\nSELECT ${DAY0} + day AS date,\n       round(tst_h, 2) AS tst_h,\n       round(rmssd_mean, 1) AS rmssd_ms,\n       round(ln_rmssd, 3) AS ln_rmssd,\n       round(load_prev, 0) AS load_prev\nFROM nights\nWHERE rmssd_mean IS NOT NULL\nORDER BY day;` },
  { name: 'Monthly stress hours', sql: `SELECT strftime(${DAY0} + day, '%Y-%m') AS month,\n       round(sum(rest_s) / 3600.0, 1) AS rest_h,\n       round(sum(low_s) / 3600.0, 1) AS low_h,\n       round(sum(med_s) / 3600.0, 1) AS medium_h,\n       round(sum(high_s) / 3600.0, 1) AS high_h,\n       count(*) AS days\nFROM days\nGROUP BY 1\nORDER BY 1;` },
  { name: 'Stage minutes by month', sql: `-- stage codes: 0 deep, 1 light, 2 REM, 3 awake, 4 unmeasured\nSELECT strftime(${DAY0} + day, '%Y-%m') AS month,\n       count(DISTINCT day) AS nights,\n       round(sum(CASE WHEN stage = 0 THEN "end" - start END) / 60000.0) AS deep_min,\n       round(sum(CASE WHEN stage = 1 THEN "end" - start END) / 60000.0) AS light_min,\n       round(sum(CASE WHEN stage = 2 THEN "end" - start END) / 60000.0) AS rem_min,\n       round(sum(CASE WHEN stage = 3 THEN "end" - start END) / 60000.0) AS awake_min\nFROM stages\nGROUP BY 1\nORDER BY 1;` },
  { name: 'Altitude days', sql: `-- days at least 700 m above your usual altitude (and above 1500 m)\nSELECT ${DAY0} + day AS date, round(altitude_avg) AS altitude_m\nFROM days\nWHERE altitude_avg >= greatest(1500, (SELECT median(altitude_avg) FROM days) + 700)\nORDER BY day;` },
];

const loadSaved = (): Saved[] => {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? '[]') as unknown;
    return Array.isArray(v) ? (v as Saved[]).filter((s) => s && typeof s.name === 'string' && typeof s.sql === 'string') : [];
  } catch {
    return [];
  }
};

function norm(v: unknown, type: string): Cell {
  if (v === null || v === undefined) return null;
  const date = (ms: number) => (type.startsWith('Date') ? new Date(ms).toISOString().slice(0, 10) : new Date(ms).toISOString().replace('T', ' ').replace(/\.000Z$/, '').replace(/Z$/, ''));
  if (typeof v === 'number') return type.startsWith('Date') || type.startsWith('Timestamp') ? date(v) : v;
  if (typeof v === 'bigint') return type.startsWith('Timestamp') ? date(Number(v)) : Number.isSafeInteger(Number(v)) ? Number(v) : v.toString();
  if (v instanceof Date) return date(v.getTime());
  if (typeof v === 'string') return v;
  return String(v);
}

const show = (c: Cell) => (c === null ? '' : typeof c === 'number' && !Number.isInteger(c) ? String(Number(c.toFixed(6))) : String(c));

export function toCsv(r: Pick<Result, 'cols' | 'rows'>): string {
  const esc = (c: Cell) => {
    const s = c === null ? '' : String(c);
    return /[",\n\r]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
  };
  return [r.cols.map(esc).join(','), ...r.rows.map((row) => row.map(esc).join(','))].join('\n') + '\n';
}

/** A result table that only renders the rows in view (fixed row height). */
function Grid({ r }: { r: Result }) {
  const [top, setTop] = useState(0);
  const first = Math.max(0, Math.floor(top / ROW_H) - 6);
  const last = Math.min(r.rows.length, first + Math.ceil(VIEW_H / ROW_H) + 12);
  const widths = [64, ...r.cols.map(() => 150)];
  return (
    <div data-testid="sql-grid" onScroll={(e) => setTop(e.currentTarget.scrollTop)} style={{ height: Math.min(VIEW_H, (r.rows.length + 1) * ROW_H + 18), overflow: 'auto', border: '1px solid var(--line)', borderRadius: 6 }}>
      <table className="t mono" style={{ tableLayout: 'fixed', width: widths.reduce((a, b) => a + b, 0), fontSize: 12 }}>
        <colgroup>{widths.map((w, i) => <col key={i} style={{ width: w }} />)}</colgroup>
        <thead><tr><th className="n" style={{ height: ROW_H }}>#</th>{r.cols.map((c, i) => <th key={i} style={{ height: ROW_H, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{c}</th>)}</tr></thead>
        <tbody>
          {first > 0 && <tr style={{ height: first * ROW_H }}><td colSpan={widths.length} style={{ border: 0 }} /></tr>}
          {r.rows.slice(first, last).map((row, k) => (
            <tr key={first + k} style={{ height: ROW_H }}>
              <td className="n muted">{first + k + 1}</td>
              {row.map((c, j) => <td key={j} className={typeof c === 'number' ? 'n' : ''} style={{ whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }} title={show(c)}>{show(c)}</td>)}
            </tr>
          ))}
          {last < r.rows.length && <tr style={{ height: (r.rows.length - last) * ROW_H }}><td colSpan={widths.length} style={{ border: 0 }} /></tr>}
        </tbody>
      </table>
    </div>
  );
}

type Status = { kind: 'idle' } | { kind: 'starting' } | { kind: 'ready' } | { kind: 'empty' } | { kind: 'error'; message: string };

export default function Sql() {
  const { demo, manifest, nights, engine, version } = useApp();
  const [q0] = useParam('q', '');
  const [sql, setSql] = useState(q0 || STARTERS[1]!.sql);
  const [status, setStatus] = useState<Status>({ kind: 'idle' });
  const [views, setViews] = useState<ViewInfo[]>([]);
  const [result, setResult] = useState<Result | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [saved, setSaved] = useState<Saved[]>(loadSaved);
  const [name, setName] = useState('');
  const db = useRef<Promise<duckdb.AsyncDuckDB> | null>(null);
  const conn = useRef<duckdb.AsyncDuckDBConnection | null>(null);
  const owned = useRef<string[]>([]);
  const chain = useRef<Promise<unknown>>(Promise.resolve());
  const seq = useRef(0);

  const hasNights = !!nights && nights.n > 0;
  const stamp = manifest?.generated_at ?? '';

  // Start DuckDB on the first visit and (re)build the views when the data change.
  useEffect(() => {
    const mine = ++seq.current;
    if (!demo && !manifest && !hasNights) return setStatus({ kind: 'empty' });
    setStatus({ kind: 'starting' });
    chain.current = chain.current.then(async () => {
      if (mine !== seq.current) return;
      try {
        const duck = await import('../lib/duck');
        const d = await (db.current ??= duck.openDuck());
        const c = (conn.current ??= await d.connect());
        for (const v of owned.current) await c.query(`DROP VIEW IF EXISTS ${v}`);
        owned.current = [];
        if (demo) {
          // no files in demo mode: the days and nights tables come from the engine
          const cols = ((schema as unknown as { tables: Record<string, [string, string][]> }).tables.days ?? []).map((x) => x[0]);
          const days = (await engine.run({ kind: 'days', from: -1_000_000, to: 1_000_000, columns: cols })) as unknown as Table;
          if (days.n > 0) {
            await duck.registerTable(d, c, 'days', days);
            owned.current.push('days');
          }
        } else if (manifest) {
          const raw = await duck.registerRaw(d, c, []);
          owned.current.push(...raw.views);
        }
        if (nights && nights.n > 0) {
          await duck.registerTable(d, c, 'nights', nights);
          owned.current.push('nights');
        }
        const info = await c.query("SELECT table_name, column_name, data_type FROM information_schema.columns WHERE table_schema = 'main' ORDER BY table_name, ordinal_position");
        const by = new Map<string, ViewInfo>();
        for (let i = 0; i < info.numRows; i++) {
          const row = info.get(i)!;
          const t = String(row.table_name);
          (by.get(t) ?? by.set(t, { name: t, cols: [] }).get(t)!).cols.push({ name: String(row.column_name), type: String(row.data_type) });
        }
        if (mine !== seq.current) return;
        setViews([...by.values()]);
        setStatus({ kind: 'ready' });
      } catch (e) {
        if (mine === seq.current) setStatus({ kind: 'error', message: String((e as Error).message ?? e) });
      }
    });
  }, [demo, stamp, nights, version, engine, manifest, hasNights]);

  useEffect(() => () => { seq.current++; }, []);

  const run = useCallback(async () => {
    const c = conn.current;
    if (!c || running || !sql.trim()) return;
    setRunning(true);
    setError(null);
    const t0 = performance.now();
    try {
      const t = await c.query(sql.replace(/;\s*$/, ''));
      const cols = t.schema.fields.map((f) => f.name);
      const types = t.schema.fields.map((f) => String(f.type));
      const vecs = cols.map((_, j) => t.getChildAt(j)!);
      const n = Math.min(t.numRows, MAX_ROWS);
      const rows: Cell[][] = new Array(n);
      for (let i = 0; i < n; i++) rows[i] = vecs.map((v, j) => norm(v.get(i), types[j]!));
      setResult({ cols, rows, total: t.numRows, ms: Math.round(performance.now() - t0) });
    } catch (e) {
      setResult(null);
      setError(String((e as Error).message ?? e));
    } finally {
      setRunning(false);
    }
  }, [sql, running]);

  const persist = (next: Saved[]) => {
    setSaved(next);
    try { localStorage.setItem(KEY, JSON.stringify(next)); } catch { /* storage unavailable */ }
  };
  const save = () => {
    const n = name.trim();
    if (!n) return;
    persist([...saved.filter((s) => s.name !== n), { name: n, sql }]);
    setName('');
  };

  const download = () => {
    if (!result) return;
    const url = URL.createObjectURL(new Blob([toCsv(result)], { type: 'text/csv' }));
    const a = Object.assign(document.createElement('a'), { href: url, download: 'query.csv' });
    document.body.append(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  };

  const ready = status.kind === 'ready';
  const notice = useMemo(() => {
    if (demo) return 'Demo data has no raw files. Load real data to query raw files; demo mode offers the nights table (and days, built from the engine) only.';
    return null;
  }, [demo]);

  return (
    <div className="page">
      <h1>SQL</h1>
      <p className="muted">DuckDB running in this browser over your data. Nothing leaves the device. <kbd>Ctrl</kbd>/<kbd>Cmd</kbd>+<kbd>Enter</kbd> runs the query.</p>
      {notice && <div className="card" role="note">{notice}</div>}
      {status.kind === 'empty' && <div className="card">No data is loaded. <a href="/data" onClick={link('/data')}>Import a Garmin export, connect garmin-mcp or load demo data</a>, then come back.</div>}
      {status.kind === 'starting' && <div className="card muted" role="status">Starting DuckDB and registering your files...</div>}
      {status.kind === 'error' && <div className="card err" role="alert">Could not start DuckDB: {status.message}</div>}

      <div style={{ display: 'grid', gridTemplateColumns: 'minmax(0, 1fr) 260px', gap: 14, alignItems: 'start' }}>
        <div>
          <div className="row" style={{ marginBottom: 8 }}>
            <select aria-label="Starter queries" value="" onChange={(e) => { const s = STARTERS[Number(e.target.value)]; if (s) setSql(s.sql); }}>
              <option value="">Starter queries...</option>
              {STARTERS.map((s, i) => <option key={s.name} value={i}>{s.name}</option>)}
            </select>
            {saved.length > 0 && (
              <select aria-label="Saved queries" value="" onChange={(e) => { const s = saved.find((x) => x.name === e.target.value); if (s) setSql(s.sql); }}>
                <option value="">Saved queries...</option>
                {saved.map((s) => <option key={s.name} value={s.name}>{s.name}</option>)}
              </select>
            )}
          </div>
          <textarea
            aria-label="SQL query"
            className="mono"
            spellCheck={false}
            value={sql}
            rows={11}
            onChange={(e) => setSql(e.target.value)}
            onKeyDown={(e) => { if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) { e.preventDefault(); void run(); } }}
            style={{ width: '100%', fontSize: 13, lineHeight: 1.5, resize: 'vertical', tabSize: 2 }}
          />
          <div className="row" style={{ margin: '8px 0 12px' }}>
            <button className="primary" disabled={!ready || running} onClick={() => void run()}>{running ? 'Running...' : 'Run'}</button>
            <button disabled={!result || result.rows.length === 0} onClick={download}>Download CSV</button>
            <span className="spacer" style={{ flex: 1 }} />
            <input aria-label="Name to save the query under" placeholder="Name" value={name} onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && save()} style={{ width: 150 }} />
            <button disabled={!name.trim()} onClick={save}>Save query</button>
          </div>
          {error && <pre className="err mono" role="alert" style={{ whiteSpace: 'pre-wrap', fontSize: 12 }}>{error}</pre>}
          {result && (
            <div>
              <p className="muted" data-testid="sql-summary">
                {result.total.toLocaleString()} {result.total === 1 ? 'row' : 'rows'} in {result.ms} ms
                {result.total > MAX_ROWS && `; showing and exporting the first ${MAX_ROWS.toLocaleString()}`}
              </p>
              {result.rows.length > 0 ? <Grid r={result} /> : <p className="muted">The query returned no rows.</p>}
            </div>
          )}
        </div>
        <aside className="card" aria-label="Tables" style={{ maxHeight: 640, overflow: 'auto' }}>
          <h3>Tables</h3>
          {views.length === 0 && <p className="muted" style={{ fontSize: 12 }}>{ready ? 'No tables.' : 'Not started.'}</p>}
          {views.map((v) => (
            <details key={v.name}>
              <summary className="mono" style={{ cursor: 'pointer' }}>{v.name} <span className="muted">({v.cols.length})</span></summary>
              <ul className="mono muted" style={{ fontSize: 11, margin: '2px 0 6px', paddingLeft: 16 }}>
                {v.cols.map((c) => <li key={c.name}>{c.name} <span style={{ opacity: 0.6 }}>{c.type.toLowerCase()}</span></li>)}
              </ul>
            </details>
          ))}
          {saved.length > 0 && (
            <>
              <h3 style={{ marginTop: 14 }}>Saved</h3>
              <ul style={{ margin: 0, paddingLeft: 0, listStyle: 'none', fontSize: 13 }}>
                {saved.map((s) => (
                  <li key={s.name} className="row" style={{ justifyContent: 'space-between', gap: 4 }}>
                    <a href="#" onClick={(e) => { e.preventDefault(); setSql(s.sql); }}>{s.name}</a>
                    <button aria-label={`Delete ${s.name}`} style={{ padding: '0 6px' }} onClick={() => persist(saved.filter((x) => x.name !== s.name))}>x</button>
                  </li>
                ))}
              </ul>
            </>
          )}
        </aside>
      </div>
    </div>
  );
}
