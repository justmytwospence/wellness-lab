# Contributing

Thanks for looking. Issues are welcome. Outside contributions require signing the CLA (`CLA.md`; the CLA bot asks on your first pull request); open an issue first.

## Setup

Everything runs in a toolchain container, so the host needs only Docker, Git and Bash:

```sh
git clone https://github.com/justmytwospence/wellness-lab.git && cd wellness-lab
scripts/tc cargo test --workspace
scripts/tc bash -c 'npm ci && npm test'
```

## Layout

| Path | What |
|---|---|
| `schema/` | the `wellness-v1` contract |
| `crates/core` | the engine: every number the app shows |
| `crates/fit` | Garmin account-export builder |
| `crates/io` | native Parquet read and write |
| `crates/wasm`, `crates/py` | browser and Python bindings |
| `crates/server` | the self-hosted server |
| `packages/*`, `app/` | TypeScript data layer and React app |
| `tools/goldens` | parity checks against the HRV notebook and garmin-mcp (local only, personal data) |

## What a change needs

- Logic that defines numbers lives in `crates/core`, with a unit test on hand-computed inputs and a registry entry citing its source.
- Engine changes that move numbers must pass the goldens (`tools/goldens`).
- No personal data in git: tests use `core::synth`.
- Conventional commits, one logical change each.
