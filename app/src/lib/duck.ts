/** DuckDB-wasm, self-hosted: the wasm and worker files are bundled assets (no CDN), so the CSP stays 'self'. */
import * as duckdb from '@duckdb/duckdb-wasm';
import mvpWasm from '@duckdb/duckdb-wasm/dist/duckdb-mvp.wasm?url';
import mvpWorker from '@duckdb/duckdb-wasm/dist/duckdb-browser-mvp.worker.js?url';
import ehWasm from '@duckdb/duckdb-wasm/dist/duckdb-eh.wasm?url';
import ehWorker from '@duckdb/duckdb-wasm/dist/duckdb-browser-eh.worker.js?url';
import { parquetWriteBuffer } from 'hyparquet-writer';
import { readFile } from '@wl/store';
import type { Manifest, Table } from '@wl/schema';

export type { duckdb };
export const ROOT = ['lab', 'wellness-v1'];
const BUNDLES: duckdb.DuckDBBundles = {
  mvp: { mainModule: mvpWasm, mainWorker: mvpWorker },
  eh: { mainModule: ehWasm, mainWorker: ehWorker },
};

export async function openDuck(): Promise<duckdb.AsyncDuckDB> {
  const bundle = await duckdb.selectBundle(BUNDLES);
  const worker = new Worker(bundle.mainWorker!);
  const db = new duckdb.AsyncDuckDB(new duckdb.VoidLogger(), worker);
  await db.instantiate(bundle.mainModule, bundle.pthreadWorker);
  return db;
}

const INT32 = new Set(['day', 'weekday', 'free_day', 'excluded', 'first_day', 'last_day', 'utc_offset_min']);
const INT64 = new Set(['onset', 'wake', 'mid', 'start', 'end', 't']);

/** An engine table (`{n, num, txt}`) as one Parquet file, with the column types the wellness-v1 tables use. */
export function tableToParquet(t: Table): Uint8Array {
  const columnData: { name: string; type: string; data: unknown[] }[] = [];
  for (const [name, v] of Object.entries(t.num)) {
    if (INT64.has(name)) columnData.push({ name, type: 'INT64', data: v.map((x) => (x === null || !Number.isFinite(x) ? null : BigInt(Math.round(x)))) });
    else if (INT32.has(name)) columnData.push({ name, type: 'INT32', data: v.map((x) => (x === null || !Number.isFinite(x) ? null : Math.round(x))) });
    else columnData.push({ name, type: 'DOUBLE', data: v.map((x) => (x === null || !Number.isFinite(x) ? null : x)) });
  }
  for (const [name, v] of Object.entries(t.txt)) columnData.push({ name, type: 'STRING', data: v });
  return new Uint8Array(parquetWriteBuffer({ columnData, codec: 'SNAPPY' } as never));
}

const q = (s: string) => `'${s.replace(/'/g, "''")}'`;
export const VIRTUAL = 'wl/';

/** Register the OPFS wellness-v1 files and create the views over them. Returns the view names. */
export async function registerRaw(db: duckdb.AsyncDuckDB, conn: duckdb.AsyncDuckDBConnection, previous: string[]): Promise<{ views: string[]; files: string[] }> {
  const m = await readFile([...ROOT, 'manifest.json']);
  if (!m) return { views: [], files: [] };
  const manifest = JSON.parse(new TextDecoder().decode(m)) as Manifest;
  const tables: string[] = [];
  const series = new Map<string, string[]>();
  const files: string[] = [];
  for (const f of manifest.files) {
    if (f.path.startsWith('rr/') || !f.path.endsWith('.parquet')) continue; // beat intervals are many tiny files; the Beats page reads them
    const buf = await readFile([...ROOT, ...f.path.split('/')]);
    if (!buf) continue;
    await db.dropFile(VIRTUAL + f.path).catch(() => {});
    await db.registerFileBuffer(VIRTUAL + f.path, new Uint8Array(buf));
    files.push(VIRTUAL + f.path);
    if (f.path.startsWith('series/')) {
      const name = f.path.split('/')[1]!;
      (series.get(name) ?? series.set(name, []).get(name)!).push(VIRTUAL + f.path);
    } else tables.push(f.path.replace(/\.parquet$/, ''));
  }
  const views: string[] = [];
  for (const old of previous) await conn.query(`DROP VIEW IF EXISTS ${old}`);
  for (const t of tables) {
    if (!/^[a-z_]+$/.test(t)) continue;
    await conn.query(`CREATE OR REPLACE VIEW ${t} AS SELECT * FROM read_parquet(${q(VIRTUAL + t + '.parquet')})`);
    views.push(t);
  }
  for (const [name, list] of series) {
    if (!/^[a-z0-9_]+$/.test(name)) continue;
    list.sort();
    await conn.query(`CREATE OR REPLACE VIEW series_${name} AS SELECT * FROM read_parquet([${list.map(q).join(', ')}], union_by_name = true)`);
    views.push(`series_${name}`);
  }
  return { views, files };
}

/** Register (or replace) one engine table as a Parquet buffer and a view of the same name. */
export async function registerTable(db: duckdb.AsyncDuckDB, conn: duckdb.AsyncDuckDBConnection, name: string, t: Table): Promise<void> {
  const file = `${VIRTUAL}engine/${name}.parquet`;
  await db.dropFile(file).catch(() => {});
  await db.registerFileBuffer(file, tableToParquet(t));
  await conn.query(`CREATE OR REPLACE VIEW ${name} AS SELECT * FROM read_parquet(${q(file)})`);
}
