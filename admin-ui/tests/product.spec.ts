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
    for (const label of ['Overview', 'Connectors', 'Skills', 'Agents', 'Settings']) {
      await expect(nav.getByRole('button', { name: label, exact: true })).toBeVisible();
    }
    await expect(nav.getByRole('link', { name: /Docs/ })).toHaveAttribute('href', 'https://docs.meshy.ai/en/webapp/plugins/meshy-dcc/introduction');
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
    await expect(list.locator('li').filter({ hasText: 'Maya' })).toContainText('Experimental');
    await shot(page, 'connectors-en');

    const godot = list.locator('li').filter({ hasText: 'Godot' });
    await godot.getByRole('button', { name: 'Add to a project' }).click();
    await godot.getByPlaceholder('Full path of the project folder').fill('/Users/demo/Projects/Platformer');
    await godot.getByRole('button', { name: 'Install', exact: true }).click();
    await expect(godot.getByRole('status')).toContainText('Godot connector 0.1.0 installed');

    await page.getByRole('button', { name: /^Coming soon/ }).click();
    await expect(list).toHaveCount(0);
    await page.getByPlaceholder('Search connectors').fill('houdini');
    await expect(page.locator('.pd-chips li')).toHaveText(['Houdini']);
  });

  test('an app the CLI cannot find asks for its location', async ({ page }) => {
    await page.route('**/admin/api/connectors', async (route) => {
      const body = await (await route.fetch()).json();
      body.installed = body.installed.filter((r: { dcc_type: string }) => r.dcc_type !== 'blender');
      return route.fulfill({ json: body });
    });
    const sent: Record<string, unknown>[] = [];
    await page.route('**/admin/api/connectors/install', (route) => {
      const body = route.request().postDataJSON();
      sent.push(body);
      return route.fulfill({ json: body.dcc_path
        ? { status: 'partial', dcc_type: 'blender', adapter_version: '0.1.0' }
        : { status: 'failed', dcc_type: 'blender', adapter_version: '0.1.0', error: { code: 'DCC_PATH_REQUIRED', message: 'blender was not found' } } });
    });
    await open(page, 'connectors');
    const blender = page.locator('.pd-list li').filter({ hasText: 'Blender' });
    await blender.getByRole('button', { name: 'Install', exact: true }).click();
    await expect(blender.getByRole('status')).toContainText('Blender was not found where it is usually installed');
    await blender.getByPlaceholder('Full path, for example /Applications/Blender.app').fill('/Applications/Blender.app');
    await blender.getByRole('button', { name: 'Install', exact: true }).last().click();
    await expect(blender.getByRole('status')).toContainText('Blender connector 0.1.0 installed');
    expect(sent).toEqual([{ dcc_type: 'blender' }, { dcc_type: 'blender', dcc_path: '/Applications/Blender.app' }]);
  });

  test('skills show what each instance loaded and where the user\'s own skills stand', async ({ page }) => {
    // Own the reload round trip so the result does not depend on earlier runs.
    let reloaded = false;
    await page.route('**/admin/api/skill-library/reload', (route) => {
      reloaded = true;
      return route.fulfill({ json: { ok: true, reloaded: 1, failed: 0, instances: [{ instance_id: 'maya-1234567890', dcc_type: 'maya' }] } });
    });
    await page.route('**/admin/api/skill-library', async (route) => {
      const response = await route.fetch();
      const body = await response.json();
      const state = reloaded ? 'active' : 'pending';
      body.folders[0].skills = body.folders[0].skills.map((skill: { name: string; state: string }) =>
        (skill.name === 'my-cleanup' ? { ...skill, state } : skill));
      return route.fulfill({ response, json: body });
    });
    await open(page, 'skills');

    const card = page.locator('.pd-skill-instance');
    await expect(card).toContainText('shot010.ma');
    await expect(card).toContainText('2 loaded · 6 available');
    await expect(card.locator('.pd-chips li')).toHaveText(['maya-scene', 'my-shot-setup']);

    const all = page.locator('section', { has: page.getByRole('heading', { name: 'All skills' }) });
    await expect(all.locator('.pd-list li').first()).toContainText('my-shot-setup');
    await expect(all.locator('.pd-list li').filter({ hasText: 'maya-scene' })).toContainText('Loaded');
    await all.getByRole('button', { name: 'Mine 1' }).click();
    await expect(all.locator('.pd-list li')).toHaveCount(1);

    const folder = page.locator('.pd-skill-folder').first();
    await expect(folder).toContainText('Maya default folder');
    await expect(folder.locator('li').filter({ hasText: 'maya-render' })).toContainText('Same name as a built-in skill');
    await expect(folder.locator('li').filter({ hasText: 'my-cleanup' })).toContainText('Waiting for reload');
    await expect(page.locator('.pd-prompt')).toContainText('~/.meshy-dcc/maya/skills');
    await expect(page.getByRole('heading', { name: 'Get more skills' })).toBeVisible();
    await shot(page, 'skills-en');

    await page.getByRole('button', { name: 'Reload in open apps' }).click();
    await expect(page.getByRole('status')).toContainText('Instances updated: 1');
    await expect(folder.locator('li').filter({ hasText: 'my-cleanup' })).toContainText('In use');

    const input = page.getByPlaceholder('Full path of another folder');
    await input.fill('team/skills');
    await page.getByRole('button', { name: 'Add folder' }).click();
    await expect(page.getByRole('status')).toContainText('Enter the full path of a folder that exists.');
    await input.fill('/Volumes/Team/skills');
    await page.getByRole('button', { name: 'Add folder' }).click();
    const added = page.locator('.pd-skill-folder').filter({ hasText: '/Volumes/Team/skills' });
    await expect(added).toContainText('Added folder');
    await added.getByRole('button', { name: 'Remove' }).click();
    await expect(added).toHaveCount(0);
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

  test('the daily update check is a saved setting', async ({ page }) => {
    let saved: unknown = null;
    let checks = 0;
    await page.route('**/admin/api/settings', (route) => {
      if (route.request().method() === 'PUT') saved = route.request().postDataJSON();
      return route.fulfill({ json: { update_check: saved === null } });
    });
    await page.route('**/v1/update/check**', (route) => {
      checks += 1;
      return route.fulfill({ json: { update_available: false, current_version: '0.2.0', latest_version: '0.2.0' } });
    });
    await open(page, 'settings');
    const daily = page.getByRole('switch', { name: 'Check for updates once a day' });
    await expect(daily).toHaveAttribute('aria-checked', 'true');
    await expect.poll(() => checks).toBe(1);
    await daily.click();
    await expect(daily).toHaveAttribute('aria-checked', 'false');
    expect(saved).toEqual({ update_check: false });
  });

  test('usage telemetry: a banner until answered, then a switch in Settings', async ({ page }) => {
    const puts: Record<string, unknown>[] = [];
    let state: Record<string, unknown> = {
      update_check: true,
      telemetry: true,
      telemetry_off_reason: null,
      telemetry_notice: true,
      telemetry_docs: 'https://docs.meshy.ai/en/webapp/plugins/meshy-dcc/telemetry',
    };
    await page.route('**/admin/api/settings', (route) => {
      if (route.request().method() === 'PUT') {
        const body = route.request().postDataJSON() as Record<string, unknown>;
        puts.push(body);
        state = { ...state, ...('telemetry' in body ? { telemetry: body.telemetry } : {}), telemetry_notice: false };
      }
      return route.fulfill({ json: state });
    });
    await open(page, 'home');
    const banner = page.getByRole('status').filter({ hasText: 'anonymous, aggregate usage statistics' });
    await expect(banner).toBeVisible();
    await expect(banner.getByRole('link', { name: 'What is sent' })).toHaveAttribute('href', /meshy-dcc\/telemetry$/);
    await shot(page, 'telemetry-banner-en');
    await banner.getByRole('button', { name: 'OK' }).click();
    await expect(banner).toHaveCount(0);

    await page.locator('.pd-sidebar').getByRole('button', { name: 'Settings', exact: true }).click();
    const toggle = page.getByRole('switch', { name: 'Send anonymous usage statistics' });
    await expect(toggle).toHaveAttribute('aria-checked', 'true');
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'false');
    expect(puts).toEqual([{ telemetry_notice_seen: true }, { telemetry: false }]);

    // An environment override (DO_NOT_TRACK, CI, ...) wins and locks the switch.
    state = { ...state, telemetry: true, telemetry_off_reason: 'DO_NOT_TRACK' };
    await page.reload();
    await expect(page.getByText('Turned off on this computer by DO_NOT_TRACK.')).toBeVisible();
    await expect(toggle).toHaveAttribute('aria-checked', 'false');
    await expect(toggle).toBeDisabled();
  });

  test('a problem report is previewed first, and only that text can be sent', async ({ page }) => {
    const calls: { summary: string; send: boolean }[] = [];
    await page.route('**/admin/api/report', (route) => {
      calls.push(route.request().postDataJSON());
      return route.fallback();
    });
    await open(page, 'settings');
    await page.getByRole('button', { name: /Report a problem/ }).click();
    const text = page.getByLabel('What happened?');
    const preview = page.getByRole('button', { name: 'Preview' });
    const send = page.getByRole('button', { name: 'Send', exact: true });
    await expect(preview).toBeDisabled();
    await expect(send).toBeDisabled();

    await text.fill('Blender import failed');
    await expect(page.getByText('21 / 4000')).toBeVisible();
    await expect(send).toBeDisabled();
    await preview.click();
    await expect(page.locator('.pd-report-payload pre')).toContainText('"schema": "meshy-dcc.report.v1"');
    await expect(page.locator('.pd-report-payload summary')).toContainText('KB');
    await expect(send).toBeEnabled();
    await shot(page, 'report-en');

    await text.fill('Blender import failed again');
    await expect(send).toBeDisabled();
    await expect(page.locator('.pd-report-payload')).toHaveCount(0);
    await preview.click();
    await expect(send).toBeEnabled();
    await send.click();
    await expect(page.getByRole('status')).toContainText('Report ID: 20261027T031500Z-1a2b3c4d');
    await expect(page.getByRole('link', { name: 'View reported problems' })).toHaveAttribute('href', /label%3Auser-report$/);
    await expect(send).toBeDisabled();
    expect(calls).toEqual([
      { summary: 'Blender import failed', send: false },
      { summary: 'Blender import failed again', send: false },
      { summary: 'Blender import failed again', send: true },
    ]);

    await page.getByRole('button', { name: '← Settings' }).click();
    await expect(page.getByRole('heading', { name: 'Settings', level: 1 })).toBeVisible();

    // Settings in the sidebar leaves the report view too.
    await page.getByRole('button', { name: /Report a problem/ }).click();
    await page.locator('.pd-sidebar').getByRole('button', { name: 'Settings', exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Settings', level: 1 })).toBeVisible();
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
      for (const panel of ['connectors', 'skills', 'agents', 'settings']) {
        await page.goto(`/admin/?panel=${panel}`);
        await expect(page.locator('.pd-page')).toBeVisible();
        await expect(page.locator('.pd-page')).not.toContainText('product.');
        if (locale !== 'en') await shot(page, `${panel}-${locale}`);
      }
    });
  }
});
