import { call } from '../../shared/ipc.ts';
import { decodeMods, decodeReport } from './types.ts';

export const modsApi = {
  list: (path: string) => call('list_mods', { path }, decodeMods),
  setEnabled: (path: string, folder: string, enabled: boolean) =>
    call('set_mod_enabled', { path, folder, enabled }, () => undefined),
  remove: (path: string, folder: string) => call('remove_mod', { path, folder }, () => undefined),
  /** Opens the archive picker; resolves to null when the user cancels. */
  pickArchive: () => call('pick_mod_archive', undefined, value => value === null ? null : String(value)),
  install: (path: string, archive: string) => call('install_mod', { path, archive }, decodeReport),
};
