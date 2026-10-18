/** One result from the Nexus Mods search. */
export interface NexusModSummary {
  modId: number;
  name: string;
  summary: string | null;
  version: string | null;
  downloads: number | null;
  author: string | null;
  /** Link to the mod page on nexusmods.com. */
  url: string;
}

const record = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null;
const optionalString = (value: unknown) => value === null || typeof value === 'string';

function invalid(): never { throw { code: 'invalidResponse' }; }

export function decodeNexusModSummary(value: unknown): NexusModSummary {
  if (!record(value) || typeof value.modId !== 'number' || typeof value.name !== 'string'
      || !optionalString(value.summary) || !optionalString(value.version)
      || !(value.downloads === null || typeof value.downloads === 'number')
      || !optionalString(value.author) || typeof value.url !== 'string') return invalid();
  return value as unknown as NexusModSummary;
}

export function decodeNexusMods(value: unknown): NexusModSummary[] {
  if (!Array.isArray(value)) return invalid();
  return value.map(decodeNexusModSummary);
}
