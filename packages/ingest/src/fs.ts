// Adapted from effort-curves packages/ingest/src/fs.ts @ bef4120 (same author), relicensed under AGPL-3.0-only
import { readFile, removeEntry } from '@wl/store';

/** The few file operations the library needs; OPFS in the browser, a map in tests. Paths are directory segments then a name. */
export interface FileStore {
  read(path: string[]): Promise<ArrayBuffer | null>;
  write(path: string[], data: Uint8Array | string): Promise<void>;
  remove(path: string[]): Promise<void>;
}

export const LIBRARY_DIR = ['lab', 'wellness-v1'];
export const TMP_DIR = ['lab', 'tmp'];

async function dir(path: string[]): Promise<FileSystemDirectoryHandle> {
  let d = await navigator.storage.getDirectory();
  for (const p of path) d = await d.getDirectoryHandle(p, { create: true });
  return d;
}

type SyncHandle = {
  truncate(size: number): void;
  write(data: BufferSource, opts?: { at: number }): number;
  flush(): void;
  close(): void;
};

/** Write through a synchronous access handle where the runtime has one (workers, incl. Safari), else `createWritable`. */
export async function writeOpfs(path: string[], data: Uint8Array | string): Promise<void> {
  const h = await (await dir(path.slice(0, -1))).getFileHandle(path.at(-1)!, { create: true });
  const bytes = typeof data === 'string' ? new TextEncoder().encode(data) : data;
  const sync = (h as unknown as { createSyncAccessHandle?: () => Promise<SyncHandle> }).createSyncAccessHandle;
  if (typeof sync === 'function') {
    const a = await sync.call(h);
    try {
      a.truncate(0);
      a.write(bytes as BufferSource, { at: 0 });
      a.flush();
    } finally {
      a.close();
    }
    return;
  }
  const w = await h.createWritable();
  await w.write(bytes as BufferSource);
  await w.close();
}

export const opfsFiles: FileStore = {
  read: readFile,
  write: writeOpfs,
  remove: removeEntry,
};

/** A FileStore in memory (tests, Node scripts). */
export function memoryFiles(): FileStore & { files: Map<string, Uint8Array> } {
  const files = new Map<string, Uint8Array>();
  const key = (p: string[]) => p.join('/');
  return {
    files,
    async read(p) {
      const b = files.get(key(p));
      return b ? (b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength) as ArrayBuffer) : null;
    },
    async write(p, d) {
      files.set(key(p), typeof d === 'string' ? new TextEncoder().encode(d) : d.slice());
    },
    async remove(p) {
      const k = key(p);
      for (const f of [...files.keys()]) if (f === k || f.startsWith(k + '/')) files.delete(f);
    },
  };
}
