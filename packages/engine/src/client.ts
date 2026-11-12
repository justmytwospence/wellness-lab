import type { Job, Label, Manifest, Proposal, ResultOf, RunArrays, Settings } from '@wl/schema';
import { createMemo } from './memo';
import type { ReqBody, Res } from './worker';

export interface LoadProgress { done: number; total: number; file: string }
export type Run<J extends Job> = ResultOf<J> & { arrays: RunArrays };

/** The app's handle on the engine worker: loading, state and memoised jobs. */
export class EngineClient {
  private worker: Worker;
  private next = 1;
  private pending = new Map<number, { resolve: (r: Extract<Res, { ok: true }>) => void; reject: (e: Error) => void }>();
  private memo = createMemo(64 * 1024 * 1024);
  private version = 'empty';
  private revision = 0;
  onProgress: (p: LoadProgress) => void = () => {};
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

  private call(req: ReqBody): Promise<Extract<Res, { ok: true }>> {
    const id = this.next++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.worker.postMessage({ ...req, id });
    });
  }

  /** Load the OPFS `lab/wellness-v1` files into the engine. */
  async load(): Promise<Manifest | null> {
    const r = await this.call({ op: 'load' });
    this.manifest = r.manifest ?? null;
    this.version = this.manifest?.generated_at ?? 'empty';
    this.memo.clear();
    return this.manifest;
  }

  /** Replace the data with synthetic demo data; returns its true labels. */
  async demo(seed = 42, days = 730): Promise<Label[]> {
    const r = await this.call({ op: 'demo', seed, days });
    this.version = `demo:${seed}:${days}`;
    this.memo.clear();
    return JSON.parse(r.json!) as Label[];
  }

  async setState(s: { labels: Label[]; proposals: Proposal[]; settings: Settings }): Promise<void> {
    await this.call({ op: 'state', labels: JSON.stringify(s.labels), proposals: JSON.stringify(s.proposals), settings: JSON.stringify(s.settings) });
    this.revision++;
    this.memo.clear();
  }

  /** Run a job. Results are memoised by job and data version; arrays come back as typed arrays under `arrays`. */
  run<J extends Job>(job: J, opts: { alive?: () => boolean } = {}): Promise<Run<J>> {
    const text = JSON.stringify(job);
    const key = `${this.version}#${this.revision}#${text}`;
    return this.memo.get(key, async () => {
      if (opts.alive && !opts.alive()) throw new Error('superseded');
      const r = await this.call({ op: 'run', job: text });
      const out = JSON.parse(r.json!) as Record<string, unknown>;
      return Object.defineProperty(out, 'arrays', { value: r.arrays ?? {}, enumerable: false }) as unknown as Run<J>;
    }) as Promise<Run<J>>;
  }

  terminate() {
    this.worker.terminate();
  }
}
