import { beforeEach, test } from 'node:test';
import assert from 'node:assert/strict';
import { createPinia, setActivePinia } from 'pinia';
import { smapiApi } from '../src/features/smapi/api.ts';
import { useSmapiStore } from '../src/features/smapi/store.ts';
import { decodeLog, decodeUninstallReport } from '../src/features/smapi/types.ts';

const log = overrides => ({
  path: '/home/x/.config/StardewValley/ErrorLogs/SMAPI-latest.txt',
  exists: true,
  modifiedMs: 1_700_000_000_000,
  size: 12,
  content: '[INFO SMAPI] hello',
  truncated: false,
  ...overrides,
});

beforeEach(() => {
  setActivePinia(createPinia());
  smapiApi.status = async () => ({ installed: true, version: '4.5.2', executable: '/game/StardewModdingAPI' });
  smapiApi.log = async () => log();
  smapiApi.uninstall = async () => ({ removed: 5, modsRemoved: 0, launcherRestored: true });
});

test('the game log is loaded on demand', async () => {
  const store = useSmapiStore();
  assert.equal(store.log, null);

  await store.loadLog();

  assert.equal(store.log.exists, true);
  assert.equal(store.log.content, '[INFO SMAPI] hello');
  assert.equal(store.logBusy, false);
});

test('a missing log is reported as state, not as an error', async () => {
  smapiApi.log = async () => log({ exists: false, content: '', size: 0, modifiedMs: null });
  const store = useSmapiStore();

  await store.loadLog();

  assert.equal(store.log.exists, false);
  assert.equal(store.error, null);
});

test('a failed log read surfaces the error code', async () => {
  smapiApi.log = async () => { throw { code: 'smapiLogUnreadable' }; };
  const store = useSmapiStore();

  await store.loadLog();

  assert.equal(store.error.code, 'smapiLogUnreadable');
  assert.equal(store.logBusy, false);
});

test('uninstalling asks first and passes the mods choice through', async () => {
  let received = null;
  smapiApi.uninstall = async (path, removeMods) => {
    received = { path, removeMods };
    return { removed: 7, modsRemoved: 4, launcherRestored: true };
  };
  const store = useSmapiStore();
  await store.refresh('/game');

  store.askUninstall();
  assert.equal(store.confirmingUninstall, true);
  store.removeMods = true;
  await store.uninstall('/game');

  assert.deepEqual(received, { path: '/game', removeMods: true });
  assert.equal(store.confirmingUninstall, false);
  assert.equal(store.removeMods, false, 'the choice is reset after use');
  assert.equal(store.uninstallReport.modsRemoved, 4);
  assert.equal(store.notice, 'smapi.uninstalledNotice');
});

test('cancelling the confirmation drops the mods choice', async () => {
  const store = useSmapiStore();
  store.askUninstall();
  store.removeMods = true;

  store.cancelUninstall();

  assert.equal(store.confirmingUninstall, false);
  assert.equal(store.removeMods, false);
});

test('switching games clears the log and the confirmation', async () => {
  const store = useSmapiStore();
  await store.refresh('/one');
  await store.loadLog();
  store.askUninstall();
  assert.notEqual(store.log, null);

  await store.refresh('/two');

  assert.equal(store.log, null);
  assert.equal(store.confirmingUninstall, false);
  assert.equal(store.uninstallReport, null);
});

test('uninstall and log payloads are validated at the IPC boundary', () => {
  const report = { removed: 1, modsRemoved: 0, launcherRestored: false };
  assert.deepEqual(decodeUninstallReport(report), report);
  assert.throws(() => decodeUninstallReport({ ...report, modsRemoved: '0' }));
  assert.throws(() => decodeUninstallReport({ removed: 1, launcherRestored: true }));

  assert.deepEqual(decodeLog(log()), log());
  assert.throws(() => decodeLog(log({ modifiedMs: 'now' })));
  assert.throws(() => decodeLog(log({ exists: 'yes' })));
  assert.throws(() => decodeLog({ ...log(), content: undefined }));
});
