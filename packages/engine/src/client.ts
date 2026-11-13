import type { Job, Label, Manifest, Proposal, ResultOf, RunArrays, Settings } from '@wl/schema';
import { readFile, writeFile } from '@wl/store';
import { createMemo } from './memo';
import type { ReqBody, Res } from './worker';

export interface LoadProgress { done: number; total: number; file: string }
export type Run<J extends Job> = ResultOf<J> & { arrays: RunArrays };

/**
 * Jobs that take a minute on a real account: they run in a second worker so the views stay responsive, and their
 * results are kept in OPFS per data version, so they are computed once per sync rather than once per visit.
 */
const HEAVY = new Set(['dfa']);
const CACHE_DIR = ['lab', 'cache'];

type Ok = Extract<Res, { ok: true }>;
const ARRAY_TYPES = { Uint8Array, Float64Array, Float32Array, Int32Array } as const;

/** A worker result as JSON (typed arrays tagged and base64-encoded), for the OPFS cache. */
function pack(r: Ok): string {
  const arrays: Record<string, [string, string]> = {};
  for (const [k, v] of Object.entries(r.arrays ?? {})) {
    const b = new Uint8Array(v.buffer, v.byteOffset, v.byteLength);
    let s = '';
    for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode(...b.subarray(i, i + 0x8000));
    arrays[k] = [v.constructor.name, btoa(s)];
  }
  return JSON.stringify({ json: r.json, arrays });
}
function unpack(text: string): Ok {
  const o = JSON.parse(text) as { json: string; arrays: Record<string, [keyof typeof ARRAY_TYPES, string]> };
  const arrays: Record<string, ArrayBufferView> = {};
  for (const [k, [type, b64]] of Object.entries(o.arrays)) {
    const bin = atob(b64);
    const b = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) b[i] = bin.charCodeAt(i);
    arrays[k] = new ARRAY_TYPES[type](b.buffer);
  }
  return { id: 0, ok: true, json: o.json, arrays };
}
async function hash(s: string): Promise<string> {
  const d = new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(s)));
  return [...d.slice(0, 12)].map((x) => x.toString(16).padStart(2, '0')).join('');
}

/** The app's handle on the engine worker: loading, state and memoised jobs. */
export class EngineClient {
  private worker: Worker;
  private next = 1;
  private pending = new Map<number, { resolve: (r: Ok) => void; reject: (e: Error) => void }>();
  private memo = createMemo(64 * 1024 * 1024);
  private version = 'empty';
  private revision = 0;
  /** what the main worker was last loaded with and its state, replayed into the side worker */
  private source: ReqBody | null = null;
  private stateReq: ReqBody | null = null;
  private side: Promise<EngineClient> | null = null;
  onProgress: (p: LoadProgress) => void = () => {};
  /** Called with the kinds of the jobs in flight whenever that set changes. */
  onBusy: (kinds: string[]) => void = () => {};
  private inflight = new Map<number, string>();
  private jobId = 0;
  private emitBusy() { this.onBusy([...new Set(this.inflight.values())]); }
  manifest: Manifest | null = null;

  constructor(worker?: Worker) {
    this.worker = worker ?? new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });
    this.worker.onmessage = (e: MessageEvent<Res>) => {
      const m = e.data;
      if ('progress' in m) return this.onProgress(m.progress);
      const p = this.pending.get(m.id);
      if (!p) return;
      this.pending.delete(m.id);
      if (m.ok) p.resolve(m);
      else p.reject(new Error(m.error));
    };
  }

  private call(req: ReqBody): Promise<Ok> {
    const id = this.next++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.worker.postMessage({ ...req, id });
    });
  }

  private resetSide() {
    void this.side?.then((s) => s.terminate());
    this.side = null;
  }

  /** The second worker, loaded with the same data and state as the first (on first use). */
  private sideClient(): Promise<EngineClient> {
    this.side ??= (async () => {
      const s = new EngineClient();
      if (this.source) await s.call(this.source);
      if (this.stateReq) await s.call(this.stateReq);
      return s;
    })();
    return this.side;
  }

  /** Load the OPFS `lab/wellness-v1` files into the engine. */
  async load(): Promise<Manifest | null> {
    this.resetSide();
    const r = await this.call({ op: 'load' });
    this.source = { op: 'load' };
    this.manifest = r.manifest ?? null;
    this.version = this.manifest?.generated_at ?? 'empty';
    this.memo.clear();
    return this.manifest;
  }

  /** Replace the data with synthetic demo data; returns its true labels. */
  async demo(seed = 42, days = 730): Promise<Label[]> {
    this.resetSide();
    const r = await this.call({ op: 'demo', seed, days });
    this.source = { op: 'demo', seed, days };
    this.version = `demo:${seed}:${days}`;
    this.memo.clear();
    return JSON.parse(r.json!) as Label[];
  }

  async setState(s: { labels: Label[]; proposals: Proposal[]; settings: Settings }): Promise<void> {
    const req: ReqBody = { op: 'state', labels: JSON.stringify(s.labels), proposals: JSON.stringify(s.proposals), settings: JSON.stringify(s.settings) };
    await this.call(req);
    this.stateReq = req;
    if (this.side) await (await this.side).call(req);
    this.revision++;
    this.memo.clear();
  }

  /** A heavy job: from the OPFS cache for this data version, else computed in the side worker and cached. */
  private async heavy(text: string): Promise<Ok> {
    const persist = this.version !== 'empty' && !this.version.startsWith('demo:');
    // one file per job, overwritten when the data or the settings change, so the cache never grows
    const file = persist ? [...CACHE_DIR, `${await hash(text)}.json`] : null;
    const key = `${this.version}#${this.stateReq && 'settings' in this.stateReq ? this.stateReq.settings : ''}`;
    if (file) {
      try {
        const b = await readFile(file);
        if (b) {
          const o = JSON.parse(new TextDecoder().decode(b)) as { key: string; result: string };
          if (o.key === key) return unpack(o.result);
        }
      } catch { /* a damaged cache entry is recomputed */ }
    }
    const r = await (await this.sideClient()).call({ op: 'run', job: text });
    if (file) void writeFile(file, JSON.stringify({ key, result: pack(r) })).catch(() => {});
    return r;
  }

  /** Run a job. Results are memoised by job and data version; arrays come back as typed arrays under `arrays`. */
  run<J extends Job>(job: J, opts: { alive?: () => boolean } = {}): Promise<Run<J>> {
    const text = JSON.stringify(job);
    const key = `${this.version}#${this.revision}#${text}`;
    return this.memo.get(key, async () => {
      if (opts.alive && !opts.alive()) throw new Error('superseded');
      const kind = (job as { kind: string }).kind;
      const jid = ++this.jobId;
      this.inflight.set(jid, kind);
      this.emitBusy();
      let r: Ok;
      try {
        r = HEAVY.has(kind) ? await this.heavy(text) : await this.call({ op: 'run', job: text });
      } finally {
        this.inflight.delete(jid);
        this.emitBusy();
      }
      const out = JSON.parse(r.json!) as Record<string, unknown>;
      return Object.defineProperty(out, 'arrays', { value: r.arrays ?? {}, enumerable: false }) as unknown as Run<J>;
    }) as Promise<Run<J>>;
  }

  terminate() {
    this.resetSide();
    this.worker.terminate();
  }
}
