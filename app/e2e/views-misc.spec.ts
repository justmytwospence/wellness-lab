import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

const registry = readFileSync(fileURLToPath(new URL('../../crates/core/src/registry.rs', import.meta.url)), 'utf8');
const METHOD_IDS = [...registry.matchAll(/^\s+m\("([^"]+)"/gm)].map((m) => m[1]!);

async function demo(page: Page) {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/?demo=1');
  // wait for the demo to finish loading: navigating earlier drops ?demo=1 from the URL
  await expect(page.getByText('Against your own baseline')).toBeVisible({ timeout: 90_000 });
  return errors;
}

test('methods lists every registry method with grade and DOI links', async ({ page }) => {
  const errors = await demo(page);
  expect(METHOD_IDS.length).toBeGreaterThan(20);
  await page.getByRole('link', { name: 'Methods' }).click();
  await expect(page.getByRole('heading', { name: 'Methods', level: 1 })).toBeVisible();
  await expect(page.locator('section[data-method]')).toHaveCount(METHOD_IDS.length);
  for (const id of METHOD_IDS) await expect(page.locator(`section[data-method="${id}"]`)).toHaveCount(1);
  await expect(page.locator('section[data-method] .grade')).toHaveCount(METHOD_IDS.length);
  const doi = page.locator('a[data-doi="true"]').first();
  await expect(doi).toBeVisible();
  expect(await doi.getAttribute('href')).toMatch(/^https:\/\/(dx\.)?doi\.org\/10\./);
  await page.getByLabel('Filter methods').fill('zzzz-no-such-method');
  await expect(page.locator('section[data-method]')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('settings: edit and save, theme and clock', async ({ page }) => {
  const errors = await demo(page);
  await page.getByRole('link', { name: 'Settings' }).click();
  const win = page.locator('#baseline_window');
  await expect(win).toHaveValue('28');
  await win.fill('35');
  const save = page.getByRole('button', { name: 'Save settings' });
  await expect(save).toBeEnabled();
  await save.click();
  await expect(page.getByText('Settings saved')).toBeVisible();
  await expect(win).toHaveValue('35');
  // an inconsistent pair of thresholds cannot be saved
  await page.locator('#ask_min').fill('0.99');
  await expect(save).toBeDisabled();
  await page.getByRole('button', { name: 'Reset to defaults' }).click();
  await expect(win).toHaveValue('28');
  // local mode: no server to send notifications
  await expect(page.getByRole('button', { name: 'Send test notification' })).toBeDisabled();
  // theme persists on this device
  await page.locator('#theme').selectOption('light');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.locator('#h12').selectOption('12');
  expect(await page.evaluate(() => [localStorage.getItem('wl.theme'), localStorage.getItem('wl.h12')])).toEqual(['light', '1']);
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  expect(errors).toEqual([]);
});

test('sql: SELECT 1 and the starter queries on demo data', async ({ page }) => {
  const errors = await demo(page);
  await page.getByRole('link', { name: 'SQL' }).click();
  const run = page.getByRole('button', { name: 'Run', exact: true });
  await expect(run).toBeEnabled({ timeout: 90_000 });
  await expect(page.getByText('Demo data has no raw files. Load real data to query raw files')).toBeVisible();

  const editor = page.getByLabel('SQL query');
  await editor.fill('SELECT 1 AS one');
  await editor.press('Control+Enter');
  await expect(page.getByTestId('sql-summary')).toContainText('1 row in');
  await expect(page.getByTestId('sql-grid')).toContainText('one');

  const starters = page.getByLabel('Starter queries');
  await starters.selectOption({ label: 'Nights with HRV and load' });
  await expect(editor).toHaveValue(/FROM nights/);
  await run.click();
  await expect(page.getByTestId('sql-summary')).toContainText(/\d[\d,]* rows in/);
  await expect(page.getByTestId('sql-grid')).toContainText('ln_rmssd');
  const rows = Number((await page.getByTestId('sql-summary').innerText()).match(/^([\d,]+) rows/)![1]!.replace(/,/g, ''));
  expect(rows).toBeGreaterThan(300);

  // a 50,000-row query is capped at 10,000 and the grid renders only the rows in view
  await editor.fill('SELECT i, i * 2 AS twice FROM range(50000) t(i)');
  await run.click();
  await expect(page.getByTestId('sql-summary')).toContainText('first 10,000');
  expect(await page.getByTestId('sql-grid').locator('tbody tr').count()).toBeLessThan(120);

  // CSV download
  const [dl] = await Promise.all([page.waitForEvent('download'), page.getByRole('button', { name: 'Download CSV' }).click()]);
  expect(dl.suggestedFilename()).toBe('query.csv');

  // saved queries persist in localStorage
  await page.getByLabel('Name to save the query under').fill('twice');
  await page.getByRole('button', { name: 'Save query' }).click();
  expect(await page.evaluate(() => JSON.parse(localStorage.getItem('wl.sql') ?? '[]'))).toEqual([{ name: 'twice', sql: 'SELECT i, i * 2 AS twice FROM range(50000) t(i)' }]);

  // days built from the engine in demo mode
  await starters.selectOption({ label: 'Monthly stress hours' });
  await run.click();
  await expect(page.getByTestId('sql-summary')).toContainText(/\d+ rows in/);
  expect(errors).toEqual([]);
});
