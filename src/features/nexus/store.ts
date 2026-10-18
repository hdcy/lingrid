import { ref } from 'vue';
import { defineStore } from 'pinia';
import { isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { normalizeError, type AppError } from '../../shared/ipc.ts';
import { nexusApi } from './api.ts';
import type { NexusModSummary } from './types.ts';

/** Event the backend emits when the system hands it an `nxm://` link. */
const NXM_EVENT = 'nxm://received';

export const useNexusStore = defineStore('nexus', () => {
  const results = ref<NexusModSummary[]>([]);
  const busy = ref(false);
  const error = ref<AppError | null>(null);
  /** Whether a search has been run, so the empty state is not shown too early. */
  const searched = ref(false);
  /** Whether a Nexus API key is stored; downloading needs one. */
  const hasKey = ref(false);
  /** i18n key of the last successful action. */
  const notice = ref<string | null>(null);
  /** Mods created by the last nxm install. */
  const installed = ref<string[]>([]);
  /** Link that arrived from the system and has not been handled yet. */
  const incoming = ref<string | null>(null);

  async function search(query: string) {
    if (busy.value) return;
    busy.value = true;
    error.value = null;
    try {
      results.value = await nexusApi.search(query.trim());
      searched.value = true;
    } catch (cause) { error.value = normalizeError(cause); }
    finally { busy.value = false; }
  }

  let listening: Promise<void> | null = null;
  /** Subscribe once, so links clicked in the browser reach the app. */
  function ensureListening(): Promise<void> {
    if (!isTauri()) return Promise.resolve();
    listening ??= listen<string>(NXM_EVENT, event => {
      if (typeof event.payload === 'string') incoming.value = event.payload;
    }).then(() => undefined).catch(() => { listening = null; });
    return listening;
  }

  /**
   * Install whatever link arrived, into the selected game.
   *
   * A link can show up before a game directory is chosen — the browser may start the app
   * for it — in which case the error tells the user what to do instead of dropping it.
   */
  async function acceptIncoming(gamePath: string | null) {
    const link = incoming.value;
    incoming.value = null;
    if (!link) return;
    if (!gamePath) {
      error.value = { code: 'nexusNeedsGame' };
      return;
    }
    await install(gamePath, link);
  }

  /** Check for a stored key; called when the Nexus pages open. */
  async function loadKeyStatus() {
    try { hasKey.value = await nexusApi.keyStatus(); }
    catch (cause) { error.value = normalizeError(cause); }
  }

  /** Save the Nexus API key; an empty value clears it. */
  async function saveKey(key: string) {
    if (busy.value) return;
    busy.value = true;
    error.value = null;
    notice.value = null;
    try {
      await nexusApi.setKey(key);
      await loadKeyStatus();
      notice.value = hasKey.value ? 'nexus.keySaved' : 'nexus.keyCleared';
    } catch (cause) { error.value = normalizeError(cause); }
    finally { busy.value = false; }
  }

  /** Install a mod from an `nxm://` link the Nexus website handed out. */
  async function install(path: string, url: string) {
    if (busy.value) return;
    busy.value = true;
    error.value = null;
    notice.value = null;
    try {
      const report = await nexusApi.installFromNxm(path, url.trim());
      installed.value = report.folders;
      notice.value = 'nexus.installedNotice';
    } catch (cause) { error.value = normalizeError(cause); }
    finally { busy.value = false; }
  }

  return {
    results, busy, error, searched, hasKey, notice, installed, incoming,
    search, ensureListening, acceptIncoming, loadKeyStatus, saveKey, install,
  };
});
