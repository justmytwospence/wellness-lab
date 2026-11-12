import { expect, test } from '@playwright/test';

// Budgets on the 730-night demo with a warm worker. A loaded CI machine gets slack.
const slack = Number(process.env.PERF_SLACK ?? 1.5);

/** In-page time from a route change until `selector` exists. */
async function until(page: import('@playwright/test').Page, to: string, selector: string): Promise<number> {
  return page.evaluate(async ([p, sel]) => {
    const t0 = performance.now();
    history.pushState(null, '', p!);
    dispatchEvent(new PopStateEvent('popstate'));
    await new Promise<void>((res) => {
      const iv = setInterval(() => {
        if (document.querySelector(sel!) || performance.now() - t0 > 10_000) { clearInterval(iv); res(); }
      }, 5);
    });
    return Math.round(performance.now() - t0);
  }, [to, selector]);
}

test('interaction budgets', async ({ page }) => {
  await page.goto('/?demo=1');
  await expect(page.getByRole('heading', { name: /Last night/ })).toBeVisible({ timeout: 30_000 });
  await page.waitForTimeout(1500);
  await until(page, '/night/20080', '[data-testid="lane-hr"]'); // chunk load, not timed
  const nightMs = await until(page, '/night/20079', '[data-testid="lane-hr"]');
  const rasterMs = await until(page, '/nights?range=all', 'canvas[style*="crosshair"]');
  const hrvMs = await until(page, '/hrv', 'main canvas, main svg[role=img], main .uplot');
  console.log(JSON.stringify({ nightMs, rasterMs, hrvMs }));
  expect(nightMs).toBeLessThan(150 * slack + 150);
  expect(rasterMs).toBeLessThan(300 * slack + 200);
  expect(hrvMs).toBeLessThan(1000 * slack + 500);
});
