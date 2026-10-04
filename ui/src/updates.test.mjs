import test from 'node:test';
import assert from 'node:assert/strict';
import { editionLabel, updateStateText } from './updates.ts';

const status = (phase, values = {}) => ({
  phase,
  installedVersion: '1.0.0',
  availableVersion: null,
  edition: 'direct_macos',
  lastSuccessfulCheck: null,
  action: null,
  message: null,
  automaticChecks: true,
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
