import { test } from '@playwright/test';

// Local only: real data through the server (personal data, screenshots stay in .local/shots).
test('real data views', async ({ page }) => {
  test.skip(!process.env.LAB_REAL, 'local only: LAB_REAL=1 with a server holding real data (.local/real-smoke.sh)');
  test.setTimeout(600_000);
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  await page.goto('/');
  await page.getByLabel('Access token').fill('real');
  await page.getByRole('button', { name: 'Sign in' }).click();
  await page.getByRole('heading', { name: /Last night/ }).waitFor({ timeout: 300_000 });
  await page.waitForTimeout(4000);
  for (const [path, name] of [['/', 'r-today'], ['/nights?range=1y', 'r-nights'], ['/inbox', 'r-inbox'], ['/beats', 'r-beats'], ['/thresholds', 'r-thr']] as const) {
    const t0 = Date.now();
    await page.evaluate((p) => { history.pushState(null, '', p); dispatchEvent(new PopStateEvent('popstate')); }, path);
    await page.waitForTimeout(path === '/thresholds' ? 60_000 : 6000);
    console.log(name, Date.now() - t0, 'ms');
    await page.screenshot({ path: `/w/.local/shots/${name}.png`, fullPage: true });
  }
  console.log('ERRORS', JSON.stringify(errors.slice(0, 10)));
});
