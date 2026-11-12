# Wellness Lab: agent notes

Open-source, local-first analytics for Garmin wellness data. AGPL-3.0-only. Read `docs/architecture.md` first.

## Conventions

- Logic that defines numbers lives in `crates/core`; logic that defines what you see lives in `packages/views` and `app`.
- Every method has a registry entry (`crates/core/src/registry.rs`) with citations and a validity grade (A to D). The UI shows the grade.
- No personal data in git. `.local/` is ignored: it holds exports, goldens and caches. Tests and e2e use `core::synth`.
- Goldens gate engine changes (`tools/goldens`, frozen per version, never overwritten).
- Run everything through `scripts/tc` (Rust, wasm-pack, maturin, Node 24, uv, Playwright live in the container).
- Releases only move forward: never replace a released asset.
- The data contract is `schema/wellness-v1.json`; garmin-mcp vendors it as a test fixture. Adding a column is a minor change; renaming or removing is v2.
- Copied effort-curves files start with `// Adapted from effort-curves <path> @ bef4120 (same author), relicensed under AGPL-3.0-only`.
- Conventional commits, one logical change each. No emojis in code or docs.
