# wellness-v1

The canonical wellness data contract: a manifest plus Parquet files. Producers (garmin-mcp's `/export/wellness/v1/`, the account-export importer, the synthetic generator) write it; the engine and the app read it. The machine-readable column lists are `wellness-v1.json`; DuckDB `DESCRIBE` of every file must equal them.

## Conventions

- Every instant is `t` BIGINT: Unix milliseconds, UTC.
- Every calendar day is `day` INTEGER: days since 1970-01-01 of Garmin's local calendar date. A night belongs to the wake-up day.
- Missing values are nulls. Strings are UTF-8.
- Parquet is snappy-compressed, one row group, rows sorted by `t` (series) or `day`.
- There are deliberately no 8- or 16-bit integer columns.
- `schema_version` is 1. Adding an optional column, a series or a table is a minor change. Renaming or removing anything is v2, written side by side with v1.

## Layout

```
manifest.json
days.parquet  sleeps.parquet  stages.parquet  activities.parquet  tags.parquet  devices.parquet
series/<name>/<YYYY-MM>.parquet     one file per UTC month
rr/garmin-<activity id>.parquet     beat-to-beat intervals (ms) of activities recorded with Log HRV (>= 500 beats)
```

## manifest.json

```json
{"schema_version": 1, "importer": "garmin-mcp@1.14.0", "generated_at": "2026-11-12T18:00:00.000Z",
 "source": {"kind": "garmin-mcp"},
 "files": [{"path": "series/hr/2026-11.parquet", "table": "series", "name": "hr", "month": "2026-11",
            "rows": 15840, "t_min": 1793491200000, "t_max": 1794441540000,
            "sha256": "...", "fingerprint": "15840|1793491200000|1794441540000|1012345.0", "updated_at": "..."}]}
```

`source.kind` is `garmin-mcp`, `garmin-export` or `synth`. `table` is one of `days`, `sleeps`, `stages`, `activities`, `tags`, `devices`, `series`, `rr`. `name` and `month` appear only for series, `activity` only for rr files. Consumers download only files whose `sha256` changed.

## Tables and series

See `wellness-v1.json` for the exact columns. Meaning of the codes:

- `sleeps.kind`: 0 main sleep, 1 nap. `stages.stage`: 0 deep, 1 light, 2 REM, 3 awake, 4 unmeasurable.
- `stress.q`: 0 measured, 1 unmeasurable, 2 activity. `body_battery.q`: 0 measured, 1 estimated.
- `spo2.mode`: 0 continuous, 1 spot check, 2 other.
- `steps.v` is the number of steps in the interval ending at `t`.
- `hrv5.v` is overnight RMSSD in ms every five minutes; `hrv5.day` is the night it belongs to.
- `altitude` (metres) comes from Garmin's undocumented monitoring message 279, decoded as `raw / 5 - 500` (inferred; verified against daily average altitude).
- `motion` (account export only, experimental) is Garmin's undocumented per-minute counts (type-58 message 318, fields 0 and 3).
- `hs_*` columns are Garmin Health Status values and their baseline bounds (null where unavailable).
