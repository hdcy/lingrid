import { call } from '../../shared/ipc.ts';
import { decodeLog, decodeOutcome, decodeRelease, decodeStatus, decodeUninstallReport } from './types.ts';

export const smapiApi = {
  status: (path: string) => call('smapi_status', { path }, decodeStatus),
  release: () => call('fetch_smapi_release', undefined, decodeRelease),
  log: () => call('smapi_log', undefined, decodeLog),
  install: (path: string) => call('install_smapi', { path }, decodeOutcome),
  uninstall: (path: string, removeMods: boolean) => call('uninstall_smapi', { path, removeMods }, decodeUninstallReport),
};
