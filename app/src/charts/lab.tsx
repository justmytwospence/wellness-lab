import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useApp } from '../lib/app';
import { useParam } from '../lib/router';
import { MethodButton } from '../layout/MethodPanel';
import { Empty } from './small';
import './lab.css';

/** True below the mobile breakpoint, where the analysis views give way to a notice. */
export function useNarrow(): boolean {
  const [n, setN] = useState(() => matchMedia('(max-width: 767px)').matches);
  useEffect(() => {
    const m = matchMedia('(max-width: 767px)');
    const on = () => setN(m.matches);
    m.addEventListener('change', on);
    return () => m.removeEventListener('change', on);
  }, []);
  return n;
}

export function NeedsLargerScreen({ name }: { name: string }) {
  return (
    <div className="page">
      <h1>{name}</h1>
      <Empty>This view needs a larger screen. Open <span className="mono">{location.pathname + location.search}</span> on a wider display.</Empty>
    </div>
  );
}

/** The "Not available from this data source" message every chart shows when a method's input is missing. */
export function Missing({ needs }: { needs: string }) {
  return <Empty>Not available from this data source: needs {needs}.</Empty>;
}

/** A chart with its title, one-line subtitle and the method button. */
export function ChartCard({ title, subtitle, method, job, children }: { title: string; subtitle?: ReactNode; method: string; job?: unknown; children: ReactNode }) {
  return (
    <section className="card" data-testid="chart-card">
      <div className="chart-head">
        <h2 style={{ margin: 0 }}>{title}</h2>
        <MethodButton id={method} job={job} />
      </div>
      {subtitle && <div className="sub muted" style={{ fontSize: 12, marginBottom: 8, maxWidth: '90ch' }}>{subtitle}</div>}
      {children}
    </section>
  );
}

export interface Key { color: string; label: string; line?: boolean }
export function Legend({ items }: { items: Key[] }) {
  return <div className="legend">{items.map((k) => <span key={k.label}><i className={k.line ? 'line' : ''} style={{ background: k.color }} />{k.label}</span>)}</div>;
}

/** The notes column: `n` toggles it (URL param `notes`). */
export function useNotes(): boolean {
  const [v, set] = useParam('notes', '1');
  const cur = useRef(v);
  cur.current = v;
  useEffect(() => {
    const on = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement;
      if (/^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName) || el.isContentEditable || e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key === 'n') set(cur.current === '1' ? '0' : null);
    };
    window.addEventListener('keydown', on);
    return () => window.removeEventListener('keydown', on);
  }, [set]);
  return v === '1';
}

export function Split({ notes, children, aside }: { notes: boolean; children: ReactNode; aside: ReactNode }) {
  return (
    <div className={`split${notes ? ' with-notes' : ''}`}>
      <div className="stack" style={{ minWidth: 0 }}>{children}</div>
      {notes && <aside className="notes" aria-label="Notes" data-testid="notes">{aside}</aside>}
    </div>
  );
}

/** A citation link: the registry's DOI link (or title search) for a reference key. */
export function Ref({ k, children }: { k: string; children: ReactNode }) {
  const { registry } = useApp();
  const r = registry?.refs.find((x) => x.key === k);
  return r ? <a href={r.url} target="_blank" rel="noreferrer" title={r.title}>{children}</a> : <>{children}</>;
}

/** A slider that moves freely and commits (to the URL and so to the engine) once it rests. */
export function Slider({ label, min, max, step, value, onCommit, fmt = (v) => String(v) }: { label: string; min: number; max: number; step: number; value: number; onCommit(v: number): void; fmt?(v: number): string }) {
  const [local, setLocal] = useState(value);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => setLocal(value), [value]);
  return (
    <label>
      {label}
      <input type="range" min={min} max={max} step={step} value={local} aria-label={label}
        onChange={(e) => {
          const v = Number(e.target.value);
          setLocal(v);
          clearTimeout(timer.current);
          timer.current = setTimeout(() => onCommit(v), 250);
        }} />
      <span className="val">{fmt(local)}</span>
    </label>
  );
}

export function Select<T extends string>({ label, value, options, onChange }: { label: string; value: T; options: [T, string][]; onChange(v: T): void }) {
  return (
    <label>
      {label}
      <select value={value} aria-label={label} onChange={(e) => onChange(e.target.value as T)}>
        {options.map(([v, t]) => <option key={v} value={v}>{t}</option>)}
      </select>
    </label>
  );
}

export const GOOD = '#5ee38b';
export const INK = 'var(--text)';
export const fmtInt = (v: number) => v.toLocaleString('en');
