import { beforeEach, test } from 'node:test';
import assert from 'node:assert/strict';
import { createPinia, setActivePinia } from 'pinia';
import { modsApi } from '../src/features/mods/api.ts';
import { useModsStore } from '../src/features/mods/store.ts';
import { decodeMod, decodeMods, decodeReport } from '../src/features/mods/types.ts';

const mod = overrides => ({
  folder: 'Alpha',
  path: '/game/Mods/Alpha',
  enabled: true,
  name: 'Alpha',
  author: 'A',
  version: '1.0.0',
  uniqueId: 'A.Alpha',
  description: null,
  nexusId: 1234,
  manifestMissing: false,
  ...overrides,
});

let calls;
beforeEach(() => {
  calls = [];
  setActivePinia(createPinia());
  modsApi.list = async () => { calls.push('list'); return [mod()]; };
  modsApi.setEnabled = async (path, folder, enabled) => { calls.push(`enabled:${folder}:${enabled}`); };
  modsApi.remove = async (path, folder) => { calls.push(`remove:${folder}`); };
  modsApi.pickArchive = async () => '/tmp/SomeMod.zip';
  modsApi.install = async (path, archive) => { calls.push(`install:${archive}`); return { folders: ['SomeMod'] }; };
});

test('the list follows the selected game and is not fetched twice', async () => {
  const store = useModsStore();

  await store.refresh('/game');
  assert.equal(store.mods.length, 1);
  assert.equal(store.enabled.length, 1);
  assert.equal(store.disabled.length, 0);

  await store.refresh('/game');
  assert.equal(calls.filter(call => call === 'list').length, 1, 'unchanged selection is not refetched');

  store.refresh('/other', true);
  assert.equal(store.mods.length, 0, 'a new selection clears the list first');
});

test('toggling a mod uses the dot convention and reloads', async () => {
  const store = useModsStore();
  await store.refresh('/game');

  await store.setEnabled('/game', 'Alpha', false);

  assert.deepEqual(calls, ['list', 'enabled:Alpha:false', 'list']);
  assert.equal(store.notice, 'mods.disabledNotice');
  assert.equal(store.busy, false);
});

test('deleting needs an explicit confirmation first', async () => {
  const store = useModsStore();
  await store.refresh('/game');

  store.askRemove('Alpha');
  assert.equal(store.confirmingRemove, 'Alpha');
  store.cancelRemove();
  assert.equal(store.confirmingRemove, null);
  assert.ok(!calls.includes('remove:Alpha'), 'cancelling does nothing');

  store.askRemove('Alpha');
  await store.remove('/game', 'Alpha');

  assert.deepEqual(calls, ['list', 'remove:Alpha', 'list']);
  assert.equal(store.confirmingRemove, null);
  assert.equal(store.notice, 'mods.removedNotice');
});

test('installing asks for an archive and skips when the picker is cancelled', async () => {
  const store = useModsStore();
  await store.refresh('/game');

  await store.install('/game');
  assert.ok(calls.includes('install:/tmp/SomeMod.zip'));
  assert.equal(store.notice, 'mods.installedNotice');

  calls = [];
  modsApi.pickArchive = async () => null;
  await store.install('/game');
  assert.deepEqual(calls, [], 'cancelling the picker installs nothing');
  assert.equal(store.notice, null);
});

test('failures are surfaced with their error code', async () => {
  modsApi.setEnabled = async () => { throw { code: 'modRenameFailed', detail: 'fixture' }; };
  const store = useModsStore();
  await store.refresh('/game');

  await store.setEnabled('/game', 'Alpha', false);

  assert.equal(store.error.code, 'modRenameFailed');
  assert.equal(store.busy, false);
});

test('mod payloads are validated at the IPC boundary', () => {
  assert.deepEqual(decodeMod(mod()), mod());
  assert.deepEqual(decodeMods([mod()]), [mod()]);
  assert.throws(() => decodeMod(mod({ enabled: 'yes' })));
  assert.throws(() => decodeMod(mod({ nexusId: '1234' })));
  assert.throws(() => decodeMod(mod({ uniqueId: undefined })));
  assert.throws(() => decodeMods({ mods: [] }));

  assert.deepEqual(decodeReport({ folders: ['A'] }), { folders: ['A'] });
  assert.throws(() => decodeReport({ folders: [1] }));
  assert.throws(() => decodeReport({}));
});
