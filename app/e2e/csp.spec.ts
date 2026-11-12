import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

// The built app runs under the CSP from vercel.json with no violations.
const csp = ((JSON.parse(readFileSync(resolve(process.cwd(), '../vercel.json'), 'utf8')) as { headers: { headers: { key: string; value: string }[] }[] }).headers[0]!.headers.find((h) => h.key === 'Content-Security-Policy')!).value;

test('no CSP violations on the main views', async ({ page }) => {
  const bad: string[] = [];
  await page.route('**/*', async (route) => {
    const res = await route.fetch();
    if (route.request().resourceType() === 'document') {
      await route.fulfill({ response: res, headers: { ...res.headers(), 'content-security-policy': csp } });
    } else await route.fulfill({ response: res });
  });
  page.on('console', (m) => { if (/Content Security Policy|Refused to/i.test(m.text())) bad.push(m.text()); });
  page.on('pageerror', (e) => bad.push(e.message));
  await page.goto('/?demo=1');
  await expect(page.getByRole('heading', { name: /Last night/ })).toBeVisible({ timeout: 30_000 });
  for (const p of ['/nights', '/night/20080', '/hrv', '/sql', '/methods']) {
    await page.evaluate((x) => { history.pushState(null, '', x); dispatchEvent(new PopStateEvent('popstate')); }, p);
    await page.waitForTimeout(800);
  }
  expect(bad).toEqual([]);
});
