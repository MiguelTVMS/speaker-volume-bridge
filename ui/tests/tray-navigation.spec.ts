import { expect, test } from '@playwright/test';

for (const platform of ['macos', 'windows', 'linux']) {
  test(`${platform} Diagnostics tray navigation works before and after frontend subscription`, async ({
    page,
  }) => {
    await page.addInitScript(() => {
      Object.assign(window, {
        isTauri: true,
        __TAURI_INTERNALS__: { metadata: { currentWindow: { label: 'main' } } },
      });
    });
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.goto(`/preview.html?platform=${platform}&pendingSettingsPage=diagnostics`);
    await expect(page.getByRole('heading', { name: 'Diagnostics', exact: true })).toBeVisible();
    await page.getByRole('button', { name: 'Devices', exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Devices', exact: true })).toBeVisible();
    await page.evaluate(async () => {
      const host = window as unknown as {
        __TAURI_INTERNALS__: { invoke: (command: string, args?: unknown) => Promise<unknown> };
      };
      await host.__TAURI_INTERNALS__.invoke('preview_settings_navigation', { page: 'diagnostics' });
      await host.__TAURI_INTERNALS__.invoke('plugin:event|emit', {
        event: 'open-settings-page',
        payload: 'diagnostics',
      });
    });
    await expect(page.getByRole('heading', { name: 'Diagnostics', exact: true })).toBeVisible();
    expect(errors).toEqual([]);
  });
}

for (const platform of ['macos', 'windows', 'linux']) {
  test(`${platform} newer Diagnostics supersedes a cold update notification`, async ({ page }) => {
    await page.addInitScript(() => {
      Object.assign(window, {
        isTauri: true,
        __TAURI_INTERNALS__: { metadata: { currentWindow: { label: 'main' } } },
      });
    });
    await page.goto(
      `/preview.html?platform=${platform}&pendingUpdates=true&pendingSettingsPage=diagnostics`,
    );
    await expect(page.getByRole('heading', { name: 'Diagnostics', exact: true })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Updates', exact: true })).not.toBeVisible();
  });
}
