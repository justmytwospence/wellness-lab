import { expect, type Page } from '@playwright/test';

/** Client-side navigation (history + popstate), so in-memory demo data survives. */
export async function go(page: Page, path: string) {
  await page.evaluate((p) => { history.pushState(null, '', p); dispatchEvent(new PopStateEvent('popstate')); }, path);
  await expect(page).toHaveURL(new RegExp(path.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').replace(/\?.*$/, '') + '(\\?.*)?$'));
}

/** Open the demo and wait until Today has rendered. */
export async function openDemo(page: Page) {
  await page.goto('/?demo=1');
  await expect(page.getByRole('heading', { name: /Last night/ })).toBeVisible({ timeout: 90_000 });
}
