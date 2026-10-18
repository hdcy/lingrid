import { computed, ref, watch } from 'vue';
import { readPreference, writePreference } from './preferences.ts';

export type ThemeMode = 'light' | 'dark' | 'system';
export type ThemeAccent = 'forest' | 'ocean' | 'violet' | 'amber';

export const themeModes: readonly ThemeMode[] = ['light', 'dark', 'system'];
export const themeAccents: readonly ThemeAccent[] = ['forest', 'ocean', 'violet', 'amber'];

const MODE_KEY = 'themeMode.v1';
const ACCENT_KEY = 'themeAccent.v1';

const isMode = (value: string | null): value is ThemeMode => themeModes.includes(value as ThemeMode);
const isAccent = (value: string | null): value is ThemeAccent => themeAccents.includes(value as ThemeAccent);

// `matchMedia` is absent outside a browser (tests), and in older WebViews.
const darkQuery = typeof window === 'undefined' ? undefined : window.matchMedia?.('(prefers-color-scheme: dark)');

/** Stored values come from localStorage and may be stale or hand-edited. */
export function normalizeMode(value: string | null): ThemeMode {
  return isMode(value) ? value : 'system';
}

export function normalizeAccent(value: string | null): ThemeAccent {
  return isAccent(value) ? value : 'forest';
}

function readMode(): ThemeMode {
  return normalizeMode(readPreference(MODE_KEY));
}

function readAccent(): ThemeAccent {
  return normalizeAccent(readPreference(ACCENT_KEY));
}

export const themeMode = ref<ThemeMode>(readMode());
export const themeAccent = ref<ThemeAccent>(readAccent());

/** `system` resolves through the OS preference, so CSS only ever sees light or dark. */
export const resolvedMode = computed<'light' | 'dark'>(() =>
  themeMode.value === 'system' ? (darkQuery?.matches ? 'dark' : 'light') : themeMode.value);

/** Publish the current theme on <html>; every colour in styles.css hangs off these. */
export function applyTheme() {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  root.dataset.theme = resolvedMode.value;
  root.dataset.accent = themeAccent.value;
}

export function setThemeMode(mode: ThemeMode) {
  themeMode.value = mode;
}

export function setThemeAccent(accent: ThemeAccent) {
  themeAccent.value = accent;
}

watch([resolvedMode, themeAccent], applyTheme, { immediate: true });
watch(themeMode, mode => writePreference(MODE_KEY, mode));
watch(themeAccent, accent => writePreference(ACCENT_KEY, accent));
darkQuery?.addEventListener?.('change', applyTheme);
