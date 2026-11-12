/// <reference lib="webworker" />
/** The engine worker: owns the wasm `Lab`, loads wellness-v1 files from OPFS into it, and runs jobs. */
import init, { Lab, synth, synthLabels } from '../pkg/wellness_lab.js';
import { readFile } from '@wl/store';
import type { Manifest } from '@wl/schema';
import { concat, plan, readColumns } from './loader';

export type Req =
  | { id: number; op: 'load' }
  | { id: number; op: 'state'; labels: string; proposals: string; settings: string }
  | { id: number; op: 'run'; job: string }
  | { id: number; op: 'demo'; seed: number; days: number }
  | { id: number; op: 'registry' };
export type ReqBody = Req extends infer R ? (R extends unknown ? Omit<R, 'id'> : never) : never;
export type Res =
  | { id: number; ok: true; json?: string; arrays?: Record<string, ArrayBufferView>; manifest?: Manifest | null }
  | { id: number; ok: false; error: string }
  | { progress: { done: number; total: number; file: string } };

let lab: Lab | null = null;
let ready: Promise<unknown> | null = null;
const root = ['lab', 'wellness-v1'];

async function manifest(): Promise<Manifest | null> {
  const b = await readFile([...root, 'manifest.json']);
  return b ? (JSON.parse(new TextDecoder().decode(b)) as Manifest) : null;
}

async function load(): Promise<Manifest | null> {
  const m = await manifest();
  const next = new Lab();
  if (!m) {
    lab = next;
    return null;
  }
  const { tables, series, rr } = plan(m);
  const total = tables.length + [...series.values()].reduce((n, l) => n + l.length, 0) + rr.length;
  let done = 0;
  const tick = (file: string) => postMessage({ progress: { done: ++done, total, file } } satisfies Res);
  const get = async (path: string) => {
    const b = await readFile([...root, ...path.split('/')]);
    if (!b) throw new Error(`missing ${path}`);
    return readColumns(b);
  };
  for (const p of tables) {
    const c = await get(p);
    next.loadTable(p.replace(/\.parquet$/, ''), c.names, c.numeric, JSON.stringify(c.strings));
    tick(p);
  }
  for (const [name, files] of series) {
    const parts = [];
    for (const f of files) {
      parts.push(await get(f));
      tick(f);
    }
    const c = concat(parts);
    const ti = c.names.indexOf('t');
    const names = c.names.filter((n) => n !== 't');
    next.loadSeries(name, c.numeric[ti]!, names, names.map((n) => c.numeric[c.names.indexOf(n)]!));
  }
  for (const p of rr) {
    const c = await get(p);
    const id = p.replace(/^rr\//, '').replace(/\.parquet$/, '');
    next.loadRr(id, Float32Array.from(c.numeric[c.names.indexOf('rr')]!));
    tick(p);
  }
  lab = next;
  return m;
}

self.onmessage = async (e: MessageEvent<Req>) => {
  const r = e.data;
  try {
    ready ??= init();
    await ready;
    if (r.op === 'load') {
      const m = await load();
      postMessage({ id: r.id, ok: true, manifest: m } satisfies Res);
    } else if (r.op === 'demo') {
      lab = synth(r.seed, r.days);
      postMessage({ id: r.id, ok: true, json: synthLabels(r.seed, r.days) } satisfies Res);
    } else if (r.op === 'state') {
      lab ??= new Lab();
      lab.setLabels(r.labels);
      lab.setProposals(r.proposals);
      lab.setSettings(r.settings);
      postMessage({ id: r.id, ok: true } satisfies Res);
    } else if (r.op === 'registry') {
      const { registry } = await import('../pkg/wellness_lab.js');
      postMessage({ id: r.id, ok: true, json: registry() } satisfies Res);
    } else {
      lab ??= new Lab();
      const res = lab.run(r.job);
      const arrays: Record<string, ArrayBufferView> = {};
      for (const name of res.names() as string[]) {
        for (const t of ['u8', 'f64', 'f32', 'i32'] as const) {
          try {
            arrays[name] = res[t](name) as ArrayBufferView;
            break;
          } catch {
            /* wrong type, try the next */
          }
        }
      }
      const json = res.json();
      res.free();
      postMessage({ id: r.id, ok: true, json, arrays } satisfies Res, Object.values(arrays).map((a) => a.buffer as ArrayBuffer));
    }
  } catch (err) {
    postMessage({ id: r.id, ok: false, error: String((err as Error)?.message ?? err) } satisfies Res);
  }
};
