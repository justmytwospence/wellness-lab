import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { EngineClient } from '@wl/engine';
import { LocalStateStore, ServerStateStore, type Answer, type StateStore } from '@wl/store';
import { DEFAULT_SETTINGS, type ExperimentDef, type Label, type Manifest, type Registry, type Settings, type State, type Table } from '@wl/schema';
import { parseRange, type Range } from '@wl/views';
import { LAB_EMPTY } from './util';
import { useParam } from './router';
import { useInspector } from './prefs';

export type Mode = 'booting' | 'local' | 'server';
export interface Toast { id: number; text: string; undo?: () => void }

export interface AppCtx {
  engine: EngineClient;
  mode: Mode;
  needsLogin: boolean;
  state: State;
  manifest: Manifest | null;
  /** Bumps whenever data or state change: effects that run jobs depend on it. */
  version: number;
  loading: { done: number; total: number } | null;
  demo: boolean;
  registry: Registry | null;
  lastDay: number;
  firstDay: number;
  range: Range;
  rangeText: string;
  setRangeText(s: string): void;
  today: number;
  store: StateStore;
  answer(id: string, a: Answer, label?: Partial<Label>): Promise<void>;
  saveLabel(l: Partial<Label> & Pick<Label, 'kind' | 'start_day' | 'end_day'>): Promise<void>;
  updateLabel(l: Label): Promise<void>;
  deleteLabel(id: string): Promise<void>;
  saveSettings(s: Settings): Promise<void>;
  saveExperiment(e: ExperimentDef): Promise<void>;
  deleteExperiment(id: string): Promise<void>;
  setAdherence(id: string, day: number, adherent: boolean): Promise<void>;
  refreshData(): Promise<void>;
  loadDemo(): Promise<void>;
  syncNow(): Promise<void>;
  toast(text: string, undo?: () => void): void;
  toasts: Toast[];
  /** engine jobs in flight (null when idle) */
  busy: { kinds: string[]; since: number } | null;
  /** false when the browser cannot keep files (no secure context or no OPFS) */
  storageOk: boolean;
  nights: Table | null;
  ui: { palette: boolean; help: boolean; method: string | null; inspector: boolean; setPalette(b: boolean): void; setHelp(b: boolean): void; setMethod(id: string | null): void; setInspector(b: boolean): void };
}

const Ctx = createContext<AppCtx>(null as never);
export const useApp = () => useContext(Ctx);

const emptyState = (): State => ({ labels: [], proposals: [], settings: { ...DEFAULT_SETTINGS }, experiments: [], adherence: {}, revision: 0 });

