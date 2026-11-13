import { useEffect, useRef, type ReactNode } from 'react';
import { useParam } from '../lib/router';
import { Spark } from '../charts/small';

export interface FocusItem { id: string; title: string; group?: string; value?: string; spark?: number[]; na?: string | null; badge?: ReactNode }

/** A list of charts on the left, the chosen one filling the canvas; j and k step through it. The choice is URL param `c`. */
export function FocusList({ items, render, param = 'c', label = 'Charts' }: { items: FocusItem[]; render(item: FocusItem): ReactNode; param?: string; label?: string }) {
  const [sel, setSel] = useParam(param, items[0]?.id ?? '');
  const cur = items.find((i) => i.id === sel) ?? items[0];
  const live = useRef({ items, cur, setSel });
  live.current = { items, cur, setSel };
  const list = useRef<HTMLElement>(null);
  useEffect(() => {
    const on = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement;
      if (/^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName) || el.isContentEditable || e.metaKey || e.ctrlKey || e.altKey) return;
      const dir = e.key === 'j' || e.key === 'ArrowDown' ? 1 : e.key === 'k' || e.key === 'ArrowUp' ? -1 : 0;
      if (!dir) return;
      const { items: it, cur: c, setSel: set } = live.current;
      const i = it.findIndex((x) => x.id === c?.id) + dir;
      const next = it[i];
      if (!next) return;
      e.preventDefault();
      set(next.id);
      requestAnimationFrame(() => list.current?.querySelector('[aria-current="true"]')?.scrollIntoView({ block: 'nearest' }));
    };
    window.addEventListener('keydown', on);
    return () => window.removeEventListener('keydown', on);
  }, []);
  if (!cur) return null;
  let group: string | undefined;
  return (
    <div className="focus">
      <nav className="focus-list" aria-label={label} ref={list}>
        {items.map((it) => {
          const head = it.group !== group ? it.group : undefined;
          group = it.group;
          return (
            <div key={it.id}>
              {head && <h3>{head}</h3>}
              <button className="focus-item" data-testid="focus-item" data-id={it.id} data-na={it.na ? '1' : '0'} aria-current={it.id === cur.id} title={it.na ? `Not available: needs ${it.na}` : it.title} onClick={() => setSel(it.id)}>
                <span>{it.title}{it.badge}</span>
                {(it.value !== undefined || it.spark) && <span className="v">{it.value}{it.spark && <Spark values={it.spark.filter(Number.isFinite).slice(-90)} width={56} height={16} />}</span>}
              </button>
            </div>
          );
        })}
      </nav>
      <div className="focus-main">{render(cur)}</div>
    </div>
  );
}
