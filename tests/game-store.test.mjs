import { beforeEach, test } from 'node:test';
import assert from 'node:assert/strict';
import { createPinia, setActivePinia } from 'pinia';
import { gameApi } from '../src/features/game/api.ts';
import { useGameStore } from '../src/features/game/store.ts';
import { decodeReport } from '../src/features/game/types.ts';
import { preferenceError } from '../src/shared/preferences.ts';

const game = path => ({ path, executable: `${path}/StardewValley`, runtime: 'linux', sources: ['steam'], smapiPath: null });
let storage;
beforeEach(() => {
  storage = new Map();
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: {
    getItem: key => storage.get(key) ?? null,
    setItem: (key, value) => storage.set(key, value),
  } });
  preferenceError.value = false;
  setActivePinia(createPinia());
  gameApi.discover = async () => ({ installations: [], warnings: [] });
  gameApi.pick = async () => null;
  gameApi.validate = async () => { throw { code: 'invalidGameDirectory' }; };
});

test('one result is selected and persisted; multiple copies require a choice', async () => {
  gameApi.discover = async () => ({ installations: [game('/one')], warnings: [] });
  const store = useGameStore();
  await store.scan();
  assert.equal(store.selected.path, '/one');
  assert.equal(storage.get('lingrid.gamePath.v1'), '/one');
  storage.clear();
  setActivePinia(createPinia());
  gameApi.discover = async () => ({ installations: [game('/one'), game('/two')], warnings: [] });
  const multiple = useGameStore();
  await multiple.scan();
  assert.equal(multiple.selected, undefined);
});

test('restores a valid manual path even when it is outside discovery locations', async () => {
  storage.set('lingrid.gamePath.v1', '/custom');
  gameApi.validate = async path => game(path);
  const store = useGameStore();
  await store.scan();
  assert.equal(store.selected.path, '/custom');
  assert.equal(store.installations.length, 1);
});

test('a missing saved copy never silently switches to another installation', async () => {
  storage.set('lingrid.gamePath.v1', '/removed');
  gameApi.discover = async () => ({ installations: [game('/other')], warnings: [] });
  const store = useGameStore();
  await store.scan();
  assert.equal(store.selected, undefined);
  assert.equal(store.selectionNotice, true);
  assert.equal(storage.get('lingrid.gamePath.v1'), '/removed');
});

test('cancelling the picker preserves selection without showing an error', async () => {
  const store = useGameStore();
  gameApi.validate = async path => game(path);
  await store.addDirectory('/existing');
  await store.addDirectory();
  assert.equal(store.selected.path, '/existing');
  assert.equal(store.error, null);
});

test('busy guard prevents overlapping operations and recovers after an error', async () => {
  let fail;
  let scans = 0;
  gameApi.discover = () => { scans++; return new Promise((_, reject) => { fail = reject; }); };
  const store = useGameStore();
  const pending = store.scan();
  await store.scan();
  await store.addDirectory('/ignored');
  assert.equal(scans, 1);
  assert.equal(store.busy, true);
  fail({ code: 'discoveryFailed', detail: 'fixture' });
  await pending;
  assert.equal(store.busy, false);
  assert.equal(store.error.code, 'discoveryFailed');
  gameApi.discover = async () => ({ installations: [], warnings: [] });
  await store.scan();
  assert.equal(store.error, null);
  assert.equal(store.scanned, true);
});

test('storage errors are visible but do not prevent directory selection', async () => {
  localStorage.setItem = () => { throw new Error('storage unavailable'); };
  gameApi.validate = async path => game(path);
  const store = useGameStore();
  await store.addDirectory('/valid');
  assert.equal(store.selected.path, '/valid');
  assert.equal(preferenceError.value, true);
});

test('IPC rejects incompatible fields and accepts the Rust camelCase contract', () => {
  assert.deepEqual(decodeReport({ installations: [game('/game')], warnings: [] }).installations[0], game('/game'));
  assert.throws(() => decodeReport({ installations: [{ ...game('/game'), smapiPath: undefined }], warnings: [] }));
  assert.throws(() => decodeReport({ installations: [{ ...game('/game'), runtime: 'unknown' }], warnings: [] }));
  assert.throws(() => decodeReport({ installations: [{ ...game('/game'), runtime: ['linux'] }], warnings: [] }));
  assert.throws(() => decodeReport({ installations: [], warnings: [{}] }));
});
