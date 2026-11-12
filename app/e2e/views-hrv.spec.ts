import { expect, test, type Page } from '@playwright/test';

// The demo data (core::synth) has nightly HRV, sleep and training load but no beat-to-beat intervals, so /thresholds and
// /beats may legitimately show the "Not available from this data source" message instead of charts.
const CHART = 'main canvas, main svg[data-chart]';

async function open(page: Page, link: string) {
  await page.locator(`nav[aria-label="Views"] a[title="${link}"]`).click();
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
  await page.getByLabel('Show', { exact: true }).selectOption('1y');
  await expect(page).toHaveURL(/span=1y/);
  await page.getByLabel('Normal range from').selectOption('28');
  await expect(page).toHaveURL(/nr=28/);
  await expect(page.getByTestId('notes')).toBeVisible();
  await page.keyboard.press('n');
  await expect(page.getByTestId('notes')).toHaveCount(0);
  await page.keyboard.press('n');
  await expect(page.getByTestId('notes')).toBeVisible();
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
