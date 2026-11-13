import { expect, test } from '@playwright/test';
import { go, openDemo } from './helpers';

const PATHS = ['/', '/nights', '/night/20080', '/typical', '/compare', '/hrv', '/trends', '/circadian', '/models', '/thresholds', '/beats', '/inbox', '/events', '/experiments', '/bench', '/garmin', '/sql', '/methods', '/data', '/settings'];

test.use({ viewport: { width: 1440, height: 900 } });

test('no view scrolls the page', async ({ page }) => {
  test.setTimeout(300_000);
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await openDemo(page);
  for (const p of PATHS) {
    await go(page, p);
    await page.waitForSelector('main .panel, main .empty, main .strip', { timeout: 60_000 });
    await page.waitForTimeout(800);
    const m = await page.evaluate(() => {
      const main = document.querySelector('main')!;
      return { doc: document.scrollingElement!.scrollHeight - innerHeight, main: main.scrollHeight - main.clientHeight };
    });
    expect(m.doc, `${p} page scroll`).toBeLessThanOrEqual(1);
    expect(m.main, `${p} main scroll`).toBeLessThanOrEqual(1);
    await expect(page.getByTestId('error-boundary'), p).toHaveCount(0);
    if (process.env.SHOTS) await page.screenshot({ path: `/w/.local/shots/ui-${p === '/' ? 'today' : p.slice(1).replace(/\//g, '_')}.png` });
  }
  expect(errors).toEqual([]);
});

test('workspaces and inspector', async ({ page }) => {
  await openDemo(page);
  await page.locator('nav[aria-label=Workspaces]').getByRole('link', { name: 'Recovery' }).click();
  await expect(page).toHaveURL(/\/hrv/);
  await expect(page.locator('nav[aria-label=Views] a')).toHaveCount(4);
  await page.keyboard.press('i');
  await expect(page.locator('aside.inspector')).toBeVisible();
  await expect(page.getByTestId('notes')).toBeVisible();
  await page.getByRole('button', { name: /^About / }).first().click();
  await expect(page.locator('aside.inspector h1')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.locator('aside.inspector h1')).toHaveCount(0);
});

test.describe('mobile', () => {
  test.use({ viewport: { width: 400, height: 800 } });
  test('stacks and scrolls, rail at the bottom', async ({ page }) => {
    await openDemo(page);
    const r = await page.evaluate(() => {
      const main = document.querySelector('main')!;
      return { scroll: main.scrollHeight > main.clientHeight, railY: document.querySelector('nav.rail')!.getBoundingClientRect().y };
    });
    expect(r.scroll).toBe(true);
    expect(r.railY).toBeGreaterThan(700);
  });
});
