export interface SmapiRelease {
  version: string;
  assetName: string;
  assetUrl: string;
  sha256: string | null;
  size: number;
}

export interface SmapiStatus {
  installed: boolean;
  /** Only known for installs performed by this application. */
  version: string | null;
  executable: string | null;
}

export interface SmapiInstallOutcome {
  version: string;
  executable: string;
  launcherReplaced: boolean;
}

export interface SmapiUninstallReport {
  /** Files and folders removed from the game directory. */
  removed: number;
  /** Mods cleared from `Mods`; only non-zero when that option was chosen. */
  modsRemoved: number;
  launcherRestored: boolean;
}

/** The log SMAPI rewrites every time the game runs. */
export interface SmapiLog {
  /** Where the log is expected, whether or not it exists yet. */
  path: string;
  exists: boolean;
  modifiedMs: number | null;
  size: number;
  content: string;
  /** True when the file was larger than the slice loaded into the UI. */
  truncated: boolean;
}

export type SmapiStep = 'release' | 'download' | 'verify' | 'extract' | 'install' | 'uninstall';
export type SmapiStepState = 'running' | 'done' | 'failed';

/** One progress event emitted by the backend. */
export interface SmapiProgress {
  step: SmapiStep;
  state: SmapiStepState;
  /** Machine-readable note: a stable error code, or `cached`. Never user-facing text. */
  detail: string | null;
  downloaded: number;
  total: number | null;
  elapsedMs: number;
}

/** A step as shown in the UI. */
export interface SmapiJob {
  step: SmapiStep;
  state: SmapiStepState;
  detail: string | null;
  elapsedMs: number;
  downloaded: number;
  total: number | null;
}

const record = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null;
const optionalString = (value: unknown) => value === null || typeof value === 'string';
const knownSteps: string[] = ['release', 'download', 'verify', 'extract', 'install', 'uninstall'];
const knownStates: string[] = ['running', 'done', 'failed'];

function invalid(): never { throw { code: 'invalidResponse' }; }

export function decodeStatus(value: unknown): SmapiStatus {
  if (!record(value) || typeof value.installed !== 'boolean' || !optionalString(value.version) || !optionalString(value.executable)) return invalid();
  return value as unknown as SmapiStatus;
}

export function decodeRelease(value: unknown): SmapiRelease {
  if (!record(value) || typeof value.version !== 'string' || typeof value.assetName !== 'string'
      || typeof value.assetUrl !== 'string' || !optionalString(value.sha256) || typeof value.size !== 'number') return invalid();
  return value as unknown as SmapiRelease;
}

export function decodeOutcome(value: unknown): SmapiInstallOutcome {
  if (!record(value) || typeof value.version !== 'string' || typeof value.executable !== 'string'
      || typeof value.launcherReplaced !== 'boolean') return invalid();
  return value as unknown as SmapiInstallOutcome;
}

export function decodeUninstallReport(value: unknown): SmapiUninstallReport {
  if (!record(value) || typeof value.removed !== 'number' || typeof value.modsRemoved !== 'number'
      || typeof value.launcherRestored !== 'boolean') return invalid();
  return value as unknown as SmapiUninstallReport;
}

export function decodeLog(value: unknown): SmapiLog {
  if (!record(value) || typeof value.path !== 'string' || typeof value.exists !== 'boolean'
      || !(value.modifiedMs === null || typeof value.modifiedMs === 'number')
      || typeof value.size !== 'number' || typeof value.content !== 'string'
      || typeof value.truncated !== 'boolean') return invalid();
  return value as unknown as SmapiLog;
}

export function decodeProgress(value: unknown): SmapiProgress {
  if (!record(value) || typeof value.step !== 'string' || !knownSteps.includes(value.step)
      || typeof value.state !== 'string' || !knownStates.includes(value.state)
      || !optionalString(value.detail) || typeof value.downloaded !== 'number'
      || !(value.total === null || typeof value.total === 'number') || typeof value.elapsedMs !== 'number') return invalid();
  return value as unknown as SmapiProgress;
}

/** Fold one progress event into the step list, keeping the order steps first appeared in. */
export function mergeProgress(jobs: SmapiJob[], progress: SmapiProgress): SmapiJob[] {
  const index = jobs.findIndex(job => job.step === progress.step);
  if (index === -1) return [...jobs, { ...progress }];
  return jobs.map((job, position) => position === index ? { ...job, ...progress } : job);
}
