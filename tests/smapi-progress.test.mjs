import { test } from 'node:test';
import assert from 'node:assert/strict';
import { decodeProgress, mergeProgress } from '../src/features/smapi/types.ts';

const event = overrides => ({
  step: 'download',
  state: 'running',
  detail: null,
  downloaded: 0,
  total: null,
  elapsedMs: 0,
  ...overrides,
});

test('steps keep the order they started in and update in place', () => {
  let jobs = [];
  jobs = mergeProgress(jobs, event({ step: 'release', state: 'running' }));
  jobs = mergeProgress(jobs, event({ step: 'release', state: 'done', elapsedMs: 120 }));
  jobs = mergeProgress(jobs, event({ step: 'download', state: 'running', total: 100 }));
  jobs = mergeProgress(jobs, event({ step: 'download', state: 'running', downloaded: 60, total: 100 }));
  jobs = mergeProgress(jobs, event({ step: 'download', state: 'done', downloaded: 100, total: 100 }));

  assert.deepEqual(jobs.map(job => job.step), ['release', 'download']);
  assert.equal(jobs[0].state, 'done');
  assert.equal(jobs[0].elapsedMs, 120);
  assert.equal(jobs[1].downloaded, 100);
  assert.equal(jobs[1].state, 'done');
});

test('a skipped download is reported as done with a cached note', () => {
  const jobs = mergeProgress([], event({ step: 'download', state: 'done', detail: 'cached' }));
  assert.equal(jobs[0].detail, 'cached');
  assert.equal(jobs[0].state, 'done');
});

test('a failing step keeps the error code for the UI to translate', () => {
  const jobs = mergeProgress([], event({ step: 'verify', state: 'failed', detail: 'smapiChecksumMismatch' }));
  assert.equal(jobs[0].detail, 'smapiChecksumMismatch');
  assert.equal(jobs[0].state, 'failed');
});

test('progress events are validated at the IPC boundary', () => {
  assert.deepEqual(decodeProgress(event()), event());
  assert.throws(() => decodeProgress(event({ step: 'teleport' })));
  assert.throws(() => decodeProgress(event({ state: 'maybe' })));
  assert.throws(() => decodeProgress(event({ elapsedMs: 'soon' })));
  assert.throws(() => decodeProgress(event({ downloaded: undefined })));
  assert.throws(() => decodeProgress(event({ total: 'many' })));
});
