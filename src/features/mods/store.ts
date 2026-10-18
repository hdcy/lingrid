import { computed, ref } from 'vue';
import { defineStore } from 'pinia';
import { normalizeError, type AppError } from '../../shared/ipc.ts';
import { modsApi } from './api.ts';
import type { InstalledMod } from './types.ts';

export const useModsStore = defineStore('mods', () => {
  const mods = ref<InstalledMod[]>([]);
  const busy = ref(false);
  const error = ref<AppError | null>(null);
  /** i18n key of the last successful action. */
  const notice = ref<string | null>(null);
  /** Selection already loaded, so several pages can share one lookup. */
  const loadedFor = ref<string | null>(null);
  /** Set while the user confirms deleting a mod. */
  const confirmingRemove = ref<string | null>(null);

  const enabled = computed(() => mods.value.filter(mod => mod.enabled));
  const disabled = computed(() => mods.value.filter(mod => !mod.enabled));

  function reset() {
    mods.value = [];
    error.value = null;
    notice.value = null;
    confirmingRemove.value = null;
  }

  async function load(path: string) {
    mods.value = await modsApi.list(path);
  }

  /** Reload the list; an unchanged selection is not fetched twice. */
  async function refresh(path: string | null, force = false) {
    if (!force && path === loadedFor.value) return;
    loadedFor.value = path;
    reset();
    if (!path) return;
    busy.value = true;
    try { await load(path); }
    catch (cause) { error.value = normalizeError(cause); }
    finally { busy.value = false; }
  }

  async function setEnabled(path: string, folder: string, isEnabled: boolean) {
    if (busy.value) return;
    busy.value = true;
    error.value = null;
    notice.value = null;
    try {
      await modsApi.setEnabled(path, folder, isEnabled);
      notice.value = isEnabled ? 'mods.enabledNotice' : 'mods.disabledNotice';
      await load(path);
    } catch (cause) { error.value = normalizeError(cause); }
    finally { busy.value = false; }
  }

  function askRemove(folder: string) {
    confirmingRemove.value = folder;
  }

  function cancelRemove() {
    confirmingRemove.value = null;
  }

  async function remove(path: string, folder: string) {
    if (busy.value) return;
    busy.value = true;
    error.value = null;
    notice.value = null;
    try {
      await modsApi.remove(path, folder);
      confirmingRemove.value = null;
      notice.value = 'mods.removedNotice';
      await load(path);
    } catch (cause) { error.value = normalizeError(cause); }
    finally { busy.value = false; }
  }

  /** Ask for an archive and install everything inside it. */
  async function install(path: string) {
    if (busy.value) return;
    busy.value = true;
    error.value = null;
    notice.value = null;
    try {
      const archive = await modsApi.pickArchive();
      if (!archive) return;
      await modsApi.install(path, archive);
      notice.value = 'mods.installedNotice';
      await load(path);
    } catch (cause) { error.value = normalizeError(cause); }
    finally { busy.value = false; }
  }

  return {
    mods, busy, error, notice, confirmingRemove, enabled, disabled,
    refresh, setEnabled, askRemove, cancelRemove, remove, install,
  };
});
