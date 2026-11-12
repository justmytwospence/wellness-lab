/** Signing in to a garmin-mcp export: OAuth authorization code with PKCE, tokens kept per server in this browser. */
// Adapted from effort-curves app/src/import/oauth.ts @ bef4120 (same author), relicensed under AGPL-3.0-only

const SESSION = 'wl.oauth';
const TOKENS = 'wl.tokens';
export const REDIRECT_PATH = '/oauth/callback';

export interface AuthMeta { mode: string; issuer: string; authorization_endpoint: string; token_endpoint: string; client_id: string; scope: string }
export interface Tokens { access_token: string; refresh_token: string; expires_at: number; token_endpoint: string; client_id: string }
export interface StoredServer { url: string; name?: string; auth?: Tokens }

const b64 = (b: ArrayBuffer | Uint8Array) => btoa(String.fromCharCode(...new Uint8Array(b))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');

export async function pkce() {
  const verifier = b64(crypto.getRandomValues(new Uint8Array(32)));
  const challenge = b64(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(verifier)));
  return { verifier, challenge };
}

export const servers = (): StoredServer[] => JSON.parse(localStorage.getItem(TOKENS) ?? '[]') as StoredServer[];
export const saveServers = (s: StoredServer[]) => localStorage.setItem(TOKENS, JSON.stringify(s));

export async function fetchAuth(base: string, f: typeof fetch = fetch): Promise<AuthMeta | string> {
  let r: Response;
  try {
    r = await f(`${base.replace(/\/$/, '')}/auth`);
  } catch {
    return `Could not reach ${new URL(base).host}. Check the address, and that the server allows this site (CORS).`;
  }
  if (r.status === 404) return 'No wellness export at this address (HTTP 404). It should end in /export/wellness/v1, and garmin-mcp needs GARMIN_EXPORT to include wellness.';
  if (!r.ok) return `The server answered HTTP ${r.status}.`;
  const m = (await r.json()) as AuthMeta;
  if (!m.authorization_endpoint) return 'This server does not offer sign-in from the browser (needs garmin-mcp 1.14 with its OIDC export auth on).';
  return m;
}

export async function beginSignIn(url: string, meta: AuthMeta, name?: string): Promise<void> {
  const { verifier, challenge } = await pkce();
  const state = b64(crypto.getRandomValues(new Uint8Array(16)));
  sessionStorage.setItem(SESSION, JSON.stringify({ state, verifier, url, name, token_endpoint: meta.token_endpoint, client_id: 'wellness-lab' }));
  const q = new URLSearchParams({ response_type: 'code', client_id: 'wellness-lab', redirect_uri: location.origin + REDIRECT_PATH, scope: meta.scope || 'export', state, code_challenge: challenge, code_challenge_method: 'S256' });
  location.assign(`${meta.authorization_endpoint}?${q}`);
}

async function tokenRequest(endpoint: string, body: Record<string, string>): Promise<Response> {
  return fetch(endpoint, { method: 'POST', headers: { 'content-type': 'application/x-www-form-urlencoded' }, body: new URLSearchParams(body) });
}
type TokenReply = { access_token: string; refresh_token?: string; expires_in?: number };
const tokensOf = (r: TokenReply, endpoint: string, client: string, prev?: Tokens): Tokens => ({
  access_token: r.access_token, refresh_token: r.refresh_token ?? prev?.refresh_token ?? '', expires_at: Date.now() + (r.expires_in ?? 3600) * 1000, token_endpoint: endpoint, client_id: client,
});

/** On the redirect back: swap the code for tokens and save the server. Returns its URL, or null when this is not a callback. */
export async function finishSignIn(): Promise<string | null> {
  const p = new URLSearchParams(location.search);
  const raw = sessionStorage.getItem(SESSION);
  sessionStorage.removeItem(SESSION);
  if (!raw || !p.get('code')) return null;
  const s = JSON.parse(raw) as { state: string; verifier: string; url: string; name?: string; token_endpoint: string; client_id: string };
  if (p.get('state') !== s.state) throw new Error('sign-in state mismatch');
  const r = await tokenRequest(s.token_endpoint, { grant_type: 'authorization_code', code: p.get('code')!, code_verifier: s.verifier, client_id: s.client_id, redirect_uri: location.origin + REDIRECT_PATH });
  if (!r.ok) throw new Error(`sign-in failed (HTTP ${r.status})`);
  const t = tokensOf((await r.json()) as TokenReply, s.token_endpoint, s.client_id);
  saveServers([...servers().filter((x) => x.url !== s.url), { url: s.url, name: s.name, auth: t }]);
  return s.url;
}

/** An access token good for the next pull, refreshed when near expiry; 'signin' when the sign-in has lapsed. */
export async function freshToken(s: StoredServer): Promise<string | 'signin'> {
  const a = s.auth;
  if (!a) return 'signin';
  if (a.expires_at - Date.now() > 120_000) return a.access_token;
  let r: Response;
  try {
    r = await tokenRequest(a.token_endpoint, { grant_type: 'refresh_token', refresh_token: a.refresh_token, client_id: a.client_id });
  } catch {
    return a.access_token;
  }
  if (r.status === 400 || r.status === 401) return 'signin';
  if (!r.ok) return a.access_token;
  const next = tokensOf((await r.json()) as TokenReply, a.token_endpoint, a.client_id, a);
  saveServers(servers().map((x) => (x.url === s.url ? { ...x, auth: next } : x)));
  return next.access_token;
}
