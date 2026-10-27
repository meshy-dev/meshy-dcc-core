/// The sidebar of `meshy-dcc ui` stays put while a long page scrolls (issue #8).

import { expect, test } from '@playwright/test';

// A window of each layout: the wide one, and one between 521px and the 760px
// breakpoint, which used to stack the sidebar above a page that scrolled as a
// whole. Phone widths (520px and below) still stack and are not covered here.
const WINDOWS = [
  { name: 'wide', width: 1280, height: 600 },
  { name: 'narrow', width: 700, height: 800 },
];

for (const win of WINDOWS) {
  test(`the sidebar stays fixed while a long page scrolls (${win.name} window)`, async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.setItem('meshy-dcc-admin-locale', 'en');
      localStorage.setItem('meshy-dcc-ui-start-dismissed', '1');
    });
    await page.setViewportSize({ width: win.width, height: win.height });
    await page.goto('/admin/?panel=connectors');
    const sidebar = page.locator('.pd-sidebar');
    const stage = page.locator('.main-stage');
    await expect(sidebar).toBeVisible();

    // Connectors must be taller than the window, or this proves nothing.
    const overflow = await stage.evaluate((el) => el.scrollHeight - el.clientHeight);
    expect(overflow).toBeGreaterThan(50);

    const lastItem = sidebar.locator('a, button').last();
    await expect(lastItem).toBeInViewport({ ratio: 1 });

    await stage.hover();
    await page.mouse.wheel(0, 5000);
    await expect.poll(() => stage.evaluate((el) => el.scrollTop)).toBeGreaterThan(0);

    // Only the stage scrolled: the page itself did not move, the sidebar still
    // spans the whole window, and its last item (the foot of the nav) is in view.
    expect(await page.evaluate(() => window.scrollY)).toBe(0);
    const box = await sidebar.boundingBox();
    expect(box).toMatchObject({ x: 0, y: 0, height: win.height });
    await expect(lastItem).toBeInViewport({ ratio: 1 });
  });
}
