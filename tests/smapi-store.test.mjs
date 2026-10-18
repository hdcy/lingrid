import { beforeEach, test } from 'node:test';
import assert from 'node:assert/strict';
import { createPinia, setActivePinia } from 'pinia';
import { smapiApi } from '../src/features/smapi/api.ts';
import { useSmapiStore } from '../src/features/smapi/store.ts';
import { decodeOutcome, decodeRelease, decodeStatus } from '../src/features/smapi/types.ts';

const status = (installed, version = null) => ({
  installed,
  version,
  executable: installed ? '/game/StardewModdingAPI' : null,
});
const release = {
  version: '4.5.2',
  assetName: 'SMAPI-4.5.2-installer.zip',
  assetUrl: 'https://example/installer',
  sha256: 'dd01ddca',
  size: 41766494,
};

beforeEach(() => {
  setActivePinia(createPinia());
  smapiApi.status = async () => status(false);
  smapiApi.release = async () => release;
  smapiApi.install = async () => ({ version: '4.5.2', executable: '/game/StardewModdingAPI', launcherReplaced: true });
  smapiApi.uninstall = async () => undefined;
});

test('installing refreshes the local status and reports the outcome', async () => {
  let installed = false;
  smapiApi.status = async () => status(installed, installed ? '4.5.2' : null);
  smapiApi.install = async () => {
    installed = true;
    return { version: '4.5.2', executable: '/game/StardewModdingAPI', launcherReplaced: true };
  };
  const store = useSmapiStore();
  await store.refresh('/game');
  assert.equal(store.installed, false);

  await store.install('/game');

  assert.equal(store.installed, true);
  assert.equal(store.status.version, '4.5.2');
  assert.equal(store.notice, 'smapi.installedNotice');
  assert.equal(store.outcome.launcherReplaced, true);
  assert.equal(store.busy, false);
});

test('uninstalling clears the installed state', async () => {
  let installed = true;
  smapiApi.status = async () => status(installed, installed ? '4.5.2' : null);
  smapiApi.uninstall = async () => { installed = false; };
  const store = useSmapiStore();
  await store.refresh('/game');

  await store.uninstall('/game');

  assert.equal(store.installed, false);
  assert.equal(store.notice, 'smapi.uninstalledNotice');
  assert.equal(store.outcome, null);
});

test('a failed install reports a localized error and recovers', async () => {
  smapiApi.install = async () => { throw { code: 'smapiChecksumMismatch', detail: 'fixture' }; };
  const store = useSmapiStore();

  await store.install('/game');

  assert.equal(store.error.code, 'smapiChecksumMismatch');
  assert.equal(store.error.detail, 'fixture');
  assert.equal(store.notice, null);
  assert.equal(store.busy, false);
});

test('an older known version is outdated, an unknown version is not', async () => {
  smapiApi.status = async () => status(true, '4.5.1');
  const older = useSmapiStore();
  await older.refresh('/old');
  await older.checkRelease();
  assert.equal(older.outdated, true);

  smapiApi.status = async () => status(true, null);
  const manual = useSmapiStore();
  await manual.refresh('/manual');
  await manual.checkRelease();
  assert.equal(manual.installed, true);
  assert.equal(manual.outdated, false);

  smapiApi.status = async () => status(true, '4.5.2');
  const current = useSmapiStore();
  await current.refresh('/current');
  await current.checkRelease();
  assert.equal(current.outdated, false);
});

test('a failed release lookup is visible without losing the install state', async () => {
  smapiApi.status = async () => status(true, '4.5.2');
  smapiApi.release = async () => { throw { code: 'smapiReleaseUnavailable' }; };
  const store = useSmapiStore();
  await store.refresh('/game');

  await store.checkRelease();

  assert.equal(store.error.code, 'smapiReleaseUnavailable');
  assert.equal(store.release, null);
  assert.equal(store.installed, true);
  assert.equal(store.checking, false);
});

test('pages watching the same selection share one status lookup', async () => {
  let calls = 0;
  smapiApi.status = async () => { calls++; return status(false); };
  const store = useSmapiStore();

  await store.refresh('/game');
  await store.refresh('/game');
  assert.equal(calls, 1, 'an unchanged selection is not refetched');

  await store.refresh('/other');
  assert.equal(calls, 2);
  await store.refresh(null);
  assert.equal(store.status, null);
});

test('busy guard prevents overlapping installs', async () => {
  let installs = 0;
  let finish;
  smapiApi.install = () => {
    installs++;
    return new Promise(resolve => { finish = () => resolve({ version: '4.5.2', executable: '/game/StardewModdingAPI', launcherReplaced: true }); });
  };
  const store = useSmapiStore();
  const pending = store.install('/game');
  await store.install('/game');
  assert.equal(installs, 1);
  finish();
  await pending;
  assert.equal(store.busy, false);
});

test('IPC rejects incompatible SMAPI payloads', () => {
  assert.deepEqual(decodeStatus(status(true, '4.5.2')), status(true, '4.5.2'));
  assert.throws(() => decodeStatus({ installed: 'yes', version: null, executable: null }));
  assert.throws(() => decodeStatus({ installed: true, version: 1, executable: null }));
  assert.throws(() => decodeStatus({ installed: true, version: '4.5.2' }));

  assert.deepEqual(decodeRelease(release), release);
  assert.throws(() => decodeRelease({ ...release, sha256: undefined }));
  assert.throws(() => decodeRelease({ ...release, size: '41766494' }));

  const outcome = { version: '4.5.2', executable: '/game/StardewModdingAPI', launcherReplaced: true };
  assert.deepEqual(decodeOutcome(outcome), outcome);
  assert.throws(() => decodeOutcome({ ...outcome, launcherReplaced: 'yes' }));
});
