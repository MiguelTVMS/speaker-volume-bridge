import { expect, test } from '@playwright/test';

for (const platform of ['macos', 'windows', 'linux']) {
  test(`${platform} brand lines have matching visible widths`, async ({ page }) => {
    await page.goto(`/preview.html?platform=${platform}`);
    const brand = page.locator('h1 .sonos-name');
    await expect(brand).toHaveText('SPEAKER');
    await expect(brand).toHaveCSS('letter-spacing', /^(normal|0px)$/);
    await expect(brand).toHaveCSS('transform', 'none');
    const first = await brand.boundingBox();
    const title = await page.locator('h1 > span').last().boundingBox();
    expect(first && title).toBeTruthy();
    expect(Math.abs(first!.x - title!.x)).toBeLessThan(1);
    expect(Math.abs(first!.width - title!.width)).toBeLessThan(1);
    expect(first!.height).toBeGreaterThan(title!.height);
  });

  test(`${platform} update metadata has no nested card`, async ({ page }) => {
    await page.goto(`/preview.html?platform=${platform}`);
    await page.getByRole('button', { name: 'Updates', exact: true }).click();
    const metadata = page.locator('[data-update] > .status-list');
    await expect(metadata).toHaveCSS('border-radius', platform === 'windows' ? '4px' : '0px');
    await expect(metadata).toHaveCSS('box-shadow', 'none');
    await expect(metadata).toHaveCSS('border-top-width', platform === 'windows' ? '1px' : '0px');
    for (const row of await metadata.locator('div').all()) {
      await expect(row).toHaveCSS('border-radius', '0px');
      await expect(row).toHaveCSS('border-left-width', '0px');
    }
    await expect(page.locator('#update-policy')).toBeVisible();
    if (platform === 'windows') {
      await expect(page.locator('#download-update')).toHaveCSS('border-radius', '4px');
      await expect(page.locator('#download-update')).toHaveCSS('min-height', '34px');
      await expect(page.locator('#download-update')).toHaveCSS('font-weight', '400');
      const guidance = page.locator('[data-update] > .setting-note').first();
      await expect(guidance).toHaveCSS('border-top-width', '0px');
      await expect(guidance).toHaveCSS('margin-top', '-4px');
      await expect(page.locator('.update-actions').first()).toHaveCSS(
        'justify-content',
        'flex-end',
      );
    }
  });

  test(`${platform} demo boots and supports hardware-free settings edits and reset`, async ({
    page,
  }) => {
    await page.goto(`/preview.html?platform=macos&ui=${platform}`);
    await expect(page.locator('html')).toHaveAttribute('data-platform', platform);
    await expect(page.getByText('UI demo · simulated devices', { exact: true })).toBeVisible();
    await expect(page.getByRole('combobox', { name: /^Sonos speaker/ })).toHaveValue(
      'sample-speaker',
    );
    await page.getByRole('button', { name: 'Speaker', exact: true }).click();
    const speech = page.locator('[data-speaker-setting="speechEnhancement"]');
    await speech.check();
    await expect(speech).toBeChecked();
    await page.getByRole('button', { name: 'General', exact: true }).click();
    await page.locator('[name="startAtLogin"]').check();
    await expect(page.locator('#notice')).toBeEmpty();
    await page.getByRole('button', { name: 'Speaker', exact: true }).click();
    await expect(speech).toBeChecked();
    await page.getByRole('button', { name: 'Night schedule', exact: true }).click();
    await page.locator('.schedule-cell').first().click();
    await page.getByRole('button', { name: 'Save schedule', exact: true }).click();
    await expect(page.locator('.schedule-cell').first()).toHaveAttribute('aria-selected', 'true');
    await page.getByRole('button', { name: 'Diagnostics', exact: true }).click();
    await page.getByRole('button', { name: 'Reset settings', exact: true }).click();
    await expect(page.locator('#notice')).toBeEmpty();
    await page.getByRole('button', { name: 'Speaker', exact: true }).click();
    await expect(speech).not.toBeChecked();
    await page.getByRole('button', { name: 'General', exact: true }).click();
    await expect(page.locator('[name="startAtLogin"]')).not.toBeChecked();
    await page.getByRole('button', { name: 'Updates', exact: true }).click();
    await expect(page.getByText('Version 1.8.0 is available.')).toBeVisible();
    await expect(page.getByText('Direct download for macOS')).toBeVisible();
    await page.getByRole('button', { name: 'Later', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Later selected' })).toBeDisabled();
    await page.getByRole('button', { name: 'Check for updates' }).click();
    await expect(page.getByRole('button', { name: 'Open release' })).toBeVisible();
  });
}

test('reset cancels a pending settings autosave', async ({ page }) => {
  await page.goto('/preview.html?platform=macos');
  await expect(page.locator('#runtime-status')).toBeVisible();
  await page.clock.install();
  await page.clock.pauseAt(new Date());
  await page.getByRole('button', { name: 'General', exact: true }).click();
  await page.locator('[name="startAtLogin"]').check();
  await page.getByRole('button', { name: 'Diagnostics', exact: true }).click();
  await page.getByRole('button', { name: 'Reset settings', exact: true }).click();
  await expect(page.locator('#notice')).toBeEmpty();
  await page.clock.fastForward(1000);
  await expect(page.locator('#notice')).toBeEmpty();
  await page.getByRole('button', { name: 'General', exact: true }).click();
  await expect(page.locator('[name="startAtLogin"]')).not.toBeChecked();
});

test('update controls are grouped and the HTTPS repository link uses the native opener', async ({
  page,
}) => {
  await page.goto('/preview.html?platform=macos');
  await page.getByRole('button', { name: 'Updates', exact: true }).click();
  await expect(page.locator('#update-state')).toHaveText('Version 1.8.0 is available.');
  await expect(page.locator('.update-actions button')).toHaveCount(4);
  const notifications = page.getByRole('switch', { name: /Update notifications/ });
  await notifications.uncheck();
  await expect(notifications).not.toBeChecked();

  await page.evaluate(() => {
    const host = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (command: string, args?: unknown) => Promise<unknown> };
      openedRepository?: string;
    };
    const invoke = host.__TAURI_INTERNALS__.invoke;
    host.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === 'open_project_repository') host.openedRepository = command;
      if (
        command === 'set_update_notifications' &&
        (args as { enabled?: boolean } | undefined)?.enabled
      )
        return false;
      return invoke(command, args);
    };
  });
  await notifications.click();
  await expect(notifications).not.toBeChecked();
  await expect(page.locator('#notice')).toHaveText(
    'Notifications remain disabled in system settings.',
  );
  await expect(page.locator('#update-available-notice')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Download update' })).toBeVisible();
  await page.getByRole('button', { name: 'About', exact: true }).click();
  const repository = page.getByRole('link', {
    name: 'https://github.com/MiguelTVMS/speaker-volume-bridge',
  });
  await expect(repository).toHaveAttribute(
    'href',
    'https://github.com/MiguelTVMS/speaker-volume-bridge',
  );
  await repository.click();
  await expect
    .poll(() =>
      page.evaluate(() => (window as unknown as { openedRepository?: string }).openedRepository),
    )
    .toBe('open_project_repository');
});

