# Self-hosting

The server is one container (`ghcr.io/justmytwospence/wellness-lab`) that serves the app, keeps your labels and settings, runs the detection that feeds the Inbox, sends notifications and answers a read-only MCP endpoint. Your wellness data lives in one volume mounted at `/data`.

The app computes in your browser, with the same engine compiled to WebAssembly. In server mode the browser pulls the server's wellness-v1 files (`/data/wellness-v1/...`, only the ones that changed) into its own storage, so the server needs little memory and no GPU. The default limit in the compose files is 1 GB.

## Next to garmin-mcp

garmin-mcp mirrors your Garmin account and publishes the wellness export that Wellness Lab reads. Both run from garmin-mcp's `compose.yaml`; the `wellness-lab` profile is defined there.

In garmin-mcp's `.env`:

```sh
GARMIN_EXPORT=canonical,wellness   # "wellness" is required; "canonical" is for Effort Curve
EXPORT_TOKEN=<openssl rand -hex 32>    # garmin-mcp's export token; the lab sends it as GARMIN_MCP_TOKEN
LAB_TOKEN=<openssl rand -hex 32>       # what you type to sign in to the lab
#LAB_PUBLIC_URL=https://lab.example.com
#NTFY_URL=https://ntfy.example.com/wellness
#NOTIFY_WEBHOOK_URL=
```

Then:

```sh
docker compose --profile wellness-lab up -d
```

The lab listens on `WELLNESS_LAB_PORT` (default 8080). It pulls `http://garmin-mcp:8000/export/wellness/v1/` every `LAB_SYNC_MINUTES` (default 15), downloading only files whose sha256 changed, then reruns detection. garmin-mcp republishes the export after each night's sleep or HRV lands (`GARMIN_SNAPSHOT_WELLNESS_DELAY_MINUTES`), so the lab follows within a sync interval. The Data view shows the last sync and any error; `POST /api/sync` (the Sync now button) forces one.

If garmin-mcp answers 404, `GARMIN_EXPORT` does not include `wellness`. If it answers 401, the `EXPORT_TOKEN` differs between the two.

## Standalone, from an account export

Use `compose.yaml` in this repository.

1. Request your data from Garmin (Garmin Connect, Account, Data Management, Export Your Data). Garmin emails a zip.
2. Put it in `./import`, and set `LAB_TOKEN` in `.env`.
3. Import, then start:

```sh
docker compose run --rm wellness-lab import-export /import/garmin-export.zip
docker compose up -d
```

`import-export` writes a wellness-v1 directory into the volume and runs detection once; it prints how many files it wrote and how many proposals it found. Import a newer export whenever you like to bring the data up to date; your labels and settings live in `state.sqlite` and the import does not touch them.

An account export has what Garmin records on the watch, including heart rate, sleep, stress, SpO2 and beat-to-beat intervals for activities recorded with Log HRV. It does not follow you automatically: re-export and re-import to update.

