# Privacy

Your data stays where you put it. There is no Wellness Lab service that receives it, and the app has no analytics, tracking or error reporting.

## Browser mode

The app is a static site. When you use it in the browser, your account export is read in the page (the zip is unpacked by code running on your machine), turned into wellness-v1 files and kept in the browser's origin-private file storage (OPFS) and local storage for that site. The engine runs as WebAssembly in a worker, and the SQL tab runs DuckDB-WebAssembly in the page, both bundled with the app: no CDN, no external fonts or scripts. The Content-Security-Policy limits the page to its own origin (`connect-src 'self' https:` exists for the pull from your own garmin-mcp, below).

The only network requests a browser-mode session makes are the ones you set up:

- loading the app itself, from wherever it is hosted;
- if you connect a garmin-mcp, requests to that address (its manifest and the changed files) and to its sign-in page for the OAuth sign-in.

Labels, answers and settings are stored in the same browser storage. Clearing the site's data removes them; Data has a backup export and import for that reason. The labels and tokens are never sent anywhere.

## Server mode

The data lives on your server, in the volume mounted at `/data`. The server's outbound requests are:

- to your own garmin-mcp (`GARMIN_MCP_URL`) to fetch the wellness export, with the token you gave it;
- to your notification endpoints (`NTFY_URL`, `NOTIFY_WEBHOOK_URL`), if you set them. A notification carries the text of a question ("possible illness, 3 nights from Nov 3: resting HR +5 bpm, HRV -18 %") and a link. Use a self-hosted or private ntfy topic if you do not want a third party to see that text.

Nothing else is contacted. The browser talking to the server sends and receives the data files and the API; with `LAB_TOKEN` set, all of it needs the token (as a session cookie or a bearer header). Without `LAB_TOKEN` anyone who can reach the port can read and change the lab, and the server logs a warning at start.

The `/mcp` endpoint gives MCP clients read-only access to labels, nightly features, proposals, the bench and HRV status. Connecting an AI assistant to it shares that data with the assistant's provider; that is your decision, and `LAB_MCP=off` turns the endpoint off.

## Garmin

The lab never talks to Garmin. Data reaches it through your account export, or through garmin-mcp, which is where your Garmin sign-in lives (see garmin-mcp's own documentation).

## This repository

No personal data is committed. Tests and the end-to-end suite use synthetic data (`crates/core/src/synth.rs`); `.local/` is git-ignored and holds exports and caches on a developer's machine. The screenshots in `docs/screenshots/`, when added, are taken from synthetic data.
