/** An import's result as wellness-v1 Parquet files in the file store, with a manifest the engine and the server share. */
import { parquetWriteBuffer } from 'hyparquet-writer';
import type { Manifest, ManifestFile } from '@wl/schema';
import schema from '@wl/schema/src/schema';
import { LIBRARY_DIR, TMP_DIR, type FileStore } from './fs';
import type { ExportResultLike } from './types';

const DAY = 86_400_000;

/** Month groups of a sorted `t` (UTC): [{month: '2026-11', from, to}) row ranges. */
export function splitMonths(t: Float64Array): { month: string; from: number; to: number }[] {
  const out: { month: string; from: number; to: number }[] = [];
  let start = 0;
  let cur = '';
  for (let i = 0; i <= t.length; i++) {
    const m = i < t.length ? new Date(t[i]!).toISOString().slice(0, 7) : '';
    if (m !== cur) {
      if (cur) out.push({ month: cur, from: start, to: i });
      cur = m;
      start = i;
    }
  }
  return out;
}

export async function sha256(bytes: Uint8Array): Promise<string> {
  const d = await crypto.subtle.digest('SHA-256', bytes as BufferSource);
  return [...new Uint8Array(d)].map((b) => b.toString(16).padStart(2, '0')).join('');
}

type ColType = string;
const specs = (kind: 'tables' | 'series', name: string): [string, ColType][] => {
  const all = (schema as unknown as Record<string, Record<string, [string, string][]>>)[kind]!;
  return all[name]!;
};

type PqCol = { name: string; data: ArrayLike<unknown>; type: string };

function column(name: string, type: ColType, n: number, num: Float64Array | null, str: (string | null)[] | null): PqCol {
  if (type === 'VARCHAR') return { name, type: 'STRING', data: str ?? new Array(n).fill(null) };
  const src = num ?? new Float64Array(n).fill(NaN);
  const data = new Array<number | bigint | null>(n);
  for (let i = 0; i < n; i++) {
    const v = src[i]!;
    data[i] = Number.isNaN(v) ? null : type === 'BIGINT' ? BigInt(Math.round(v)) : v;
  }
  return { name, type: type === 'BIGINT' ? 'INT64' : type === 'INTEGER' ? 'INT32' : 'FLOAT', data: data as never };
}

function slice(c: PqCol, a: number, b: number): PqCol {
  return { ...c, data: Array.prototype.slice.call(c.data, a, b) };
}

const buf = (cols: PqCol[]): Uint8Array => new Uint8Array(parquetWriteBuffer({ columnData: cols as never, codec: 'SNAPPY' } as never));

export async function writeExport(res: ExportResultLike, files: FileStore, importer: string, kind = 'garmin-export'): Promise<Manifest> {
  const entries: ManifestFile[] = [];
  const put = async (path: string, bytes: Uint8Array, rows: number, tmin?: number, tmax?: number, fingerprint?: string) => {
    await files.write([...LIBRARY_DIR, ...path.split('/')], bytes);
    const sha = await sha256(bytes);
    entries.push({ path, sha256: sha, bytes: bytes.length, rows, t_min: tmin ?? null, t_max: tmax ?? null, fingerprint: fingerprint ?? `${rows}|${sha}` });
  };
  for (const table of res.tables()) {
    const types = specs('tables', table);
    const cols = res.columns(table);
    const have = new Set(cols);
    const n = cols.length ? res.numeric(table, cols.find((c) => types.find((s) => s[0] === c)?.[1] !== 'VARCHAR') ?? cols[0]!).length : 0;
    const out: PqCol[] = types.map(([name, type]) => {
      if (!have.has(name)) return column(name, type, n, null, null);
      return type === 'VARCHAR' ? column(name, type, n, null, JSON.parse(res.strings(table, name)) as (string | null)[]) : column(name, type, n, res.numeric(table, name), null);
    });
    await put(`${table}.parquet`, buf(out), n);
  }
  for (const name of res.series()) {
    const types = specs('series', name);
    const cols = types.map(([c, type]) => ({ name: c, type, arr: res.seriesColumn(name, c) }));
    const t = cols[0]!.arr;
    for (const { month, from, to } of splitMonths(t)) {
      const out = cols.map((c) => slice(column(c.name, c.type, t.length, c.arr, null), from, to));
      let sum = 0;
      const v = cols[1]?.arr;
      if (v) for (let i = from; i < to; i++) if (!Number.isNaN(v[i]!)) sum += v[i]!;
      await put(`series/${name}/${month}.parquet`, buf(out), to - from, t[from], t[to - 1], `${to - from}|${t[from]}|${t[to - 1]}|${sum.toFixed(3)}`);
    }
  }
  for (const id of res.rrIds()) {
    const rr = res.rr(id);
    await put(`rr/${id}.parquet`, buf([column('rr', 'FLOAT', rr.length, Float64Array.from(rr), null)]), rr.length);
  }
  const manifest: Manifest = {
    schema_version: schema.schema_version, importer, generated_at: new Date().toISOString(), source: { kind }, files: entries.sort((a, b) => a.path.localeCompare(b.path)),
  };
  // the manifest last, so a half-written import has none
  await files.write([...LIBRARY_DIR, 'manifest.json'], JSON.stringify(manifest, null, 1));
  await files.remove(TMP_DIR).catch(() => {});
  void DAY;
  return manifest;
}
