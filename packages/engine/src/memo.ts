// Adapted from effort-curves packages/engine/src/memo.ts @ bef4120 (same author), relicensed under AGPL-3.0-only
/** A short, stable fingerprint of a typed array (two 32-bit FNV-1a passes over its words), for cache keys. */
export function hashArray(a: ArrayBufferView): string {
  const words = a.byteLength % 4 === 0 ? new Uint32Array(a.buffer, a.byteOffset, a.byteLength / 4) : new Uint8Array(a.buffer, a.byteOffset, a.byteLength);
  let h1 = 0x811c9dc5, h2 = 0x9e3779b9;
  const m2 = 0x01000193 ^ 0x5bd1e995;
  for (let i = 0; i < words.length; i++) {
    const w = words[i]!;
    h1 = Math.imul(h1 ^ w, 0x01000193);
    h2 = Math.imul(h2 ^ w, m2);
  }
  return `${a.byteLength}:${(h1 >>> 0).toString(36)}:${(h2 >>> 0).toString(36)}`;
}

/** The bytes held by the typed arrays one level down in a result (1 KB when it has none). */
export function bytesOf(v: unknown): number {
  if (ArrayBuffer.isView(v)) return v.byteLength;
  if (!v || typeof v !== 'object') return 1024;
  const nested = (v as { arrays?: Record<string, unknown> }).arrays;
  if (nested) return Object.values(nested).reduce<number>((n, x) => n + (ArrayBuffer.isView(x) ? x.byteLength : 0), 1024);
  let n = 0;
  for (const x of Object.values(v as Record<string, unknown>)) if (ArrayBuffer.isView(x)) n += x.byteLength;
  return n || 1024;
}

/**
 * Results by key, oldest out first once their typed arrays pass `capBytes`. The promise is stored at once, so the same
 * request asked twice while it runs runs once; a rejection is forgotten. Results are shared: never write into them.
 */
export function createMemo(capBytes: number) {
  const map = new Map<string, { p: Promise<unknown>; bytes: number }>();
  let total = 0;
  const evict = () => {
    for (const [k, v] of map) {
      if (total <= capBytes || map.size <= 1) break;
      if (v.bytes === 0) continue; // still running
      map.delete(k);
      total -= v.bytes;
    }
  };
  return {
    get<T>(key: string, make: () => Promise<T>): Promise<T> {
      const hit = map.get(key);
      if (hit) {
        map.delete(key);
        map.set(key, hit);
        return hit.p as Promise<T>;
      }
      const entry = { p: null as unknown as Promise<unknown>, bytes: 0 };
      entry.p = make().then(
        (v) => {
          if (map.get(key) === entry) {
            entry.bytes = bytesOf(v);
            total += entry.bytes;
            evict();
          }
          return v;
        },
        (e) => {
          if (map.get(key) === entry) map.delete(key);
          throw e;
        },
      );
      map.set(key, entry);
      return entry.p as Promise<T>;
    },
    clear() {
      map.clear();
      total = 0;
    },
  };
}
