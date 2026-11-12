# Wellness Lab

A lab instrument for the part of your Garmin data that is not activities: the nights, the heart rate and HRV underneath them, stress, breathing, SpO2, Body Battery and the rhythm of your days. It shows them at full resolution, says how each number is computed and how far to trust it on wrist data, and gives you tools to ask questions of your own data instead of reading a score.

It is open source (AGPL-3.0-only), local-first, and meant for people who want the details. It reports signs in your numbers; it is not a medical device and does not diagnose anything.

Screenshots go in `docs/screenshots/`.

## Three ways to run it

1. **In the browser, with an account export.** Open the app, drop in the zip Garmin emails you (Garmin Connect, Account, Data Management, Export Your Data), and everything is computed on your machine. Nothing is uploaded; `?demo=1` loads synthetic data to look around.
2. **Self-hosted next to garmin-mcp.** [garmin-mcp](https://github.com/justmytwospence/garmin-mcp) mirrors your Garmin account and publishes a wellness export; the `wellness-lab` compose profile syncs it, runs detection, keeps your labels and sends notifications. Set `GARMIN_EXPORT=canonical,wellness`, `EXPORT_TOKEN` and `LAB_TOKEN`, then `docker compose --profile wellness-lab up -d`.
3. **A standalone server with an account export.** The same server without garmin-mcp:

   ```sh
   docker compose run --rm wellness-lab import-export /import/garmin-export.zip
   docker compose up -d        # http://localhost:8080, sign in with LAB_TOKEN
   ```

Details, settings, reverse proxies, notifications and backups: [docs/self-hosting.md](docs/self-hosting.md). To build and run from source: `docker build -t wellness-lab:dev .`, or see [CONTRIBUTING.md](CONTRIBUTING.md).

## Privacy

Your data stays in your browser or on your server. The only outbound requests are to your own garmin-mcp and to the notification URLs you set; there is no analytics, no tracking and no hosted service. See [docs/privacy.md](docs/privacy.md).

## What it does

- **Night viewer.** One night at full resolution: heart rate, HRV every five minutes, breathing, SpO2, stress and stages on shared axes, with each number scored against your own baseline.
- **Nights raster.** Every night as a row of a heatmap (heart rate, stress, HRV and more), aligned on falling asleep or on the clock, and sortable by date, heart rate, HRV, sleep length or timing, so a pattern across months shows at a glance.
- **Typical night.** What a night usually looks like for you: the median curve and its spread, week by week, and one period against another.
- **HRV deep dive.** Overnight RMSSD against your 7-night baseline and normal range, the four weekly signals and the trend patterns they make, the response to training load, the shape of HRV inside the night, and a state-space model of what tonight should have been.
- **DFA a1 thresholds.** Aerobic and anaerobic thresholds estimated from beat-to-beat data of activities recorded with Log HRV, with a worked example of every step.
- **Beat lab.** The beat intervals themselves: cleaning, tachogram, Poincare plot, RMSSD, SD1, SD2 and the spectrum, window by window.
- **Circadian metrics.** Interdaily stability, intradaily variability, L5 and M10, the Sleep Regularity Index, cosinor, social jet lag and chronotype, sleep debt, the heart-rate rhythm and a modelled melatonin onset.
- **Inference inbox.** The app notices stretches that depart from your normal, proposes what they might be (illness-like strain, vaccination, alcohol, hard training, stress, travel, altitude, a lasting shift) and asks you within a weekly budget, in the app or through ntfy or a webhook. Your answers teach it your patterns. See [docs/inference.md](docs/inference.md).
- **Event studies.** How every metric moves around the nights of an event you labelled, against matched control nights, with a bootstrap interval.
- **Metric bench.** For every number, its typical error and reliability on your quiet nights, the smallest change that is real, and how it responds to your events: per-user reliability, not a population claim.
- **Garmin score teardown.** What Garmin's own published inputs predict for its sleep score and training readiness, a rebuilt Body Battery, and the window that reproduces its HRV status band.
- **Self-experiments.** N-of-1 trials with alternating blocks, randomised order and washout nights: the answer is a probability of a worthwhile effect, with "inconclusive" kept apart from "no effect".
- **SQL tab.** DuckDB over your own Parquet files, in the page.
- **MCP endpoint.** In server mode, `/mcp` gives an MCP client read-only access to your labelled episodes, nightly features, proposals, the bench and HRV status.

Also: compare any two sets of nights, trends by month, weekday and year, a strain model, and a Methods page that lists every method with its formula, parameters and papers. Settings, labels, answers and experiments are yours to export and back up.

## Validity grades

Wrist data is not laboratory data, and the numbers do not deserve equal trust. Every method carries a grade, shown wherever it is used, and listed with its citations on the Methods page.

| Grade | Meaning |
|---|---|
| A | Solid: a direct computation from what the watch records, or a certain event such as a time-zone change. |
| B | Good for your own trends: reliable enough to follow your changes over time. |
| C | Experimental: a model or an estimate with weak validation on wrist data; read it as a hint. |
| D | Shown for completeness, never used for inference (for example Garmin's sleep stages). |

## Documentation

- [docs/self-hosting.md](docs/self-hosting.md): compose, standalone, settings, proxies, notifications, backups
- [docs/inference.md](docs/inference.md): what is detected, how, and what is never inferred
- [docs/privacy.md](docs/privacy.md): where the data lives and what is sent where
- [docs/architecture.md](docs/architecture.md): data flow, crates and packages, the `wellness-v1` contract, goldens
- [schema/SPEC.md](schema/SPEC.md): the data contract
- [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md)

## License

AGPL-3.0-only (`LICENSE`). Outside contributions require signing the CLA (`CLA.md`); the CLA bot asks on your first pull request, and please open an issue first.
