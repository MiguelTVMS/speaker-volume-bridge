import { expect, test } from '@playwright/test';

for (const edition of ['direct_macos', 'direct_windows', 'debian']) {
  test(`${edition} selects preview policy without changing automatic checks`, async ({ page }) => {
    await page.goto(`/preview.html?edition=${edition}&policyDelay=300`);
    await page.getByRole('button', { name: 'Updates', exact: true }).click();
    const selector = page.getByRole('combobox', { name: 'Release policy' });
    await expect(selector).toHaveValue('stable');
    await expect(selector.locator('option')).toHaveText(['Stable only', 'Include prereleases']);
    await expect(page.getByText(/Include prereleases includes public Alpha\/Beta/)).toBeVisible();
    await page.locator('#automatic-update-checks').uncheck();
    await selector.selectOption('prereleases');
    await expect(page.locator('#update-state')).toHaveText('Checking for updates…');
    await expect(page.getByRole('button', { name: 'Open release' })).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Download update' })).toHaveCount(0);
    await expect(page.locator('#update-state')).toContainText('is available');
    await expect(selector).toHaveValue('prereleases');
    await expect(page.locator('#automatic-update-checks')).not.toBeChecked();
    await selector.selectOption('stable');
    await expect(selector).toHaveValue('stable');
    await expect(page.getByText(/you will receive the next newer stable release/)).toBeVisible();
  });
}

for (const edition of ['direct_macos', 'direct_windows', 'debian']) {
  test(`${edition} offers an explicit download without installation`, async ({ page }) => {
    await page.goto(`/preview.html?edition=${edition}`);
    await expect(page.locator('#update-available-notice')).toBeVisible();
    await page.getByRole('button', { name: 'Updates', exact: true }).click();
    await expect(
      page.getByText('Opens the installer download in your browser.', { exact: false }),
    ).toBeVisible();
    await page.evaluate(() => {
      const host = window as unknown as {
        __TAURI_INTERNALS__: { invoke: (command: string, args?: unknown) => Promise<unknown> };
        downloadCommands?: { command: string; args?: unknown }[];
      };
      host.downloadCommands = [];
      const invoke = host.__TAURI_INTERNALS__.invoke;
      host.__TAURI_INTERNALS__.invoke = (command, args) => {
        if (command.startsWith('open_update') || command.includes('install'))
          host.downloadCommands!.push({ command, args });
        return invoke(command, args);
      };
    });
    await page.getByRole('button', { name: 'Download update', exact: true }).click();
    expect(
      await page.evaluate(
        () => (window as unknown as { downloadCommands: unknown[] }).downloadCommands,
      ),
    ).toEqual([{ command: 'open_update_download', args: { version: '1.8.0', generation: 0 } }]);
    await expect(page.getByRole('button', { name: 'Open release' })).toBeVisible();
    const download = await page
      .getByRole('button', { name: 'Download update', exact: true })
      .boundingBox();
    const release = await page
      .getByRole('button', { name: 'Open release', exact: true })
      .boundingBox();
    expect(Math.abs(download!.y - release!.y)).toBeLessThan(2);
    expect(release!.x).toBeGreaterThan(download!.x + download!.width);
    await page.getByRole('button', { name: 'Open release', exact: true }).click();
    expect(
      await page.evaluate(
        () => (window as unknown as { downloadCommands: unknown[] }).downloadCommands,
      ),
    ).toEqual([
      { command: 'open_update_download', args: { version: '1.8.0', generation: 0 } },
      {
        command: 'open_update_page',
        args: {
          version: '1.8.0',
          generation: 0,
          url: 'https://github.com/MiguelTVMS/speaker-volume-bridge/releases/tag/v1.8.0',
        },
      },
    ]);
    await page.getByRole('button', { name: 'Later', exact: true }).click();
    await expect(page.locator('#update-available-notice')).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Download update', exact: true })).toBeVisible();
  });
}

test('download stays bound to the displayed offer while a gesture defers rendering', async ({
  page,
}) => {
  await page.addInitScript(() => {
    Object.assign(window, {
      isTauri: true,
      __TAURI_INTERNALS__: { metadata: { currentWindow: { label: 'main' } } },
    });
  });
  await page.goto('/preview.html');
  await page.getByRole('button', { name: 'Updates', exact: true }).click();
  await expect(page.locator('#update-state')).toHaveText('Version 1.8.0 is available.');
  // Begin a keyboard gesture without ending it, exercising the real render guard.
  await page.locator('#maximum-volume').dispatchEvent('keydown', { key: 'ArrowRight' });
  await page.evaluate(async () => {
    const host = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (command: string, args?: unknown) => Promise<unknown> };
      downloadedOffer?: unknown;
    };
    const invoke = host.__TAURI_INTERNALS__.invoke;
    const status = (await invoke('get_update_status')) as Record<string, unknown>;
    host.__TAURI_INTERNALS__.invoke = (command, args) => {
      if (command === 'open_update_download') host.downloadedOffer = args;
      return invoke(command, args);
    };
    await invoke('plugin:event|emit', {
      event: 'update-status-changed',
      payload: { ...status, availableVersion: '1.9.0', generation: 1 },
    });
  });
  await expect(page.locator('#update-state')).toHaveText('Version 1.8.0 is available.');
  await page.getByRole('button', { name: 'Download update' }).click();
  expect(
    await page.evaluate(() => (window as unknown as { downloadedOffer?: unknown }).downloadedOffer),
  ).toEqual({ version: '1.8.0', generation: 0 });
  await page.locator('#maximum-volume').dispatchEvent('keyup', { key: 'ArrowRight' });
  await page.evaluate(async () => {
    const host = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (command: string, args?: unknown) => Promise<unknown> };
    };
    const status = (await host.__TAURI_INTERNALS__.invoke('get_update_status')) as Record<
      string,
      unknown
    >;
    await host.__TAURI_INTERNALS__.invoke('plugin:event|emit', {
      event: 'update-status-changed',
      payload: { ...status, availableVersion: '1.9.0', generation: 1 },
    });
  });
  await expect(page.locator('#update-state')).toHaveText('Version 1.9.0 is available.');
});

