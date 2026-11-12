import { expect, test, type Page } from '@playwright/test';

/** Open the demo (730 synthetic nights, injected illness labels) and wait until Today has rendered. */
async function openDemo(page: Page) {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/?demo=1');
  await expect(page.getByRole('heading', { name: /Last night/ })).toBeVisible({ timeout: 90_000 });
  await expect(page.locator('table.t tbody tr').first()).toBeVisible({ timeout: 90_000 });
  return errors;
}

/** Go to a route through the rail (client-side, so the demo data stays loaded). */
async function visit(page: Page, path: string) {
  await page.locator(`nav[aria-label="Views"] a[href="${path}"]`).click();
  await expect(page).toHaveURL(new RegExp(`${path}(\\?.*)?$`));
}

const noBoundary = (page: Page) => expect(page.locator('[data-testid=error-boundary]')).toHaveCount(0);

test.describe('analysis views on demo data', () => {
  test('events, compare, bench, models, garmin, experiments', async ({ page }) => {
    const errors = await openDemo(page);

    // Events: label kind illness gives a plotted chart per metric
    await visit(page, '/events');
    await expect(page.getByRole('heading', { name: 'Events', level: 1 })).toBeVisible();
    await expect(page.getByLabel('Label kind')).toHaveValue('illness');
    await expect(page.locator('[data-testid=event-hr_min30] [data-testid=chart] canvas').first()).toBeVisible();
    await expect(page.locator('[data-testid=chart-diff] canvas').first()).toBeVisible();
    await expect(page.locator('[data-testid=episode-raster] svg rect').first()).toBeVisible();
    await expect(page.getByTestId('event-summary')).toContainText('episodes');
    await noBoundary(page);

    // Compare: select by label kind, ranked list with paired strips
    await visit(page, '/compare');
    await expect(page.getByRole('heading', { name: 'Compare', level: 1 })).toBeVisible();
    await expect(page.getByTestId('no-selection')).toBeVisible();
    await page.getByLabel('Select by label kind').selectOption('illness');
    await expect(page).toHaveURL(/sel=/);
    await expect(page.getByTestId('compare-row').first()).toBeVisible();
    await expect(page.getByTestId('paired-strip').first()).toBeVisible();
    await noBoundary(page);

    // Bench: sortable table, signal-to-noise first, detail chart
    await visit(page, '/bench');
    await expect(page.getByRole('heading', { name: 'Bench', level: 1 })).toBeVisible();
    await expect(page.getByTestId('bench-row').first()).toBeVisible();
    await expect(page.getByTestId('bench-detail').locator('canvas').first()).toBeVisible();
    await expect(page.getByRole('button', { name: /^Signal to noise/ })).toHaveAttribute('aria-sort', 'descending');
    await page.getByRole('button', { name: /^TE/ }).click();
    await expect(page.getByRole('button', { name: /^TE/ })).toHaveAttribute('aria-sort', 'descending');
    await expect(page).toHaveURL(/sort=te/);
    await noBoundary(page);

    // Models: strain factor, loadings, residual heatmap
    await visit(page, '/models');
    await expect(page.getByRole('heading', { name: 'Models', level: 1 })).toBeVisible();
    await expect(page.locator('[data-testid=chart-strain] canvas').first()).toBeVisible();
    await expect(page.getByTestId('hbars').first()).toBeVisible();
    await expect(page.locator('[data-testid=residual-heatmap] svg rect').first()).toBeVisible();
    await noBoundary(page);

    // Garmin: regressions, Body Battery, band grid, stress vs HR (or the missing-input message)
    await visit(page, '/garmin');
    await expect(page.getByRole('heading', { name: 'Garmin', level: 1 })).toBeVisible();
    await expect(page.getByTestId('card-bb')).toBeVisible();
    await expect(page.getByTestId('card-hrv-band')).toBeVisible();
    await expect(page.getByTestId('card-stress-hr')).toBeVisible();
    await expect(page.locator('[data-testid=reg-garmin\\.teardown-Sleep\\ score\\ from\\ its\\ sub-scores]')).toContainText(/Coefficient|Not available from this data source/);
    await expect(page.getByTestId('bb-heatmap').locator('svg rect').first()).toBeVisible();
    await expect(page.getByTestId('band-grid-sd').locator('svg rect').first()).toBeVisible();
    await expect(page.getByTestId('band-best')).toContainText('Best fit');
    await noBoundary(page);

    // Experiments: wizard, schedule with adherence, result
    await visit(page, '/experiments');
    await expect(page.getByRole('heading', { name: 'Experiments', level: 1 })).toBeVisible();
    await page.getByRole('button', { name: 'New experiment' }).first().click();
    await expect(page.getByTestId('wizard').getByTestId('sched-cell').first()).toBeVisible();
    await page.getByLabel('Start date').fill('2025-03-03'); // inside the demo's nights, so the result has data
    await page.getByRole('button', { name: 'Save experiment' }).click();
    await expect(page.getByTestId('experiment-row')).toHaveCount(1);
    await expect(page.getByTestId('card-schedule').getByTestId('sched-cell').first()).toBeVisible();
    const box = page.getByTestId('card-schedule').getByRole('checkbox').first();
    await expect(box).toBeChecked();
    await box.uncheck();
    await expect(box).not.toBeChecked();
    await expect(page.getByTestId('status-sentence')).toBeVisible();
    await expect(page.getByTestId('quantile-dots').locator('circle')).toHaveCount(20);
    await page.getByLabel(/per protocol/).check();
    await expect(page.getByTestId('quantile-dots').locator('circle')).toHaveCount(20);
    await noBoundary(page);

    expect(errors).toEqual([]);
  });
});

test('analysis views say so on a phone', async ({ page }) => {
  await page.setViewportSize({ width: 400, height: 800 });
  await page.goto('/events');
  await expect(page.getByTestId('needs-larger-screen')).toContainText('/events');
});
