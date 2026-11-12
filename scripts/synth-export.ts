// Write synthetic data as a wellness-v1 directory with the browser's writer (@wl/ingest writeExport), so
// tools/goldens/check_schema_files.py can check what the TS writer produces. Needs the nodejs wasm build
// (scripts/build-wasm.sh; WL_WASM_NODE names another build). Run through scripts/tc:
//
//   npx tsx scripts/synth-export.ts .local/synth-ts [seed] [days]
import { createRequire } from 'node:module';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { LIBRARY_DIR, memoryFiles, writeExport, type ExportResultLike } from '@wl/ingest';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const wasmDir = process.env.WL_WASM_NODE ?? join(root, '.local/wasm-node');
const wasm = createRequire(import.meta.url)(join(wasmDir, 'wellness_lab.js')) as {
  synthExport(seed: number, days: number): ExportResultLike;
  version(): string;
};

const [outArg, seedArg, daysArg] = process.argv.slice(2);
if (!outArg) {
  console.error('usage: synth-export.ts <out-dir> [seed=42] [days=120]');
  process.exit(2);
}
const out = resolve(outArg);
const seed = Number(seedArg ?? 42);
const days = Number(daysArg ?? 120);

const res = wasm.synthExport(seed, days);
const files = memoryFiles();
const manifest = await writeExport(res, files, `wellness-lab-synth-ts@${wasm.version()}`, 'synth');

rmSync(out, { recursive: true, force: true });
const prefix = LIBRARY_DIR.join('/') + '/';
let n = 0;
for (const [key, bytes] of files.files) {
  if (!key.startsWith(prefix)) continue;
  const path = join(out, key.slice(prefix.length));
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, bytes);
  n++;
}
console.log(`${n} files (${manifest.files.length} in the manifest) in ${out}`);
