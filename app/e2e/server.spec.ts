import { expect, test } from '@playwright/test';

// Needs a running wellness-lab-server with LAB_TOKEN=e2e and synthetic data, serving app/dist: see scripts/e2e-server.sh.
const base = process.env.LAB_E2E_SERVER;

test('server mode: sign in, answer a proposal, it persists across reload', async ({ page }) => {
  test.skip(!base, 'LAB_E2E_SERVER not set');
  await page.goto(`${base}/inbox`);
  await expect(page.getByLabel('Access token')).toBeVisible({ timeout: 30_000 });
  await page.getByLabel('Access token').fill('e2e');
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(page.getByRole('heading', { name: 'Inbox' })).toBeVisible({ timeout: 60_000 });
  const card = page.getByTestId('proposal').first();
  await expect(card).toBeVisible({ timeout: 60_000 });
  const labels = page.getByTestId('labels');
  const before = await labels.locator('b').count();
  await card.getByRole('button', { name: /Confirm/ }).click();
  await expect(labels.locator('b')).toHaveCount(before + 1);
  await page.reload();
  await expect(page.getByTestId('labels').locator('b')).toHaveCount(before + 1, { timeout: 60_000 });
});