export function AppProvider({ children }: { children: ReactNode }) {
  const engine = useMemo(() => new EngineClient(), []);
  const [mode, setMode] = useState<Mode>('booting');
  const [needsLogin, setNeedsLogin] = useState(false);
  const [store, setStore] = useState<StateStore>(() => new LocalStateStore());
  const [state, setState] = useState<State>(emptyState());
  const [manifest, setManifest] = useState<Manifest | null>(null);
  const [version, setVersion] = useState(0);
  const [loading, setLoading] = useState<{ done: number; total: number } | null>(null);
  const [demo, setDemo] = useState(false);
  const [registry, setRegistry] = useState<Registry | null>(null);
  const [nights, setNights] = useState<Table | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [palette, setPalette] = useState(false);
  const [help, setHelp] = useState(false);
  const [method, setMethod] = useState<string | null>(null);
  const [inspector, setInspector] = useInspector();
  const [busy, setBusy] = useState<{ kinds: string[]; since: number } | null>(null);
  const storageOk = useMemo(() => window.isSecureContext && typeof navigator.storage?.getDirectory === 'function', []);
  const [rangeText, setRangeText] = useParam('range', '180d');
  useEffect(() => {
    engine.onBusy = (kinds) => setBusy((b) => (kinds.length === 0 ? null : { kinds, since: b?.since ?? Date.now() }));
  }, [engine]);
  const storeRef = useRef(store);
  storeRef.current = store;
  const stateRef = useRef(state);
  stateRef.current = state;

  const toast = useCallback((text: string, undo?: () => void) => {
    const id = Date.now() + Math.random();
    setToasts((t) => [...t, { id, text, undo }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), undo ? 10_000 : 4000);
  }, []);

  /** Push state into the engine, refresh the night table, and (locally) detect episodes. */
  const settle = useCallback(async (s: State, detect: boolean) => {
    await engine.setState(s);
    const n = (await engine.run({ kind: 'nights' })) as unknown as Table;
    setNights(n);
    if (detect && n.n > 30 && mode !== 'server') {
      const d = await engine.run({ kind: 'detect' });
      const st = storeRef.current;
      await st.saveProposals(d.proposals);
      // certain exposures (time zone, altitude) are labelled on their own, with a one-click undo
      let cur = await st.load();
      for (const p of cur.proposals.filter((p) => p.auto && p.status === 'open')) await st.answer(p.id, 'confirm');
      cur = await st.load();
      await engine.setState(cur);
      setState(cur);
      setVersion((v) => v + 1);
      return;
    }
    setState(s);
    setVersion((v) => v + 1);
  }, [engine, mode]);

  const refreshData = useCallback(async () => {
    engine.onProgress = (p) => setLoading({ done: p.done, total: p.total });
    setLoading({ done: 0, total: 1 });
    try {
      const m = await engine.load();
      setManifest(m);
      setDemo(false);
      const s = await storeRef.current.load();
      await settle(s, true);
    } finally {
      setLoading(null);
    }
  }, [engine, settle]);

  const loadDemo = useCallback(async () => {
    const labels = await engine.demo(42, 730);
    setDemo(true);
    setManifest(null);
    // demo labels are real ground truth for the demo's detector; keep them in memory only
    const s: State = { ...emptyState(), labels: labels.filter((l) => l.kind !== 'illness' || l.start_day % 2 === 0) };
    await engine.setState(s);
    const n = (await engine.run({ kind: 'nights' })) as unknown as Table;
    setNights(n);
    const d = await engine.run({ kind: 'detect' });
    // the demo keeps its labels and answers in memory, never in the browser's real state
    const files = new Map<string, string>();
    const mem = new LocalStateStore(async (p) => { const t = files.get(p.join('/')); return t ? (new TextEncoder().encode(t).buffer as ArrayBuffer) : null; }, async (p, t) => void files.set(p.join('/'), t));
    for (const l of s.labels) await mem.createLabel(l);
    await mem.saveProposals(d.proposals);
    storeRef.current = mem;
    setStore(mem);
    const s2 = await mem.load();
    await engine.setState(s2);
    setState(s2);
    setVersion((v) => v + 1);
  }, [engine]);

  // boot: server or local mode, registry, data
  useEffect(() => {
    let dead = false;
    (async () => {
      let m: Mode = 'local';
      try {
        const r = await fetch('/api/info', { credentials: 'same-origin' });
        if (r.ok && (r.headers.get('content-type') ?? '').includes('json')) {
          const info = (await r.json()) as { mode?: string; auth?: boolean; authed?: boolean };
          if (info.mode === 'server') { m = 'server'; if (info.auth && !info.authed) setNeedsLogin(true); }
        }
      } catch { /* local mode */ }
      if (dead) return;
      const st = m === 'server' ? new ServerStateStore() : new LocalStateStore();
      storeRef.current = st;
      setStore(st);
      setMode(m);
      engine.run({ kind: 'registry' }).then((r) => !dead && setRegistry(r as unknown as Registry)).catch(() => {});
    })();
    return () => { dead = true; };
  }, [engine]);

  useEffect(() => {
    if (mode === 'booting' || needsLogin || !storageOk) return;
    (async () => {
      if (mode === 'server') {
        try {
          const { pullWellness } = await import('@wl/ingest');
          const { opfsFiles } = await import('@wl/ingest');
          setLoading({ done: 0, total: 1 });
          await pullWellness({ base: '/data/wellness-v1', files: opfsFiles, emit: (p) => setLoading({ done: p.done, total: p.total }) });
        } catch (e) {
          if (String((e as Error).message).includes('sign-in')) { setNeedsLogin(true); return; }
          toast(`Could not pull data: ${(e as Error).message}`);
        }
      }
      await refreshData();
      if (new URLSearchParams(location.search).get('demo') === '1') await loadDemo();
    })().catch((e) => toast(String((e as Error).message ?? e)));
  }, [mode, needsLogin, storageOk, refreshData, toast, loadDemo]);

  const wrap = useCallback(async (f: () => Promise<unknown>) => {
    await f();
    const s = await storeRef.current.load();
    await settle(s, false);
  }, [settle]);

  const answer = useCallback(async (id: string, a: Answer, label?: Partial<Label>) => {
    const r = await storeRef.current.answer(id, a, label);
    await wrap(async () => {});
    const verb = { confirm: 'Confirmed', deny: 'Denied', unsure: 'Marked unsure', snooze: 'Snoozed for 7 days' }[a];
    toast(verb, () => { void storeRef.current.undo(id, r.label?.id).then(() => wrap(async () => {})); });
  }, [wrap, toast]);

  const syncNow = useCallback(async () => {
    if (mode === 'server') {
      await fetch('/api/sync', { method: 'POST', credentials: 'same-origin' });
      const { pullWellness, opfsFiles } = await import('@wl/ingest');
      await pullWellness({ base: '/data/wellness-v1', files: opfsFiles });
    }
    await refreshData();
  }, [mode, refreshData]);

  const days = (nights?.num.day ?? []) as number[];
  const lastDay = days.length ? (days[days.length - 1] as number) : Math.floor(Date.now() / 86_400_000);
  const firstDay = days.length ? (days[0] as number) : lastDay - 365;
  const range = parseRange(rangeText, lastDay, firstDay) ?? [lastDay - 179, lastDay];

  const value: AppCtx = {
    engine, mode, needsLogin, state, manifest, version, loading, demo, registry, lastDay, firstDay, range, rangeText, setRangeText: (s) => setRangeText(s), today: Math.floor(Date.now() / 86_400_000),
    store, answer, nights,
    saveLabel: (l) => wrap(() => storeRef.current.createLabel(l)),
    updateLabel: (l) => wrap(() => storeRef.current.updateLabel(l)),
    deleteLabel: (id) => wrap(() => storeRef.current.deleteLabel(id)),
    saveSettings: (s) => wrap(() => storeRef.current.saveSettings(s)),
    saveExperiment: (e) => wrap(() => storeRef.current.saveExperiment(e)),
    deleteExperiment: (id) => wrap(() => storeRef.current.deleteExperiment(id)),
    setAdherence: (id, day, adherent) => wrap(() => storeRef.current.setAdherence(id, day, adherent)),
    refreshData, loadDemo, syncNow, toast, toasts, busy, storageOk,
    ui: { palette, help, method, inspector, setPalette, setHelp, setMethod, setInspector },
  };
  void LAB_EMPTY;
  if (import.meta.env.DEV || new URLSearchParams(location.search).get('debug') === '1') (window as unknown as { __wl: AppCtx }).__wl = value;
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}