test('an expired download refreshes the offer and allows an explicit retry', async ({ page }) => {
  await page.goto('/preview.html');
  await page.getByRole('button', { name: 'Updates', exact: true }).click();
  await page.locator('#automatic-update-checks').uncheck();
  await page.evaluate(() => {
    const host = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (command: string, args?: unknown) => Promise<unknown> };
    };
    const invoke = host.__TAURI_INTERNALS__.invoke;
    let rejected = false;
    let checks = 0;
    host.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === 'open_update_download') {
        rejected = true;
        throw new Error('Offer expired.');
      }
      const result = await invoke(command, args);
      if (rejected && command === 'check_for_updates' && ++checks === 1)
        return { ...(result as object), offerStale: true };
      return result;
    };
  });
  await page.clock.install();
  await page.clock.fastForward(24 * 60 * 60 * 1000 + 1);
  await expect(page.getByRole('button', { name: 'Download update' })).toBeVisible();
  await page.getByRole('button', { name: 'Download update' }).click();
  await expect(page.getByRole('button', { name: 'Download update' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Open release' })).toHaveCount(0);
  await expect(page.locator('#notice')).not.toContainText('You can also use Open release.');
  await page.getByRole('button', { name: 'Check for updates' }).click();
  await expect(page.getByRole('button', { name: 'Download update' })).toBeEnabled();
  await expect(page.locator('#automatic-update-checks')).not.toBeChecked();
});

test('a browser-opening failure retains the refreshed release-page fallback', async ({ page }) => {
  await page.goto('/preview.html');
  await page.getByRole('button', { name: 'Updates', exact: true }).click();
  await page.evaluate(() => {
    const host = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (command: string, args?: unknown) => Promise<unknown> };
    };
    const invoke = host.__TAURI_INTERNALS__.invoke;
    host.__TAURI_INTERNALS__.invoke = (command, args) =>
      command === 'open_update_download'
        ? Promise.reject(new Error('Browser unavailable.'))
        : invoke(command, args);
  });
  await page.getByRole('button', { name: 'Download update' }).click();
  await expect(page.locator('#notice')).toContainText('You can also use Open release.');
  await expect(page.getByRole('button', { name: 'Open release' })).toBeEnabled();
});

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

test('Check again recovers when a policy write fails before the backend generation changes', async ({
  page,
}) => {
  await page.goto('/preview.html');
  await page.getByRole('button', { name: 'Updates', exact: true }).click();
  await page.evaluate(() => {
    const host = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (command: string, args?: unknown) => Promise<unknown> };
    };
    const invoke = host.__TAURI_INTERNALS__.invoke;
    host.__TAURI_INTERNALS__.invoke = (command, args) =>
      command === 'set_update_policy'
        ? Promise.reject(new Error('Preference could not be saved.'))
        : invoke(command, args);
  });
  await page.getByRole('combobox', { name: 'Release policy' }).selectOption('prereleases');
  await expect(page.locator('#update-state')).toContainText('Check again to retry');
  await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
  await expect(page.locator('#update-state')).toContainText('is available');
  await expect(page.getByRole('combobox', { name: 'Release policy' })).toHaveValue('stable');
  await expect(page.getByRole('button', { name: 'Download update', exact: true })).toBeEnabled();
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

