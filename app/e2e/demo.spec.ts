import { expect, test } from '@playwright/test';

test('demo data loads and every route renders', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/?demo=1');
  await expect(page.getByRole('heading', { name: /Last night/ })).toBeVisible({ timeout: 30_000 });
  await expect(page.getByText('Against your own baseline')).toBeVisible();
  for (const [key, heading] of [['n', null], ['r', 'Nights'], ['i', 'Inbox'], ['u', 'Data']] as const) {
    await page.keyboard.press('g');
    await page.keyboard.press(key);
    if (heading) await expect(page.getByRole('heading', { name: heading })).toBeVisible();
    await expect(page.getByTestId('error-boundary')).toHaveCount(0);
  }
  if (process.env.SHOTS) {
    for (const [path, name] of [['/', 'today'], ['/nights', 'nights'], ['/inbox', 'inbox'], ['/night/20080', 'night']] as const) {
      await page.evaluate((p) => { history.pushState(null, '', p); dispatchEvent(new PopStateEvent('popstate')); }, path);
      await page.waitForTimeout(2500);
      await page.screenshot({ path: `/w/.local/shots/${name}.png`, fullPage: true });
    }
  }
  expect(errors).toEqual([]);
});