Other commands in the image: `serve` (the default), `detect` (rerun detection and send due notifications), `synth --days N` (write synthetic data, for trying the app), `healthcheck` (what the image's `HEALTHCHECK` runs).

## Settings

| Variable | Default | |
|---|---|---|
| `LAB_TOKEN` | unset | Bearer token for the app, the API, the data files and `/mcp`. Unset means no authentication and the server says so at start. |
| `LAB_DATA_DIR` | `/data` | Where tenants keep data and state. |
| `LAB_PORT` | `8080` | |
| `LAB_PUBLIC_URL` | unset | The address you reach the lab at; used for links in notifications and to mark the session cookie `Secure` when it starts with `https://`. |
| `LAB_TENANT` | `default` | Letters, digits, `-` and `_`. Data lives under `/data/tenants/<tenant>`. |
| `GARMIN_MCP_URL`, `GARMIN_MCP_TOKEN` | unset | garmin-mcp's base URL and its `EXPORT_TOKEN`; the URL needs the token. Unset means import mode. |
| `LAB_SYNC_MINUTES` | `15` | |
| `NTFY_URL`, `NTFY_TOKEN` | unset | An ntfy topic and, if it needs one, an access token. |
| `NOTIFY_WEBHOOK_URL` | unset | |
| `NOTIFY_WEBHOOK_TOKEN` | unset | sent as `Authorization: Bearer <token>` with every webhook call |
| `LAB_MCP` | `on` | `off` disables `/mcp`. |

Empty values count as unset, so compose's `${VAR:-}` is safe.

## Sign-in, reverse proxies and forward auth

`LAB_TOKEN` is the only credential. The sign-in form exchanges it for an HttpOnly, SameSite=Strict session cookie; scripts and MCP clients send `Authorization: Bearer <token>`. A wrong token waits a second before it answers 401.

Put the lab behind a reverse proxy that terminates TLS and forwards to port 8080, and set `LAB_PUBLIC_URL` to the https address so the cookie is marked `Secure`. Nothing needs websockets. The app sends a strict Content-Security-Policy (`default-src 'self'` with WebAssembly allowed), so a proxy that adds its own CSP header should leave it alone or match it.

With forward auth (Authelia, Authentik, oauth2-proxy), protect everything except what your clients need, and keep `LAB_TOKEN` set as well: the token is what `/mcp` and scripts use, and it keeps the lab closed if the proxy is bypassed. The lab does not read identity headers.

To connect an MCP client, point it at `https://<your lab>/mcp` (streamable HTTP) with the bearer token. The tools are read-only: `list_episodes`, `get_night`, `nights`, `get_proposals`, `get_bench`, `get_hrv_status` and `event_study`. They return your own data to whatever client you connect, so treat the token like the data.

## Notifications

When the Inbox has a question the policy wants to ask (see [inference](inference.md)), the lab sends it to:

- **ntfy:** set `NTFY_URL` to a topic URL (for example `https://ntfy.sh/your-long-random-topic`; self-host ntfy if you do not want a third party to see the text). The message carries a link to the Inbox when `LAB_PUBLIC_URL` is set.
- **Webhook:** `NOTIFY_WEBHOOK_URL` receives a JSON POST: `{"type": "proposal", "proposal": {...}, "text": "...", "url": "..."}`; the test message is `{"type": "test", ...}`. When a question that was sent is answered (in the app, or by whatever the webhook calls back through `/api/proposals/{id}/answer`), the webhook gets `{"type": "answered", "proposal": {...}, "label": {...} | null, "url": "..."}`, and `{"type": "reopened", ...}` after an undo, so a chat message can be edited to say so. With `NOTIFY_WEBHOOK_TOKEN` set, every call carries it as a bearer token.

Settings has a test button (`POST /api/notify/test`) and switches for each channel. Questions are limited by the weekly budget (default 2).

## Backups

Everything is under `/data/tenants/<tenant>/`:

| Path | What | Back up? |
|---|---|---|
| `state.sqlite` | your labels, proposals and answers, settings, experiments and the notification log | **yes: irreplaceable** |
| `wellness-v1/` | the data (Parquet and `manifest.json`) | no when garmin-mcp feeds the lab (it resyncs); yes for an account-export lab if you do not keep the zip |
| `cache/`, `tmp/` | derived and in-flight | no |

Stop the container, or copy with SQLite's `.backup`, for a consistent `state.sqlite`. Restoring is putting the file back before the first start. In the browser, Data has a backup export for the labels the browser holds; in server mode the server is the source of truth.

If you bind-mount a host directory at `/data`, make it writable by uid 10001 (`chown 10001 <dir>`); a named volume already is.

## Upgrading

`docker compose pull && docker compose up -d`. Tags: `:<version>`, `:<major>` and `:latest`; the compose files follow `:0`, so a 0.x minor release is picked up by pulling again. The `wellness-v1` data contract only grows within a major version, so data and state carry over.