for (const [edition, name] of [
  ['microsoft_store', 'Microsoft Store'],
  ['mac_app_store', 'Mac App Store'],
]) {
  test(`${edition} delegates Updates to its Store`, async ({ page }) => {
    await page.goto(`/preview.html?edition=${edition}`);
    await page.getByRole('button', { name: 'Updates', exact: true }).click();
    await expect(page.locator('#update-state')).toContainText(`Updates are managed by the ${name}`);
    await expect(page.getByRole('button', { name: 'Check for updates' })).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Open release' })).toHaveCount(0);
    await expect(page.locator('#automatic-update-checks')).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Download update' })).toHaveCount(0);
    await expect(page.locator('#update-available-notice')).toHaveCount(0);
    await expect(page.locator('#update-notifications')).toBeDisabled();
    await page.evaluate(() => {
      const host = window as unknown as {
        __TAURI_INTERNALS__: { invoke: (command: string, args?: unknown) => Promise<unknown> };
        storeCommands?: string[];
      };
      host.storeCommands = [];
      const invoke = host.__TAURI_INTERNALS__.invoke;
      host.__TAURI_INTERNALS__.invoke = (command, args) => {
        if (
          command === 'open_update_store' ||
          command === 'check_for_updates' ||
          command === 'open_update_page'
        ) {
          host.storeCommands!.push(command);
        }
        return invoke(command, args);
      };
    });
    await page.getByRole('button', { name: `Open ${name}`, exact: true }).click();
    expect(
      await page.evaluate(() => (window as unknown as { storeCommands: string[] }).storeCommands),
    ).toEqual(['open_update_store']);
    await expect(page.locator('#update-state')).toContainText('Updates are managed by');
  });
}

for (const [locale, hour12] of [
  ['pt-PT', false],
  ['en-PT', false],
  ['en-GB', true],
] as const) {
  test(`last successful check follows native ${locale} region and clock preferences at startup`, async ({
    browser,
  }) => {
    const context = await browser.newContext({ locale: 'en-US', timezoneId: 'Europe/Lisbon' });
    const page = await context.newPage();
    await page.goto(`/preview.html?locale=${locale}&hour12=${hour12}`);
    await page.getByRole('button', { name: 'Updates', exact: true }).click();
    const expected = await page.evaluate(
      ({ locale, hour12 }) =>
        new Intl.DateTimeFormat(locale, { dateStyle: 'short', timeStyle: 'medium', hour12 }).format(
          new Date(1791108000 * 1000),
        ),
      { locale, hour12 },
    );
    await expect(
      page.getByText('Last successful check', { exact: true }).locator('..').locator('dd'),
    ).toHaveText(expected);
    await context.close();
  });
}

test('last successful check refreshes desktop regional preferences on focus', async ({ page }) => {
  await page.goto('/preview.html?locale=en-US&hour12=true');
  await page.getByRole('button', { name: 'Updates', exact: true }).click();
  const value = page
    .getByText('Last successful check', { exact: true })
    .locator('..')
    .locator('dd');
  const before = await value.innerText();
  await page.evaluate(() => {
    const host = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (command: string, args?: unknown) => Promise<unknown> };
    };
    const invoke = host.__TAURI_INTERNALS__.invoke;
    host.__TAURI_INTERNALS__.invoke = (command, args) => {
      if (command === 'get_system_locale') return Promise.resolve('en-PT');
      if (command === 'get_system_hour12') return Promise.resolve(false);
      return invoke(command, args);
    };
    window.dispatchEvent(new Event('focus'));
  });
  const expected = await page.evaluate(() =>
    new Intl.DateTimeFormat('en-PT', {
      dateStyle: 'short',
      timeStyle: 'medium',
      hour12: false,
    }).format(new Date(1791108000 * 1000)),
  );
  await expect(value).toHaveText(expected);
  expect(expected).not.toBe(before);
});

test('an unrepresentable persisted check time leaves Updates usable and manual recovery works', async ({
  page,
}) => {
  await page.goto('/preview.html?locale=en-PT&hour12=false');
  await page.getByRole('button', { name: 'Updates', exact: true }).click();
  await page.evaluate(async () => {
    const host = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (command: string, args?: unknown) => Promise<unknown> };
    };
    const invoke = host.__TAURI_INTERNALS__.invoke;
    const status = (await invoke('get_update_status')) as Record<string, unknown>;
    host.__TAURI_INTERNALS__.invoke = (command, args) =>
      command === 'get_update_status'
        ? Promise.resolve({ ...status, lastSuccessfulCheck: Number.MAX_SAFE_INTEGER })
        : invoke(command, args);
    window.dispatchEvent(new Event('focus'));
  });
  const value = page
    .getByText('Last successful check', { exact: true })
    .locator('..')
    .locator('dd');
  await expect(value).toHaveText('Unavailable');
  await expect(page.getByRole('button', { name: 'Check for updates', exact: true })).toBeEnabled();
  await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
  await expect(value).not.toHaveText('Unavailable');
});

test('cold-start notification selects Updates after frontend subscription', async ({ page }) => {
  await page.addInitScript(() => {
    Object.assign(window, {
      isTauri: true,
      __TAURI_INTERNALS__: { metadata: { currentWindow: { label: 'main' } } },
    });
  });
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/preview.html?platform=windows&edition=direct_windows&pendingUpdates=true');
  await expect(page.getByRole('heading', { name: 'Updates', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Devices', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Devices', exact: true })).toBeVisible();
  await page.evaluate(async () => {
    const host = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (command: string, args?: unknown) => Promise<unknown> };
    };
    await host.__TAURI_INTERNALS__.invoke('plugin:event|emit', { event: 'open-updates' });
  });
  await expect(page.getByRole('heading', { name: 'Updates', exact: true })).toBeVisible();
  expect(errors).toEqual([]);
});
