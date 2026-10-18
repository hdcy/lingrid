import { computed, ref } from 'vue';
import { defineStore } from 'pinia';
import { normalizeError, type AppError } from '../../shared/ipc.ts';
import { readPreference, writePreference } from '../../shared/preferences.ts';
import { gameApi } from './api.ts';
import type { DiscoveryWarning, GameInstallation } from './types.ts';

export const useGameStore = defineStore('game', () => {
  const installations = ref<GameInstallation[]>([]);
  const warnings = ref<DiscoveryWarning[]>([]);
  const selectedPath = ref<string | null>(null);
  const selected = computed(() => installations.value.find(game => game.path === selectedPath.value));
  const busy = ref(false);
  const scanned = ref(false);
  const error = ref<AppError | null>(null);
  const selectionNotice = ref(false);

  function select(game: GameInstallation) {
    selectedPath.value = game.path;
    writePreference('gamePath.v1', game.path);
  }

  function merge(game: GameInstallation) {
    if (!installations.value.some(item => item.path === game.path)) installations.value.push(game);
    select(game);
  }

  async function scan() {
    if (busy.value) return;
    busy.value = true;
    error.value = null;
    selectionNotice.value = false;
    try {
      const report = await gameApi.discover();
      const previous = selectedPath.value ?? readPreference('gamePath.v1');
      installations.value = report.installations;
      warnings.value = report.warnings;
      selectedPath.value = null;
      // A saved/manual location may not be discoverable; always revalidate it on refresh/startup.
      if (previous) {
        const found = installations.value.find(game => game.path === previous);
        if (found) select(found);
        else {
          try { merge(await gameApi.validate(previous)); }
          catch { selectionNotice.value = true; }
        }
      }
      // Do not silently switch to a different game when a previously selected copy disappears.
      if (!previous && installations.value.length === 1) select(installations.value[0]!);
      scanned.value = true;
    } catch (cause) { error.value = normalizeError(cause); }
    finally { busy.value = false; }
  }

  async function addDirectory(path?: string) {
    if (busy.value) return;
    busy.value = true;
    error.value = null;
    try {
      const game = path === undefined ? await gameApi.pick() : await gameApi.validate(path.trim());
      if (game) { merge(game); selectionNotice.value = false; }
    } catch (cause) { error.value = normalizeError(cause); }
    finally { busy.value = false; }
  }

  return { installations, warnings, selectedPath, selected, busy, scanned, error, selectionNotice, select, scan, addDirectory };
});
