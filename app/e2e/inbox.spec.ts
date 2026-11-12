import { expect, test } from '@playwright/test';

test('confirming a suggestion makes a label and undo removes it', async ({ page }) => {
  await page.goto('/?demo=1');
  await page.goto('/inbox?demo=1');
  const card = page.getByTestId('proposal').first();
  await expect(card).toBeVisible({ timeout: 30_000 });
  const labels = page.getByTestId('labels');
  await page.waitForTimeout(1500);
  const before = await labels.locator('b').count();
  await card.getByRole('button', { name: /Confirm/ }).click();
  await expect(labels.locator('b')).toHaveCount(before + 1);
  await page.getByRole('button', { name: 'Undo' }).click();
  await expect(labels.locator('b')).toHaveCount(before);
});
