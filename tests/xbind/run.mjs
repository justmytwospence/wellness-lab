// Adapted from effort-cubes tests/xbind/run.mjs @ ee7ff1d (same author), relicensed under AGPL-3.0-only
// Replay .local/xbind cases (written by make_cases.py through the wheel) through the nodejs wasm build
// (.local/wasm-node, built by scripts/build-wasm.sh; WL_WASM_NODE names another build) and require byte-equal JSON
// and arrays.
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const wasmDir = process.env.WL_WASM_NODE ?? join(root, ".local/wasm-node");
const wasm = createRequire(import.meta.url)(join(wasmDir, "wellness_lab.js"));
const base = join(root, ".local", "xbind");
const GETTERS = { float64: "f64", float32: "f32", int32: "i32", uint8: "u8" };
const CTORS = { float64: Float64Array, float32: Float32Array, int32: Int32Array, uint8: Uint8Array };

function load(ref) {
  const b = readFileSync(join(base, ref.file));
  return new CTORS[ref.dtype](b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength));
}
const bytes = (a) => new Uint8Array(a.buffer, a.byteOffset, a.byteLength);

/** The first differing character, with some context. */
function firstDiff(a, b) {
  let i = 0;
  while (i < a.length && i < b.length && a[i] === b[i]) i++;
  return `at char ${i}: wasm ...${a.slice(Math.max(0, i - 60), i + 60)}... vs wheel ...${b.slice(Math.max(0, i - 60), i + 60)}...`;
}

const man = JSON.parse(readFileSync(join(base, "cases.json"), "utf8"));
const lab = wasm.synth(man.seed, man.days);
lab.loadRr(man.rr.id, load(man.rr));

let ok = 0;
for (const c of man.jobs) {
  let bad = null;
  let arrays = 0;
  try {
    const res = lab.run(c.job);
    const json = res.json();
    const want = readFileSync(join(base, c.json), "utf8");
    if (json !== want) bad = `json differs (${json.length} vs ${want.length} chars) ${firstDiff(json, want)}`;
    const names = [...res.names()].sort();
    const wantNames = Object.keys(c.arrays).sort();
    if (!bad && names.join(",") !== wantNames.join(",")) bad = `arrays ${names} vs ${wantNames}`;
    for (const k of wantNames) {
      if (bad) break;
      const ref = c.arrays[k];
      const have = res[GETTERS[ref.dtype]](k);
      const want = load(ref);
      if (have.length !== want.length) {
        bad = `${k} length ${have.length} vs ${want.length}`;
        break;
      }
      const hb = bytes(have), wb = bytes(want);
      for (let i = 0; i < wb.length; i++) {
        if (hb[i] !== wb[i]) {
          const j = Math.floor(i / want.BYTES_PER_ELEMENT);
          bad = `${k} first diff at ${j}: ${have[j]} vs ${want[j]}`;
          break;
        }
      }
      arrays++;
    }
  } catch (e) {
    bad = `threw: ${e && e.message ? e.message : e}`;
  }
  if (bad) console.log(`FAIL ${c.name} ${bad}`);
  else {
    ok++;
    console.log(`PASS ${c.name} ${arrays} arrays`);
  }
}
console.log(`${ok}/${man.jobs.length} identical`);
process.exit(ok === man.jobs.length ? 0 : 1);
