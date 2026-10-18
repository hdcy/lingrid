export type GameSource = 'steam' | 'gog' | 'common' | 'manual';
export type GameRuntime = 'windows' | 'linux';

export interface GameInstallation {
  path: string;
  executable: string;
  runtime: GameRuntime;
  sources: GameSource[];
  smapiPath: string | null;
}

export interface DiscoveryWarning { code: string; path: string; detail: string }
export interface DiscoveryReport { installations: GameInstallation[]; warnings: DiscoveryWarning[] }

const record = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null;
const sources: string[] = ['steam', 'gog', 'common', 'manual'];

function invalid(): never { throw { code: 'invalidResponse' }; }

// Keep the Rust/TypeScript boundary explicit: incompatible payloads fail visibly.
export function decodeInstallation(value: unknown): GameInstallation {
  if (!record(value) || typeof value.path !== 'string' || typeof value.executable !== 'string'
      || (value.runtime !== 'windows' && value.runtime !== 'linux') || !Array.isArray(value.sources)
      || !value.sources.every((source: unknown) => typeof source === 'string' && sources.includes(source))
      || !(value.smapiPath === null || typeof value.smapiPath === 'string')) return invalid();
  return value as unknown as GameInstallation;
}

export function decodeReport(value: unknown): DiscoveryReport {
  if (!record(value) || !Array.isArray(value.installations) || !Array.isArray(value.warnings)) return invalid();
  return {
    installations: value.installations.map(decodeInstallation),
    warnings: value.warnings.map((warning: unknown) => {
      if (!record(warning) || typeof warning.code !== 'string' || typeof warning.path !== 'string' || typeof warning.detail !== 'string') return invalid();
      return warning as unknown as DiscoveryWarning;
    }),
  };
}
