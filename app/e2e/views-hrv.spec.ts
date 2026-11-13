import { expect, test, type Page } from '@playwright/test';
import { go } from './helpers';

// The demo data (core::synth) has nightly HRV, sleep and training load but no beat-to-beat intervals, so /thresholds and
// /beats may legitimately show the "Not available from this data source" message instead of charts.
const CHART = 'main canvas, main svg[data-chart]';

const PATHS: Record<string, string> = { HRV: '/hrv', Thresholds: '/thresholds', Beats: '/beats' };
async function open(page: Page, link: string) {
  await go(page, PATHS[link]!);
}

test('HRV, Thresholds and Beats render on demo data', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/?demo=1');
  await expect(page.getByText('Last night').first()).toBeVisible({ timeout: 90_000 });

  await open(page, 'HRV');
  await expect(page.getByRole('heading', { name: 'HRV', level: 1 })).toBeVisible();
  await expect(page.locator(CHART).first()).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId('error-boundary')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'About baseline.normal_range' })).toBeVisible();

  // controls sync to the URL; the notes column toggles with n
  await page.getByRole('button', { name: '1y', exact: true }).click();
  await expect(page).toHaveURL(/range=1y/);
  await page.getByLabel('Normal range from').selectOption('28');
  await expect(page).toHaveURL(/nr=28/);
  await expect(page.getByTestId('notes')).toHaveCount(0);
  await page.keyboard.press('n');
  await expect(page.getByTestId('notes')).toBeVisible();
  await page.keyboard.press('n');
  await expect(page.getByTestId('notes')).toHaveCount(0);
  for (const tab of ['Trends', 'Response', 'Inside the night', 'Model']) {
    await page.getByRole('tab', { name: tab }).click();
    await expect(page.locator(CHART).first()).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId('error-boundary')).toHaveCount(0);
  }

  for (const [link, heading] of [['Thresholds', 'Thresholds'], ['Beats', 'Beats']] as const) {
    await open(page, link);
    await expect(page.getByRole('heading', { name: heading, level: 1 })).toBeVisible();
    await expect(page.locator(`${CHART}, :text("Not available from this data source")`).first()).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId('error-boundary')).toHaveCount(0);
    await expect(page.getByTestId('chart-card').first()).toBeVisible();
  }
  expect(errors).toEqual([]);
});
