import { test } from 'node:test';
import assert from 'node:assert/strict';

// theme.ts reads the stored preference while loading, so stub storage before importing it.
Object.defineProperty(globalThis, 'localStorage', {
  configurable: true,
  value: { getItem: () => null, setItem: () => undefined },
});
const { normalizeAccent, normalizeMode } = await import('../src/shared/theme.ts');

test('an unknown stored mode falls back to following the system', () => {
  assert.equal(normalizeMode(null), 'system');
  assert.equal(normalizeMode('sepia'), 'system');
  assert.equal(normalizeMode(''), 'system');
  assert.equal(normalizeMode('light'), 'light');
  assert.equal(normalizeMode('dark'), 'dark');
  assert.equal(normalizeMode('system'), 'system');
});

test('an unknown stored accent falls back to the default palette', () => {
  assert.equal(normalizeAccent(null), 'forest');
  assert.equal(normalizeAccent('chartreuse'), 'forest');
  assert.equal(normalizeAccent('ocean'), 'ocean');
  assert.equal(normalizeAccent('violet'), 'violet');
  assert.equal(normalizeAccent('amber'), 'amber');
});
