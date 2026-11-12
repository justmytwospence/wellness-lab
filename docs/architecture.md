# Architecture

One engine, three ways to run it. Every number is computed by the Rust crate `crates/core`; the app, the server and the Python package are thin shells around it.

## Data flow

```
Garmin account export (zip) ---+
                               +--> wellness-v1 files (manifest.json + Parquet) --> engine --> views
garmin-mcp /export/wellness/v1 +                                                     ^
synthetic generator (core::synth) -----------------------------------------------+   |
                                                                                  labels, settings
```

1. **Sources** write the `wellness-v1` contract (below): the account-export importer (`crates/fit` builds the tables from the export's FIT and JSON files; `crates/io` and the browser's writer in `packages/ingest` put them in Parquet), garmin-mcp's `/export/wellness/v1/`, and the synthetic generator used by tests and the demo.
2. **The engine** (`crates/core`) reads those tables into a `Data` value, holds your labels and settings in a `Lab`, and runs jobs described as JSON (`lab.rs`): nightly features, baselines, HRV, beats and DFA a1, circadian metrics, detection, event studies, the bench, the Garmin teardown, experiments, and the downsampling and rasters the charts draw. Large outputs cross as typed arrays, never JSON.
3. **The app** (`app/`, React, built with Vite) shows the results. It loads the engine as WebAssembly in a worker (`packages/engine`) and keeps files and state in the browser (`packages/store`: OPFS and local storage). Charts are uPlot and d3; the SQL tab is DuckDB-WebAssembly over the same Parquet files.
4. **The server** (`crates/server`, axum) is the self-hosted shell. It syncs garmin-mcp's export into `/data/tenants/<tenant>/wellness-v1/`, serves the app and those files (`/data/wellness-v1/...`), keeps labels, proposals, settings and experiments in `state.sqlite`, runs the same detection natively after each sync, sends notifications (ntfy, webhook) and answers the read-only MCP endpoint. The browser in server mode pulls the changed files into its own storage and computes locally; answers to the Inbox and edits to labels go back to the server through `/api`.

Because the browser engine and the server's detection are the same `core` code, a proposal in a notification matches the Inbox. A test replays the same jobs through the Python wheel and the wasm build and requires byte-equal results (`tests/xbind`).

## Crates and packages

| Path | What |
|---|---|
| `crates/core` | The engine: statistics, baselines, nightly features, HRV (the HRV4Training-style analysis), DFA a1, beat cleaning, circadian metrics, detection and classification, event studies, the metric bench, the Garmin teardown, self-experiments, rasters and downsampling, synthetic data, and `registry.rs` (every method with its citations and validity grade). |
| `crates/fit` | Builds wellness-v1 tables from a Garmin account export (FIT decoding, JSON summaries). |
| `crates/io` | Native Parquet read and write of wellness-v1 directories; the zip importer. |
| `crates/wasm` | The browser build (wasm-bindgen, built by `scripts/build-wasm.sh`, with SIMD). |
| `crates/py` | The Python build, the `wellness-lab-engine` wheel (PyO3, abi3 for Python 3.11 and later; `import wellness_lab`). |
| `crates/server` | `wellness-lab-server`: `serve`, `import-export`, `detect`, `synth`, `healthcheck`. |
| `packages/schema` | Shared TypeScript types and the table schema. |
| `packages/ingest` | Reading a zip or a garmin-mcp export in the browser, the wellness-v1 writer, OAuth sign-in to garmin-mcp. |
| `packages/store` | Browser file store (OPFS), local state, backup export and import, the server-mode store. |
| `packages/engine` | The wasm engine in a worker, with a memo cache. |
| `packages/views` | Formatting, palettes, date ranges, URL state and the "why" text for evidence. |
| `app/` | The React app and its Playwright specs. |
| `schema/` | The data contract. |
| `tools/goldens` | Parity checks against the HRV notebook and garmin-mcp; local only (see below). |

Rule of thumb: logic that defines numbers lives in `crates/core`; logic that defines what you see lives in `packages/views` and `app`.

## The wellness-v1 contract

`schema/SPEC.md` describes it and `schema/wellness-v1.json` lists every column. It is a `manifest.json` (a sha256 per file, so consumers download only what changed) plus Parquet files: tables `days`, `sleeps`, `stages`, `activities`, `tags`, `devices`; one file per UTC month for each series (`series/<name>/<YYYY-MM>.parquet`: heart rate, stress, HRV every five minutes, respiration, SpO2, Body Battery, steps, altitude and others); and `rr/` with beat-to-beat intervals for activities recorded with Log HRV. Instants are Unix milliseconds in UTC, days are days since 1970-01-01 of Garmin's local date, and a night belongs to the day you woke up. Missing values are nulls.

Adding an optional column, a series or a table is a minor change. Renaming or removing anything is v2, written side by side. garmin-mcp vendors the schema as a test fixture, so the two projects cannot drift apart unnoticed.

## Methods and validity

Every method has an entry in `crates/core/src/registry.rs`: what it computes, the formula, its parameters and ranges, the inputs and minimum data it needs, the papers it comes from and a validity grade A to D. The app's Methods page and the grade badges on charts read from that registry, and the parameters it lists are the ones the UI lets you adjust. A method is not added without an entry.

## Goldens and tests

- Unit tests sit beside the code in each crate, on hand-computed inputs; detection is tested for recall on synthetic data with injected episodes.
- `tools/goldens` records the HRV notebook's outputs from a real garmin-mcp snapshot as frozen goldens (`.local/goldens/hrv-v<version>/`, never overwritten, never committed because they are personal data) and checks the engine against them (`check_hrv.py`) and the Parquet writers' output against the schema (`check_schema_files.py`). They gate any engine change that moves numbers. `scripts/goldens.sh` runs them.
- `crates/py/tests` exercises the wheel; `tests/xbind` compares the wheel and the wasm build.
- `app/e2e` is a Playwright suite over the built app with synthetic data, including performance budgets, and a server-mode spec run against a real server.
- Everything runs through `scripts/tc`, a toolchain container with Rust 1.99.1, wasm-pack, maturin, Node 24, uv and Playwright; CI runs the same steps.

## Build and release

`Dockerfile`: stage one builds the server and, with `scripts/build-wasm.sh`, the wasm engine on the pinned Rust toolchain; stage two builds the app against that wasm; stage three is `debian:bookworm-slim` with the binary and `app/dist`, running as uid 10001 with `/data` as its volume. A tag `v*` publishes the image to `ghcr.io/justmytwospence/wellness-lab` for amd64 and arm64 and attaches the wheels and `wellness-lab-wasm-<version>.tgz` to the GitHub release. Releases only move forward.

## Tenants

Every file a server owns lives under `TenantPaths` (`/data/tenants/<tenant>/`: `wellness-v1/`, `state.sqlite`, `cache/`, `tmp/`). Only one tenant (`default`) is used today; the seam exists so a hosted, multi-user service would be a configuration change, but none is planned.
