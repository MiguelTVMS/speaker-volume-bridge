import { expect, test } from '@playwright/test';

for (const edition of ['direct_macos', 'direct_windows', 'debian']) {
  test(`${edition} selects preview policy without changing automatic checks`, async ({ page }) => {
    await page.goto(`/preview.html?edition=${edition}&policyDelay=300`);
    await page.getByRole('button', { name: 'Updates', exact: true }).click();
    const selector = page.getByRole('combobox', { name: 'Release policy' });
    await expect(selector).toHaveValue('stable');
    await expect(page.getByText(/Prereleases includes public Alpha\/Beta/)).toBeVisible();
    await page.locator('#automatic-update-checks').uncheck();
    await selector.selectOption('prereleases');
    await expect(page.locator('#update-state')).toHaveText('Checking for updates…');
    await expect(page.getByRole('button', { name: 'Open update page' })).toHaveCount(0);
    await expect(page.locator('#update-state')).toContainText('is available');
    await expect(selector).toHaveValue('prereleases');
    await expect(page.locator('#automatic-update-checks')).not.toBeChecked();
    await selector.selectOption('stable');
    await expect(selector).toHaveValue('stable');
    await expect(page.getByText(/you will receive the next newer stable release/)).toBeVisible();
  });
}

for (const edition of [
  'microsoft_store',
  'mac_app_store',
  'windows_sideload',
  'unknown',
  'development',
]) {
  test(`${edition} excludes release selector`, async ({ page }) => {
    await page.goto(`/preview.html?edition=${edition}`);
    await page.getByRole('button', { name: 'Updates', exact: true }).click();
    await expect(page.getByRole('combobox', { name: 'Release policy' })).toHaveCount(0);
  });
}

test('policy failure exposes an actionable state and keeps selection', async ({ page }) => {
  await page.goto('/preview.html?policyFailure=true');
  await page.getByRole('button', { name: 'Updates', exact: true }).click();
  await page.getByRole('combobox', { name: 'Release policy' }).selectOption('prereleases');
  await expect(page.locator('#update-state')).toContainText('Check again to retry');
  await expect(page.getByRole('combobox', { name: 'Release policy' })).toHaveValue('prereleases');
  await expect(page.getByRole('button', { name: 'Check for updates' })).toBeEnabled();
});

test('dark dropdown menus give every option an explicit readable surface', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.goto('/preview.html?appearance=dark');
  for (const section of ['Devices', 'Night schedule', 'Updates']) {
    await page.getByRole('button', { name: section, exact: true }).click();
    const colors = await page
      .locator('section:not([hidden]) select option')
      .evaluateAll((options) =>
        options.map((option) => {
          const style = getComputedStyle(option);
          return { foreground: style.color, background: style.backgroundColor };
        }),
      );
    expect(colors.length).toBeGreaterThan(0);
    for (const color of colors) {
      expect(color.background).not.toBe('rgba(0, 0, 0, 0)');
      const luminance = (rgb: string) => {
        const channels = rgb
          .match(/\d+/g)!
          .slice(0, 3)
          .map(Number)
          .map((channel) => {
            const value = channel / 255;
            return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
          });
        return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
      };
      const foreground = luminance(color.foreground);
      const background = luminance(color.background);
      expect(
        (Math.max(foreground, background) + 0.05) / (Math.min(foreground, background) + 0.05),
      ).toBeGreaterThanOrEqual(4.5);
    }
  }
});
