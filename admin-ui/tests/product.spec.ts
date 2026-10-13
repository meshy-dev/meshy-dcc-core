/// The user-facing pages of `meshy-dcc ui`, against the dev-server mocks.
/// Set PRODUCT_SHOTS=<dir> to also save a screenshot of every page and locale.

import path from 'node:path';

import { expect, test, type Page } from '@playwright/test';

import { SUPPORTED_LOCALES } from '../src/i18n';

const SHOTS = process.env.PRODUCT_SHOTS;

async function open(page: Page, panel: string, options: { locale?: string; start?: 'dismissed' | 'shown' } = {}) {
  await page.addInitScript(({ locale, start }) => {
    if (locale) localStorage.setItem('meshy-dcc-admin-locale', locale);
    if (start === 'dismissed') localStorage.setItem('meshy-dcc-ui-start-dismissed', '1');
    else localStorage.removeItem('meshy-dcc-ui-start-dismissed');
  }, { locale: options.locale ?? 'en', start: options.start ?? 'dismissed' });
  await page.setViewportSize({ width: 1280, height: 1000 });
  await page.goto(`/admin/?panel=${panel}`);
  await expect(page.locator('.pd-sidebar')).toBeVisible();
}

async function shot(page: Page, name: string) {
  if (!SHOTS) return;
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(SHOTS, `${name}.png`), fullPage: true });
}

test.describe('meshy-dcc ui', () => {
  test('the sidebar has the three pages, open apps and the links out', async ({ page }) => {
    await open(page, 'home');
    const nav = page.locator('.pd-sidebar');
    await expect(nav.getByText('DCC Core', { exact: true })).toBeVisible();
    await expect(nav.getByRole('button', { name: /Running · Local/ })).toBeVisible();
    for (const label of ['Overview', 'Connectors', 'Agents', 'Settings']) {
      await expect(nav.getByRole('button', { name: label, exact: true })).toBeVisible();
    }
    await expect(nav.getByRole('link', { name: /Docs/ })).toHaveAttribute('href', 'https://docs.meshy.ai');
    await expect(nav.getByRole('link', { name: /Official Website/ })).toHaveAttribute('href', 'https://www.meshy.ai');
    // Developer pages stay hidden until Settings turns them on.
    await expect(nav.getByRole('button', { name: 'Developer' })).toHaveCount(0);
  });

  test('overview shows the core service, open apps and what is installed', async ({ page }) => {
    await open(page, 'home');
    await expect(page.getByRole('heading', { name: 'Meshy DCC Core', level: 1 })).toBeVisible();
    await expect(page.getByText('Local gateway', { exact: true })).toBeVisible();
    await expect(page.getByText('Network access off')).toBeVisible();
    await expect(page.getByText('Stops 30 s after the last app closes')).toBeVisible();
    await expect(page.getByText('Blender connector')).toBeVisible();
    await expect(page.locator('.pd-installed-in')).toContainText('Claude Code');
    await expect(page.getByRole('row').filter({ hasText: 'Maya' })).toContainText('Ready');
    await expect(page.getByRole('row').filter({ hasText: 'Houdini' })).toContainText('Starting');
    await shot(page, 'overview-en');
  });

  test('get started lists three steps and can be closed', async ({ page }) => {
    // No successful call yet, so step 3 is open and the checklist shows.
    await page.route('**/admin/api/stats?range=all', (route) =>
      route.fulfill({ json: { range: 'all', total_calls: 0, successful_calls: 0, success_rate: 0 } }));
    await open(page, 'home', { start: 'shown' });
    await expect(page.getByRole('heading', { name: 'Welcome to Meshy DCC Core' })).toBeVisible();
    await expect(page.locator('.pd-step')).toHaveCount(3);
    await shot(page, 'get-started-en');
    await page.getByRole('button', { name: 'Go to overview' }).click();
    await expect(page.getByRole('heading', { name: 'Meshy DCC Core', level: 1 })).toBeVisible();
    await page.getByRole('button', { name: 'Show getting started' }).click();
    await expect(page.locator('.pd-step')).toHaveCount(3);
  });

  test('connectors are grouped and a per-project connector asks for the folder', async ({ page }) => {
    await open(page, 'connectors');
    const list = page.locator('.pd-list');
    await expect(list.locator('li').filter({ hasText: 'Blender' })).toContainText('Installed');
    await expect(list.locator('li').filter({ hasText: 'Unity' })).toContainText('In 1 project');
    await expect(page.locator('.pd-chips')).toContainText('Roblox Studio');
    await shot(page, 'connectors-en');

    const godot = list.locator('li').filter({ hasText: 'Godot' });
    await godot.getByRole('button', { name: 'Add to a project' }).click();
    await godot.getByPlaceholder('Full path of the project folder').fill('/Users/demo/Projects/Platformer');
    await godot.getByRole('button', { name: 'Install', exact: true }).click();
    await expect(godot.getByRole('status')).toContainText('Godot connector 0.1.0 installed');

    await page.getByRole('button', { name: /^Coming soon/ }).click();
    await expect(list).toHaveCount(0);
    await page.getByPlaceholder('Search connectors').fill('maya');
    await expect(page.locator('.pd-chips li')).toHaveText(['Maya']);
  });

  test('agents found on this computer switch the skill on and off', async ({ page }) => {
    await open(page, 'agents');
    await expect(page.locator('.pd-agent-card').filter({ hasText: 'Codex' })).toContainText('shared with Cline, Warp, Zed');
    const cursor = page.getByRole('switch', { name: 'Meshy DCC skill for Cursor' });
    const before = await cursor.getAttribute('aria-checked');
    await cursor.click();
    await expect(cursor).toHaveAttribute('aria-checked', before === 'true' ? 'false' : 'true');
    await expect(page.locator('.pd-agent-grid li').filter({ hasText: 'GitHub Copilot' })).toBeVisible();
    await expect(page.locator('.pd-code').last()).toContainText('meshy-dcc agent-skill --dir');
    await shot(page, 'agents-en');
  });

  test('settings turn on the collapsed Developer section', async ({ page }) => {
    await open(page, 'settings');
    await shot(page, 'settings-en');
    await page.getByRole('switch', { name: 'Show developer details' }).click();
    const nav = page.locator('.pd-sidebar');
    const developer = nav.getByRole('button', { name: 'Developer' });
    await expect(developer).toHaveAttribute('aria-expanded', 'true');
    await nav.getByRole('button', { name: 'Logs', exact: true }).click();
    await expect(page).toHaveURL(/panel=logs/);
    await developer.click();
    await expect(nav.getByRole('button', { name: 'Logs', exact: true })).toHaveCount(0);
  });

  for (const locale of SUPPORTED_LOCALES) {
    test(`every page renders in ${locale}`, async ({ page }) => {
      await open(page, 'home', { locale });
      await expect(page.locator('html')).not.toContainText('product.');
      await shot(page, `overview-${locale}`);
      for (const panel of ['connectors', 'agents', 'settings']) {
        await page.goto(`/admin/?panel=${panel}`);
        await expect(page.locator('.pd-page')).toBeVisible();
        await expect(page.locator('.pd-page')).not.toContainText('product.');
        if (locale !== 'en') await shot(page, `${panel}-${locale}`);
      }
    });
  }
});
