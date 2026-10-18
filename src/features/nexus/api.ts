import { call } from '../../shared/ipc.ts';
import { decodeReport } from '../mods/types.ts';
import { decodeNexusMods } from './types.ts';

export const nexusApi = {
  /** Anonymous search; an empty query browses the newest mods. */
  search: (query: string) => call('search_nexus_mods', { query }, decodeNexusMods),
  /** Whether a key is stored; the key itself never leaves the backend. */
  keyStatus: () => call('nexus_api_key_status', undefined, value => value === true),
  setKey: (key: string) => call('set_nexus_api_key', { key }, () => undefined),
  installFromNxm: (path: string, url: string) => call('install_mod_from_nxm', { path, url }, decodeReport),
};
