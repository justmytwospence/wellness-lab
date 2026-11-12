import { dirname, resolve } from 'node:path';
import { createReadStream, existsSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import react from '@vitejs/plugin-react';
import { defineConfig, loadEnv, type Plugin } from 'vite';

const root = dirname(fileURLToPath(import.meta.url));

/** dev and preview: serve a local wellness-v1 directory at /data/wellness-v1 (never part of `vite build`) */
function localData(): Plugin {
  const dir = process.env.LAB_LOCAL_DATA ? resolve(process.env.LAB_LOCAL_DATA) : '';
  const handler = (req: { url?: string }, res: { setHeader(k: string, v: string): void; statusCode: number; end(): void }, next: () => void) => {
    const url = (req.url ?? '').split('?')[0]!;
    const prefix = '/data/wellness-v1/';
    if (!dir || !url.startsWith(prefix)) return next();
    const file = resolve(dir, decodeURIComponent(url.slice(prefix.length)));
    if (!file.startsWith(dir) || !existsSync(file) || !statSync(file).isFile()) {
      res.statusCode = 404;
      return res.end();
    }
    res.setHeader('Content-Type', file.endsWith('.json') ? 'application/json' : 'application/octet-stream');
    createReadStream(file).pipe(res as never);
  };
  return { name: 'wl-local-data', configureServer: (s) => void s.middlewares.use(handler as never), configurePreviewServer: (s) => void s.middlewares.use(handler as never) };
}

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, root, '');
  const server = env.LAB_DEV_SERVER;
  return {
    plugins: [react(), localData()],
    worker: { format: 'es' as const },
    optimizeDeps: { exclude: ['@duckdb/duckdb-wasm'] },
    build: { target: 'es2022', sourcemap: true, chunkSizeWarningLimit: 1500, assetsInlineLimit: 0 },
    server: {
      host: true,
      fs: { allow: [resolve(root, '..')] },
      proxy: server ? { '/api': server, '/data': server, '/mcp': server } : undefined,
    },
    preview: { host: true },
  };
});
