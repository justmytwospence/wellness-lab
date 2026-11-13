import { Suspense, useEffect, useRef, useState } from 'react';
import { Command } from 'cmdk';
import { Loader2, PanelRight, RefreshCw, Search, Waves } from 'lucide-react';
import { parseDay } from '@wl/views';
import { useApp } from '../lib/app';
import { link, navigate, useLoc } from '../lib/router';
import { ROUTES, WORKSPACES, routeFor } from '../routes';
import { ErrorBoundary } from '../charts/ErrorBoundary';
import { InspectorPanel, InspectorProvider } from '../ui/Inspector';
import { RangePicker } from '../ui/RangePicker';

function useKeys() {
  const app = useApp();
  // the listener is installed once and reads the app through a ref: re-installing it on every render (the status
  // bar ticks each second) lost the first half of a `g` chord
  const live = useRef(app);
  live.current = app;
  useEffect(() => {
    let g = false;
    let timer: ReturnType<typeof setTimeout>;
    const on = (e: KeyboardEvent) => {
      const { ui, lastDay } = live.current;
      const el = e.target as HTMLElement;
      if (/^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName) || el.isContentEditable) return;
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); ui.setPalette(!ui.palette); return; }
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key === '?') { ui.setHelp(!ui.help); return; }
      if (e.key === 'Escape') { ui.setMethod(null); ui.setHelp(false); ui.setPalette(false); return; }
      if (!g && (e.key === 'i' || e.key === 'n')) { ui.setInspector(!ui.inspector); return; }
      if (g) {
        g = false;
        clearTimeout(timer);
        e.stopPropagation();
        const r = ROUTES.find((x) => x.key === e.key);
        if (r) navigate(r.path.includes(':day') ? `/night/${lastDay}` : r.path);
        return;
      }
      if (e.key === 'g') { g = true; timer = setTimeout(() => (g = false), 1200); }
    };
    window.addEventListener('keydown', on, true);
    return () => window.removeEventListener('keydown', on, true);
  }, []);
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
              {ROUTES.map((r) => { const w = WORKSPACES.find((x) => x.id === r.ws)!; return <Command.Item key={r.path} value={`go ${w.name} ${r.name}`} onSelect={() => go(r.path.includes(':day') ? `/night/${lastDay}` : r.path)}><w.icon size={15} />{w.name} › {r.name}<span style={{ marginLeft: 'auto' }}><kbd>g</kbd> <kbd>{r.key}</kbd></span></Command.Item>; })}
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
  const rows: [string, string][] = [['Ctrl/Cmd K', 'Command palette'], ['?', 'This sheet'], ['i or n', 'Inspector: notes and methods'], ['j k', 'Next and previous chart in a list (Trends, Circadian, Events, Garmin) or card (Inbox)'], ['g then t n r y d h x b c i e m k o a p s f u', 'Go to a view (t today, n night, r nights, y typical, d trends, h HRV, x thresholds, b beats, c circadian, i inbox, e events, m compare, k bench, o models, a garmin, p experiments, s sql, f methods, u data)'], ['[  ]', 'Previous and next night'], ['a', 'Toggle onset and clock alignment (Nights, Typical)'], ['l', 'Label this night'], ['c d u s e', 'Inbox: confirm, deny, not sure, snooze, edit'], ['j k', 'Inbox: next and previous card'], ['Esc', 'Close panels']];
  return (
    <div className="overlay" onClick={() => ui.setHelp(false)}>
      <div className="dialog" onClick={(e) => e.stopPropagation()} role="dialog" aria-label="Keyboard shortcuts">
        <h1>Keyboard</h1>
        <table className="t"><tbody>{rows.map(([k, d]) => <tr key={k}><td className="mono" style={{ whiteSpace: 'nowrap' }}>{k}</td><td>{d}</td></tr>)}</tbody></table>
      </div>
    </div>
  );
}

function Login() {
  const [token, setToken] = useState('');
  const [err, setErr] = useState('');
  const go = async () => {
    const r = await fetch('/api/login', { method: 'POST', credentials: 'same-origin', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ token }) });
    if (r.ok) location.reload();
    else setErr('That token was not accepted.');
  };
  return (
    <div className="overlay" style={{ alignItems: 'center', padding: 0 }}>
      <form className="dialog" style={{ width: 360 }} onSubmit={(e) => { e.preventDefault(); void go(); }}>
        <h1>Wellness Lab</h1>
        <p className="muted">Enter the access token for this server (LAB_TOKEN).</p>
        <input type="password" autoFocus value={token} onChange={(e) => setToken(e.target.value)} aria-label="Access token" style={{ width: '100%', marginBottom: 10 }} />
        {err && <p className="err">{err}</p>}
        <button className="primary" type="submit">Sign in</button>
      </form>
    </div>
  );
}

const SPIN = { animation: 'spin 1s linear infinite' } as const;

/** The path a workspace opens to: the last view visited in it, else its first. */
function wsHome(id: string, lastDay: number): string {
  const saved = sessionStorage.getItem('wl.ws.' + id);
  const first = id === 'sleep' ? '/nights' : ROUTES.find((r) => r.ws === id)!.path;
  const p = saved ?? first;
  return p.includes(':day') ? `/night/${lastDay}` : p;
}

