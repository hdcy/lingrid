import { invoke, isTauri } from '@tauri-apps/api/core';

export interface AppError { code: string; detail?: string | null }

export function normalizeError(error: unknown): AppError {
  if (typeof error === 'object' && error !== null && 'code' in error && typeof error.code === 'string') {
    return { code: error.code, detail: 'detail' in error && typeof error.detail === 'string' ? error.detail : null };
  }
  return { code: 'unexpected', detail: error instanceof Error ? error.message : String(error) };
}

export async function call<T>(command: string, args: Record<string, unknown> | undefined, decode: (value: unknown) => T): Promise<T> {
  if (!isTauri()) throw { code: 'desktopRequired' } satisfies AppError;
  try {
    return decode(await invoke<unknown>(command, args));
  } catch (error) {
    throw normalizeError(error);
  }
}
