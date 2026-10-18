/** One folder inside `Mods/`, as the backend reports it. */
export interface InstalledMod {
  /** Folder name without the leading dot used for disabling. */
  folder: string;
  path: string;
  /** Whether SMAPI will load it. */
  enabled: boolean;
  name: string;
  author: string;
  version: string;
  uniqueId: string | null;
  description: string | null;
  /** Nexus mod id parsed from the manifest's update keys. */
  nexusId: number | null;
  /** True for content packs and anything else without a readable manifest. */
  manifestMissing: boolean;
}

/** One archive can hold several mods. */
export interface InstalledModsReport {
  folders: string[];
}

const record = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null;
const optionalString = (value: unknown) => value === null || typeof value === 'string';

function invalid(): never { throw { code: 'invalidResponse' }; }

export function decodeMod(value: unknown): InstalledMod {
  if (!record(value) || typeof value.folder !== 'string' || typeof value.path !== 'string'
      || typeof value.enabled !== 'boolean' || typeof value.name !== 'string'
      || typeof value.author !== 'string' || typeof value.version !== 'string'
      || !optionalString(value.uniqueId) || !optionalString(value.description)
      || !(value.nexusId === null || typeof value.nexusId === 'number')
      || typeof value.manifestMissing !== 'boolean') return invalid();
  return value as unknown as InstalledMod;
}

export function decodeMods(value: unknown): InstalledMod[] {
  if (!Array.isArray(value)) return invalid();
  return value.map(decodeMod);
}

export function decodeReport(value: unknown): InstalledModsReport {
  if (!record(value) || !Array.isArray(value.folders)
      || !value.folders.every((folder: unknown) => typeof folder === 'string')) return invalid();
  return value as unknown as InstalledModsReport;
}
