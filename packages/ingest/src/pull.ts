/** Pull a garmin-mcp wellness-v1 export into the file store: only files whose sha256 changed, six at a time, manifest last. */
import type { Manifest } from '@wl/schema';
import { LIBRARY_DIR, type FileStore } from './fs';

export interface PullOptions {
  base: string;
  token?: () => Promise<string | null>;
  files: FileStore;
  fetch?: typeof fetch;
  emit?: (e: { done: number; total: number; file: string }) => void;
  signal?: AbortSignal;
}
export class AuthError extends Error {}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function get(o: PullOptions, path: string, tries = 4): Promise<Response> {
  const f = o.fetch ?? fetch;
  for (let a = 0; ; a++) {
    const t = o.token ? await o.token() : null;
    const r = await f(`${o.base.replace(/\/$/, '')}/${path}`, { headers: t ? { authorization: `Bearer ${t}` } : {}, signal: o.signal });
    if (r.status === 401) throw new AuthError('sign-in needed');
    if ((r.status === 429 || r.status >= 500) && a < tries) {
      const wait = Number(r.headers.get('retry-after')) || 2 ** a;
      await sleep(Math.min(wait, 30) * 1000);
      continue;
    }
    if (!r.ok) throw new Error(`${path}: HTTP ${r.status}`);
    return r;
  }
}

export async function localManifest(files: FileStore): Promise<Manifest | null> {
  const b = await files.read([...LIBRARY_DIR, 'manifest.json']);
  return b ? (JSON.parse(new TextDecoder().decode(b)) as Manifest) : null;
}

export async function pullWellness(o: PullOptions): Promise<{ changed: string[]; removed: string[]; manifest: Manifest }> {
  const remote = (await (await get(o, 'manifest.json')).json()) as Manifest;
  if (!Array.isArray(remote.files)) throw new Error('not a wellness-v1 manifest');
  const local = await localManifest(o.files);
  const have = new Map((local?.files ?? []).map((f) => [f.path, f.sha256]));
  const todo = remote.files.filter((f) => have.get(f.path) !== f.sha256);
  const want = new Set(remote.files.map((f) => f.path));
  const removed = [...have.keys()].filter((p) => !want.has(p));
  let done = 0;
  const queue = [...todo];
  const worker = async () => {
    for (let f = queue.shift(); f; f = queue.shift()) {
      const bytes = new Uint8Array(await (await get(o, f.path)).arrayBuffer());
      await o.files.write([...LIBRARY_DIR, ...f.path.split('/')], bytes);
      o.emit?.({ done: ++done, total: todo.length, file: f.path });
    }
  };
  await Promise.all(Array.from({ length: Math.min(6, todo.length) }, worker));
  for (const p of removed) await o.files.remove([...LIBRARY_DIR, ...p.split('/')]).catch(() => {});
  await o.files.write([...LIBRARY_DIR, 'manifest.json'], JSON.stringify(remote));
  return { changed: todo.map((f) => f.path), removed, manifest: remote };
}
