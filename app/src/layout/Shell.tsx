import { Suspense, useEffect, useState } from 'react';
import { Command } from 'cmdk';
import { Command as CmdIcon, PanelLeft } from 'lucide-react';
import { parseRange, formatRange, parseDay } from '@wl/views';
import { useApp } from '../lib/app';
import { link, navigate, useLoc } from '../lib/router';
import { ROUTES, routeFor } from '../routes';
import { ErrorBoundary } from '../charts/ErrorBoundary';
import { MethodPanel } from './MethodPanel';

function useKeys() {
  const { ui, lastDay, state } = useApp();
  useEffect(() => {
    let g = false;
    let timer: ReturnType<typeof setTimeout>;
    const on = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement;
      if (/^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName) || el.isContentEditable) return;
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); ui.setPalette(!ui.palette); return; }
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key === '?') { ui.setHelp(!ui.help); return; }
      if (e.key === 'Escape') { ui.setMethod(null); ui.setHelp(false); ui.setPalette(false); return; }
      if (e.key === '\\') { document.querySelector('.shell')?.classList.toggle('collapsed'); return; }
      if (g) {
        g = false;
        clearTimeout(timer);
        const r = ROUTES.find((x) => x.key === e.key);
        if (r) navigate(r.path.includes(':day') ? `/night/${lastDay}` : r.path);
        return;
      }
      if (e.key === 'g') { g = true; timer = setTimeout(() => (g = false), 1200); }
    };
    window.addEventListener('keydown', on);
    return () => window.removeEventListener('keydown', on);
  }, [ui, lastDay, state]);
}

function Palette() {
  const { ui, registry, lastDay, loadDemo, syncNow } = useApp();
  const [q, setQ] = useState('');
  if (!ui.palette) return null;
  const go = (to: string) => { ui.setPalette(false); setQ(''); navigate(to); };
  const day = /^\d{4}-\d{2}-\d{2}$/.test(q) ? parseDay(q) : null;
  return (
    <div className="overlay" onClick={() => ui.setPalette(false)}>
      <div onClick={(e) => e.stopPropagation()}>
        <Command label="Command palette" shouldFilter>
          <Command.Input autoFocus value={q} onValueChange={setQ} placeholder="Go to a view, a night (2026-11-04), a metric, a method..." />
          <Command.List>
            <Command.Empty>Nothing matches.</Command.Empty>
            {day !== null && <Command.Group heading="Night"><Command.Item value={`night ${q}`} onSelect={() => go(`/night/${day}`)}>Go to night {q}</Command.Item></Command.Group>}
            <Command.Group heading="Go to">
              {ROUTES.filter((r) => r.rail).map((r) => <Command.Item key={r.path} value={`go ${r.name}`} onSelect={() => go(r.path)}><r.icon size={15} />{r.name}<span style={{ marginLeft: 'auto' }}><kbd>g</kbd> <kbd>{r.key}</kbd></span></Command.Item>)}
              <Command.Item value="go last night" onSelect={() => go(`/night/${lastDay}`)}>Last night</Command.Item>
            </Command.Group>
            <Command.Group heading="Actions">
              <Command.Item value="sync now" onSelect={() => { ui.setPalette(false); void syncNow(); }}>Sync now</Command.Item>
              <Command.Item value="import export" onSelect={() => go('/data')}>Import Garmin export...</Command.Item>
              <Command.Item value="load demo data" onSelect={() => { ui.setPalette(false); void loadDemo(); }}>Load demo data</Command.Item>
            </Command.Group>
            <Command.Group heading="Nights raster">
              {['hr', 'hrv5', 'stress', 'resp', 'spo2', 'stage', 'steps', 'motion', 'altitude'].map((m) => <Command.Item key={m} value={`raster ${m}`} onSelect={() => go(`/nights?metric=${m}`)}>Nights raster: {m}</Command.Item>)}
            </Command.Group>
            <Command.Group heading="Methods">
              {registry?.methods.map((m) => <Command.Item key={m.id} value={`method ${m.name} ${m.id}`} onSelect={() => { ui.setPalette(false); ui.setMethod(m.id); }}>Open method: {m.name}</Command.Item>)}
            </Command.Group>
          </Command.List>
        </Command>
      </div>
    </div>
  );
}

