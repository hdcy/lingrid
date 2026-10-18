import { ref } from 'vue';

// UI preferences only. Game files and future domain settings belong to Rust services.
export const preferenceError = ref(false);

export function readPreference(key: string): string | null {
  try { return localStorage.getItem(`lingrid.${key}`); }
  catch { preferenceError.value = true; return null; }
}

export function writePreference(key: string, value: string): void {
  try { localStorage.setItem(`lingrid.${key}`, value); }
  catch { preferenceError.value = true; }
}
