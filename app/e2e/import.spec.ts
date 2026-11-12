import { expect, test } from '@playwright/test';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';

const zip = resolve(process.cwd(), 'e2e/.fixtures/fake-export.zip');

test('importing a Garmin export zip in the browser', async ({ page }) => {
  test.skip(!existsSync(zip), 'run the make_fake_export example first');
  await page.goto('/data');
  await page.getByTestId('import-file').setInputFiles(zip);
  const report = page.getByTestId('import-report');
  await expect(report).toBeVisible({ timeout: 60_000 });
  await expect(report).toContainText('monitoring_b');
  await expect(report).toContainText('device files');
  // the data is in the app now: Today has the last night
  await page.goto('/');
  await expect(page.getByRole('heading', { name: /Last night/ })).toBeVisible({ timeout: 30_000 });
  await page.goto('/nights');
  await expect(page.locator('canvas')).toBeVisible();
});
