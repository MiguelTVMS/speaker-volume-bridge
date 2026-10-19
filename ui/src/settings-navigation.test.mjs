import assert from 'node:assert/strict';
import test from 'node:test';
import { adjacentPage, settingsPages, SettingsNavigation } from './settings-navigation.ts';

test('arrows traverse all settings in sidebar order without wrapping', () => {
  assert.equal(adjacentPage('devices', -1), undefined);
  assert.equal(adjacentPage('about', 1), undefined);
  for (let index = 0; index < settingsPages.length - 1; index++) {
    assert.equal(adjacentPage(settingsPages[index], 1), settingsPages[index + 1]);
    assert.equal(adjacentPage(settingsPages[index + 1], -1), settingsPages[index]);
  }
});

test('schedule has a dedicated sidebar destination after speaker controls', () => {
  assert.equal(adjacentPage('speaker', 1), 'schedule');
  assert.equal(adjacentPage('schedule', 1), 'volume');
});

test('ordinary Settings cancels a destination consumed by delayed startup IPC', async () => {
  const opened = [];
  const navigation = new SettingsNavigation((page) => opened.push(page));
  let release;
  const response = new Promise((resolve) => {
    release = resolve;
  });
  const startup = response.then((request) => navigation.accept(request));
  navigation.accept({ revision: 2, page: null });
  release({ revision: 1, page: 'diagnostics' });
  await startup;
  assert.deepEqual(opened, []);
  navigation.accept({ revision: 3, page: 'updates' });
  assert.deepEqual(opened, ['updates']);
});

test('newer tray navigation wins over delayed notification events and duplicate acknowledgement', () => {
  const opened = [];
  const navigation = new SettingsNavigation((page) => opened.push(page));
  navigation.accept({ revision: 2, page: 'diagnostics' });
  navigation.accept({ revision: 1, page: 'updates' });
  navigation.accept({ revision: 2, page: 'diagnostics' });
  assert.deepEqual(opened, ['diagnostics']);
});