function ShellBody() {
  const app = useApp();
  const loc = useLoc();
  useKeys();
  const route = routeFor(loc.path);
  const ws = WORKSPACES.find((w) => w.id === route?.ws);
  const open = app.state.proposals.filter((p) => p.status === 'open' && p.asked).length;
  const [syncing, setSyncing] = useState(false);
  const [, tick] = useState(0);
  useEffect(() => {
    if (route) sessionStorage.setItem('wl.ws.' + route.ws, loc.path);
  }, [route, loc.path]);
  useEffect(() => {
    if (!app.busy) return;
    const t = setInterval(() => tick((n) => n + 1), 1000);
    return () => clearInterval(t);
  }, [app.busy]);
  const tabs = ws ? ROUTES.filter((r) => r.ws === ws.id) : [];
  const status = app.loading
    ? { c: 'var(--base)', t: `Loading files ${app.loading.done}/${app.loading.total}` }
    : !app.storageOk && !app.demo
      ? { c: 'var(--alert)', t: 'This page cannot store data: open it over HTTPS' }
      : app.demo
        ? { c: 'var(--hr)', t: 'Demo data, kept in memory' }
        : app.manifest
          ? { c: app.mode === 'server' ? 'var(--hrv)' : 'var(--base)', t: `${app.mode === 'server' ? 'garmin-mcp' : 'Imported'} · data from ${app.manifest.generated_at.slice(0, 10)}` }
          : { c: 'var(--faint)', t: 'No data' };
  if (app.needsLogin) return <Login />;
  return (
    <div className="shell">
      <nav className="rail" aria-label="Workspaces">
        <div className="brand" title="Wellness Lab"><Waves size={20} /></div>
        {WORKSPACES.filter((w) => !w.foot).map((w) => (
          <a key={w.id} href={wsHome(w.id, app.lastDay)} className={ws?.id === w.id ? 'active' : ''} title={w.name} onClick={link(wsHome(w.id, app.lastDay))}>
            <w.icon size={18} /><span>{w.name}</span>{w.id === 'journal' && open > 0 && <span className="badge">{open}</span>}
          </a>
        ))}
        <div className="spacer" />
        {WORKSPACES.filter((w) => w.foot).map((w) => (
          <a key={w.id} href={wsHome(w.id, app.lastDay)} className={ws?.id === w.id ? 'active' : ''} title={w.name} onClick={link(wsHome(w.id, app.lastDay))}>
            <w.icon size={18} /><span>{w.name}</span>
          </a>
        ))}
      </nav>
      <header className="header">
        {tabs.length > 1 ? (
          <nav className="tabs" aria-label="Views">
            {tabs.map((r) => {
              const to = r.path.includes(':day') ? `/night/${app.lastDay}` : r.path;
              return <a key={r.path} href={to} title={r.name} className={route?.path === r.path ? 'on' : ''} onClick={link(to)}>{r.name}</a>;
            })}
          </nav>
        ) : <span className="ws-name">{ws?.name ?? 'Wellness Lab'}</span>}
        <span className="spacer" />
        {route?.range && <RangePicker />}
        {app.mode === 'server' && <button className="icon ghost" aria-label="Sync now" title="Sync now" disabled={syncing} onClick={() => { setSyncing(true); void app.syncNow().finally(() => setSyncing(false)); }}><RefreshCw size={14} style={syncing ? SPIN : undefined} /></button>}
        <button onClick={() => app.ui.setPalette(true)} aria-label="Command palette" style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}><Search size={13} /> Search <kbd>⌘K</kbd></button>
        <button className="icon ghost" aria-label="Inspector" aria-pressed={app.ui.inspector} title="Inspector (i)" onClick={() => app.ui.setInspector(!app.ui.inspector)} style={app.ui.inspector ? { background: 'var(--hover)' } : undefined}><PanelRight size={15} /></button>
      </header>
      <main className="main">
        <ErrorBoundary name={route?.name} key={loc.path}>
          <Suspense fallback={<div className="empty">Loading...</div>}>
            {route ? <route.view /> : <div className="empty">No such page. <a href="/" onClick={link('/')}>Go to Today</a></div>}
          </Suspense>
        </ErrorBoundary>
      </main>
      <InspectorPanel />
      <footer className="statusbar">
        <span><i className="dot" style={{ background: status.c }} />{status.t}</span>
        {app.busy && <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}><Loader2 size={12} style={SPIN} />Computing {app.busy.kinds.join(', ')} · {Math.max(0, Math.round((Date.now() - app.busy.since) / 1000))}s</span>}
        <span className="spacer" />
        <button className="ghost" onClick={() => app.ui.setHelp(true)}>Shortcuts <kbd>?</kbd></button>
      </footer>
      <Palette />
      <Help />
      {app.toasts.map((t) => <div className="toast" key={t.id} role="status">{t.text}{t.undo && <button onClick={() => { t.undo!(); }}>Undo</button>}</div>)}
    </div>
  );
}

export function Shell() {
  return <InspectorProvider><ShellBody /></InspectorProvider>;
}
