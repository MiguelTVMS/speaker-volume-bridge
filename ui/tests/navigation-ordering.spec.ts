import { expect, test } from '@playwright/test';

for (const platform of ['macos', 'windows', 'linux']) {
  for (const destination of [null, 'updates']) {
    test(`${platform} ${destination ?? 'ordinary Settings'} supersedes consumed startup navigation`, async ({
      page,
    }) => {
      await page.addInitScript(() => {
        Object.assign(window, {
          isTauri: true,
          __TAURI_INTERNALS__: { metadata: { currentWindow: { label: 'main' } } },
        });
      });
      await page.goto(
        `/preview.html?platform=${platform}&pendingSettingsPage=diagnostics&holdNavigation=true`,
      );
      const invoke = async (command: string) =>
        page.evaluate(async (name) => {
          const host = window as unknown as {
            __TAURI_INTERNALS__: { invoke: (command: string) => Promise<unknown> };
          };
          return host.__TAURI_INTERNALS__.invoke(name);
        }, command);
      await expect.poll(() => invoke('preview_navigation_held')).toBe(true);
      await page.evaluate(async (destination) => {
        const host = window as unknown as {
          __TAURI_INTERNALS__: { invoke: (command: string, args?: unknown) => Promise<unknown> };
        };
        const request = await host.__TAURI_INTERNALS__.invoke('preview_settings_navigation', {
          page: destination,
        });
        await host.__TAURI_INTERNALS__.invoke('plugin:event|emit', {
          event: 'open-settings-page',
          payload: request,
        });
        await host.__TAURI_INTERNALS__.invoke('preview_release_navigation');
        // Allow the held IPC continuation to run before observing the next paint.
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      }, destination);
      await expect(
        page.getByRole('heading', { name: destination ? 'Updates' : 'Devices', exact: true }),
      ).toBeVisible();
      await expect(
        page.getByRole('heading', { name: 'Diagnostics', exact: true }),
      ).not.toBeVisible();
    });
  }
}

for (const platform of ['macos', 'windows', 'linux']) {
  test(`${platform} warm delivery is not replayed on frontend initialization`, async ({ page }) => {
    await page.addInitScript(() => {
      Object.assign(window, {
        isTauri: true,
        __TAURI_INTERNALS__: { metadata: { currentWindow: { label: 'main' } } },
      });
    });
    await page.goto(`/preview.html?platform=${platform}`);
    await expect(page.getByRole('heading', { name: 'Devices', exact: true })).toBeVisible();
    await page.evaluate(async () => {
      const host = window as unknown as {
        __TAURI_INTERNALS__: { invoke: (command: string, args?: unknown) => Promise<unknown> };
      };
      const request = await host.__TAURI_INTERNALS__.invoke('preview_settings_navigation', {
        page: 'diagnostics',
      });
      await host.__TAURI_INTERNALS__.invoke('plugin:event|emit', {
        event: 'open-settings-page',
        payload: request,
      });
    });
    await expect(page.getByRole('heading', { name: 'Diagnostics', exact: true })).toBeVisible();
    await page.getByRole('button', { name: 'Devices', exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Devices', exact: true })).toBeVisible();
    // Keep the native preview backend alive while repeating the startup consumer.
    // A full document reload would recreate the mock backend and hide the defect.
    const pending = await page.evaluate(async () => {
      const host = window as unknown as {
        __TAURI_INTERNALS__: { invoke: (command: string) => Promise<unknown> };
      };
      return host.__TAURI_INTERNALS__.invoke('take_pending_settings_navigation');
    });
    expect(pending).toBeNull();
  });
}
