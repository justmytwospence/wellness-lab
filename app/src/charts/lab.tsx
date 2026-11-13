import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Panel } from '../ui/Panel';
import { View } from '../ui/View';
import { InspectorNotes } from '../ui/Inspector';
import { useApp } from '../lib/app';
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
    <View title={name}>
      <Empty>This view needs a larger screen. Open <span className="mono">{location.pathname + location.search}</span> on a wider display.</Empty>
    </View>
  );
}

/** The "Not available from this data source" message every chart shows when a method's input is missing. */
export function Missing({ needs }: { needs: string }) {
  return <Empty>Not available from this data source: needs {needs}.</Empty>;
}

/** A chart with its title, one-line subtitle and the method button. */
export function ChartCard({ title, subtitle, method, job, children }: { title: string; subtitle?: ReactNode; method: string; job?: unknown; children: ReactNode }) {
  return <Panel title={title} sub={subtitle} method={method} job={job}>{children}</Panel>;
}

export interface Key { color: string; label: string; line?: boolean }
export function Legend({ items }: { items: Key[] }) {
  return <div className="legend">{items.map((k) => <span key={k.label}><i className={k.line ? 'line' : ''} style={{ background: k.color }} />{k.label}</span>)}</div>;
}

/** True while the inspector drawer is open. The Shell owns the `n` and `i` keys. */
export function useNotes(): boolean {
  return useApp().ui.inspector;
}

/** The view's main content, with its notes sent to the inspector drawer. */
export function Split({ children, aside }: { notes?: boolean; children: ReactNode; aside: ReactNode }) {
  return (
    <>
      {children}
      <InspectorNotes>{aside}</InspectorNotes>
    </>
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
