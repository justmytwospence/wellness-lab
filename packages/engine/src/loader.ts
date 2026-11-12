/** wellness-v1 Parquet files to the typed arrays the wasm engine takes. Runs in the engine worker. */
import { parquetRead } from 'hyparquet';
import { compressors } from 'hyparquet-compressors';
import type { Manifest } from '@wl/schema';

export interface Columns { names: string[]; numeric: Float64Array[]; strings: Record<string, (string | null)[]>; rows: number }

type ChunkData = ArrayLike<unknown>;

function toF64(a: ChunkData): Float64Array {
  const out = new Float64Array(a.length);
  for (let i = 0; i < a.length; i++) {
    const v = a[i];
    out[i] = v === null || v === undefined ? NaN : typeof v === 'bigint' ? Number(v) : (v as number);
  }
  return out;
}

/** Read every column of a one-row-group Parquet file (chunks joined in row order). */
export async function readColumns(buf: ArrayBuffer): Promise<Columns> {
  const parts = new Map<string, { start: number; data: ChunkData }[]>();
  await parquetRead({
    file: buf,
    compressors,
    onChunk: (c: { columnName: string; columnData: ChunkData; rowStart: number }) => {
      let l = parts.get(c.columnName);
      if (!l) parts.set(c.columnName, (l = []));
      l.push({ start: c.rowStart, data: c.columnData });
    },
  });
  const cols: Columns = { names: [], numeric: [], strings: {}, rows: 0 };
  for (const [name, chunks] of parts) {
    chunks.sort((a, b) => a.start - b.start);
    const total = chunks.reduce((n, c) => n + c.data.length, 0);
    cols.rows = Math.max(cols.rows, total);
    const first = chunks.find((c) => c.data.length)?.data[0];
    if (typeof first === 'string') {
      const all: (string | null)[] = [];
      for (const c of chunks) for (let i = 0; i < c.data.length; i++) all.push((c.data[i] as string | null) ?? null);
      cols.strings[name] = all;
    } else {
      const out = new Float64Array(total);
      let o = 0;
      for (const c of chunks) {
        out.set(toF64(c.data), o);
        o += c.data.length;
      }
      cols.names.push(name);
      cols.numeric.push(out);
    }
  }
  return cols;
}

/** Concatenate the per-month column sets of one series. */
export function concat(parts: Columns[]): Columns {
  if (parts.length === 1) return parts[0]!;
  const names = parts[0]?.names ?? [];
  const rows = parts.reduce((n, p) => n + p.rows, 0);
  const numeric = names.map((n, i) => {
    const out = new Float64Array(rows);
    let o = 0;
    for (const p of parts) {
      const j = p.names.indexOf(n);
      if (j >= 0) out.set(p.numeric[j]!, o);
      else out.fill(NaN, o, o + p.rows);
      o += p.rows;
    }
    void i;
    return out;
  });
  return { names, numeric, strings: {}, rows };
}

export const TABLES = ['days', 'sleeps', 'stages', 'activities', 'tags', 'devices'] as const;

export function plan(m: Manifest) {
  const tables: string[] = [];
  const series = new Map<string, string[]>();
  const rr: string[] = [];
  for (const f of m.files) {
    const p = f.path;
    if (p.startsWith('series/')) {
      const name = p.split('/')[1]!;
      (series.get(name) ?? series.set(name, []).get(name)!).push(p);
    } else if (p.startsWith('rr/')) rr.push(p);
    else if (p.endsWith('.parquet')) tables.push(p);
  }
  for (const l of series.values()) l.sort();
  return { tables, series, rr };
}