test('reset follows an autosave already in flight', async ({ page }) => {
  await page.goto('/preview.html?platform=macos');
  await expect(page.locator('#runtime-status')).toBeVisible();
  await page.evaluate(() => {
    const host = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (command: string, args?: unknown) => Promise<unknown> };
      releaseSave: () => void;
      saveStarted: boolean;
      writes: string[];
    };
    const invoke = host.__TAURI_INTERNALS__.invoke;
    const gate = new Promise<void>((resolve) => {
      host.releaseSave = resolve;
    });
    host.writes = [];
    host.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command === 'save_configuration') {
        host.saveStarted = true;
        await gate;
      }
      if (['save_configuration', 'reset_configuration'].includes(command))
        host.writes.push(command);
      return invoke(command, args);
    };
  });
  await page.getByRole('button', { name: 'General', exact: true }).click();
  await page.locator('[name="startAtLogin"]').check();
  await expect
    .poll(() => page.evaluate(() => (window as unknown as { saveStarted: boolean }).saveStarted))
    .toBe(true);
  await page.getByRole('button', { name: 'Diagnostics', exact: true }).click();
  await page.getByRole('button', { name: 'Reset settings', exact: true }).click();
  await page.evaluate(() => (window as unknown as { releaseSave: () => void }).releaseSave());
  await expect(page.locator('#notice')).toBeEmpty();
  await expect
    .poll(() => page.evaluate(() => (window as unknown as { writes: string[] }).writes))
    .toEqual(['save_configuration', 'reset_configuration']);
  await page.getByRole('button', { name: 'General', exact: true }).click();
  await expect(page.locator('[name="startAtLogin"]')).not.toBeChecked();
});