function Help() {
  const { ui } = useApp();
  if (!ui.help) return null;
  const rows: [string, string][] = [['Ctrl/Cmd K', 'Command palette'], ['?', 'This sheet'], ['g then t n r y d h x b c i e m k o a p s f u', 'Go to a view (t today, n night, r nights, y typical, d trends, h HRV, x thresholds, b beats, c circadian, i inbox, e events, m compare, k bench, o models, a garmin, p experiments, s sql, f methods, u data)'], ['[  ]', 'Previous and next night'], ['a', 'Toggle onset and clock alignment (Nights, Typical)'], ['n', 'Toggle notes (HRV, Thresholds)'], ['l', 'Label this night'], ['c d u s e', 'Inbox: confirm, deny, not sure, snooze, edit'], ['j k', 'Inbox: next and previous card'], ['\\', 'Collapse the left rail'], ['Esc', 'Close panels']];
  return (
    <div className="overlay" onClick={() => ui.setHelp(false)}>
      <div className="dialog" onClick={(e) => e.stopPropagation()} role="dialog" aria-label="Keyboard shortcuts">
        <h1>Keyboard</h1>
        <table className="t"><tbody>{rows.map(([k, d]) => <tr key={k}><td className="mono" style={{ whiteSpace: 'nowrap' }}>{k}</td><td>{d}</td></tr>)}</tbody></table>
      </div>
    </div>
  );
}

export function Shell() {
  const app = useApp();
  const loc = useLoc();
  useKeys();
  const route = routeFor(loc.path);
  const open = app.state.proposals.filter((p) => p.status === 'open' && p.asked).length;
  const [text, setText] = useState(app.rangeText);
  useEffect(() => setText(app.rangeText), [app.rangeText]);
  const apply = () => {
    const r = parseRange(text, app.lastDay, app.firstDay);
    if (r) app.setRangeText(text);
    else app.toast(`Could not read "${text}" as a date range`);
  };
  const fresh = app.manifest ? `${app.demo ? 'demo' : `data from ${app.manifest.generated_at.slice(0, 10)}`}` : app.demo ? 'demo data' : 'no data';
  return (
    <div className="shell">
      <nav className="rail" aria-label="Views">
        <div className="brand"><PanelLeft size={18} onClick={() => document.querySelector('.shell')?.classList.toggle('collapsed')} style={{ cursor: 'pointer' }} /><span>Wellness Lab</span></div>
        {ROUTES.filter((r) => r.rail).map((r) => {
          const on = route?.path === r.path || (r.path === '/nights' && route?.path === '/night/:day');
          return (
            <a key={r.path} href={r.path} className={on ? 'active' : ''} onClick={link(r.path)} title={r.name}>
              <r.icon size={16} /><span className="label">{r.name}</span>{r.path === '/inbox' && open > 0 && <span className="badge">{open}</span>}
            </a>
          );
        })}
      </nav>
      <header className="topbar">
        <input value={text} onChange={(e) => setText(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && apply()} onBlur={apply} aria-label="Date range" placeholder="last 180 days, 2025, summer 2024..." />
        <span className="muted mono" style={{ fontSize: 12 }}>{formatRange(app.range)}</span>
        <span className="spacer" />
        <span className="muted" style={{ fontSize: 12 }}>{app.loading ? `loading ${app.loading.done}/${app.loading.total}` : fresh}</span>
        <button onClick={() => app.ui.setPalette(true)} aria-label="Command palette"><CmdIcon size={13} /> K</button>
      </header>
      <main className="main">
        <ErrorBoundary name={route?.name} key={loc.path}>
          <Suspense fallback={<div className="empty">Loading...</div>}>
            {route ? <route.view /> : <div className="empty">No such page. <a href="/" onClick={link('/')}>Go to Today</a></div>}
          </Suspense>
        </ErrorBoundary>
      </main>
      <nav className="mobile-bar" aria-label="Main">
        {ROUTES.filter((r) => r.mobile).map((r) => <a key={r.path} href={r.path} onClick={link(r.path)}>{r.name}</a>)}
      </nav>
      <Palette />
      <Help />
      <MethodPanel />
      {app.toasts.map((t) => <div className="toast" key={t.id} role="status">{t.text}{t.undo && <button onClick={() => { t.undo!(); }}>Undo</button>}</div>)}
    </div>
  );
}
