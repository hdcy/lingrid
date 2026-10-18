import { call } from '../../shared/ipc.ts';
import { decodeInstallation, decodeReport } from './types.ts';

export const gameApi = {
  discover: () => call('discover_game_installations', undefined, decodeReport),
  validate: (path: string) => call('validate_game_directory', { path }, decodeInstallation),
  pick: () => call('pick_game_directory', undefined, value => value === null ? null : decodeInstallation(value)),
};
