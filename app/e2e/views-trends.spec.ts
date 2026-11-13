import { expect, test, type Page } from '@playwright/test';
import { go } from './helpers';

/** Demo data (synthetic, 730 nights), then each of the three views through the rail. */
async function open(page: Page, path: string, heading: string) {
  await go(page, path);
  await expect(page.getByRole('heading', { name: heading, exact: true, level: 1 })).toBeVisible();
  await expect(page.getByTestId('error-boundary')).toHaveCount(0);
  await expect(page.getByTestId('chart').first()).toBeVisible({ timeout: 45_000 });
}

test('Typical, Trends and Circadian render charts on demo data', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/?demo=1');
  await expect(page.getByText('Against your own baseline')).toBeVisible({ timeout: 45_000 });

  await open(page, '/typical', 'Typical');
  await expect(page.getByRole('img', { name: 'Typical night quantile bands' })).toBeVisible({ timeout: 45_000 });
  await expect(page.getByRole('img', { name: /Median curve for/ }).first()).toBeVisible({ timeout: 45_000 });
  await page.keyboard.press('a');
  await expect(page.getByLabel('Alignment')).toHaveValue('clock');
  await expect(page.getByText('Clock time, from 20:00')).toBeVisible();

  await open(page, '/trends', 'Trends');
  await expect(page.locator('.u-over').first()).toBeVisible({ timeout: 45_000 });
  const pick = (id: string) => page.locator(`[data-testid=focus-item][data-id=${id}]`).click();
  await pick('timing');
  await expect(page.getByRole('img', { name: 'Every night, bed to wake' })).toBeVisible();
  // the synthetic data has no Garmin readiness factors: the item says what it needs instead of drawing nothing
  await page.locator('[data-testid=focus-item][data-na="1"]').first().click();
  await expect(page.getByTestId('needs-input').first()).toContainText('Not available from this data source: needs');
  await pick('periods');
  await expect(page.getByRole('img', { name: /Box plots/ }).first()).toBeVisible();
  await pick('score');
  await expect(page.getByRole('img', { name: 'Histogram' })).toBeVisible();
  await expect(page.getByRole('button', { name: /^About / }).first()).toBeVisible();

  await open(page, '/circadian', 'Circadian');
  await expect(page.getByRole('img', { name: /Double-plotted actogram/ })).toBeVisible({ timeout: 60_000 });
  await expect(page.locator('nav[aria-label=Charts]').getByText('Experimental', { exact: true })).toBeVisible();
  await pick('sri');
  await expect(page.locator('.u-over').first()).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId('error-boundary')).toHaveCount(0);

  expect(errors).toEqual([]);
});
