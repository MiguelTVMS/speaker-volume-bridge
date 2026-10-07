import test from 'node:test';
import assert from 'node:assert/strict';
import {
  editionLabel,
  storeActionLabel,
  releasePolicyControl,
  updateStateText,
} from './updates.ts';

const status = (phase, values = {}) => ({
  phase,
  installedVersion: '1.0.0',
  availableVersion: null,
  edition: 'direct_macos',
  lastSuccessfulCheck: null,
  action: null,
  message: null,
  automaticChecks: true,
  updateNotifications: true,
  promptDismissed: false,
  ...values,
});

test('every update state has distinct user-facing copy', () => {
  const states = [
    'idle',
    'checking',
    'up_to_date',
    'update_available',
    'unavailable',
    'unsupported',
  ];
  const labels = states.map((phase) =>
    updateStateText(status(phase, { availableVersion: '2.0.0' })),
  );
  assert.equal(new Set(labels).size, states.length);
});

test('unavailable is not described as current and custom builds remain unknown', () => {
  assert.doesNotMatch(updateStateText(status('unavailable')), /up to date/i);
  assert.equal(editionLabel('custom'), 'Unknown or custom build');
});

test('cached offers remain visible but require a fresh check before opening', () => {
  const text = updateStateText(
    status('update_available', { availableVersion: '2.0.0', offerStale: true }),
  );
  assert.match(text, /was available/);
  assert.match(text, /Check again/);
});

test('release dropdown exposes Stable only and Include prereleases with persisted keys', () => {
  const markup = releasePolicyControl(
    status('idle', { prereleaseSupported: true, policy: 'stable' }),
  );
  assert.match(markup, /value="stable" selected>Stable only/);
  assert.match(markup, /value="prereleases">Include prereleases/);
});

test('Store editions explain delegation and expose only their Store action', () => {
  for (const edition of ['microsoft_store', 'mac_app_store']) {
    const managed = status('store_managed', { edition, prereleaseSupported: false });
    assert.match(updateStateText(managed), /Updates are managed by/);
    assert.equal(storeActionLabel(managed), `Open ${editionLabel(edition)}`);
    assert.equal(releasePolicyControl(managed), '');
    assert.doesNotMatch(updateStateText(managed), /up to date|unavailable/i);
  }
  assert.equal(storeActionLabel(status('idle')), null);
});
