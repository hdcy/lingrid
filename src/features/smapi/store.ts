import { computed, ref } from 'vue';
import { defineStore } from 'pinia';
import { isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { normalizeError, type AppError } from '../../shared/ipc.ts';
import { smapiApi } from './api.ts';
import {
  decodeProgress,
  mergeProgress,
  type SmapiInstallOutcome,
  type SmapiJob,
  type SmapiLog,
  type SmapiRelease,
  type SmapiStatus,
  type SmapiUninstallReport,
} from './types.ts';

/** Event the backend emits while it works, so long operations stay observable. */
const PROGRESS_EVENT = 'smapi://progress';

export const useSmapiStore = defineStore('smapi', () => {
  const status = ref<SmapiStatus | null>(null);
  const release = ref<SmapiRelease | null>(null);
  const outcome = ref<SmapiInstallOutcome | null>(null);
  const uninstallReport = ref<SmapiUninstallReport | null>(null);
  /** Steps reported by the running (or last) operation. */
  const steps = ref<SmapiJob[]>([]);
  /** The game's own SMAPI log, loaded on demand. */
  const log = ref<SmapiLog | null>(null);
  const logBusy = ref(false);
  const busy = ref(false);
  const checking = ref(false);
  const error = ref<AppError | null>(null);
  /** i18n key of the last successful action. */
  const notice = ref<string | null>(null);
  /** Selection already loaded, so multiple pages can share one lookup. */
  const loadedFor = ref<string | null>(null);
  /** Uninstall asks first: clearing `Mods` cannot be undone. */
  const confirmingUninstall = ref(false);
  const removeMods = ref(false);

  const installed = computed(() => status.value?.installed === true);
  // An unknown version (manual install) is not treated as outdated.
  const outdated = computed(() => installed.value && !!status.value?.version && !!release.value && status.value.version !== release.value.version);

  const downloadStep = computed(() => steps.value.find(job => job.step === 'download') ?? null);
  const downloadedPercent = computed(() => {
    const job = downloadStep.value;
    if (!job?.total) return null;
    return Math.min(100, Math.round((job.downloaded / job.total) * 100));
  });

  let listening: Promise<void> | null = null;
  /** Subscribe once, before work starts, so no early event is missed. */
  function ensureListening(): Promise<void> {
    if (!isTauri()) return Promise.resolve();
    listening ??= listen<unknown>(PROGRESS_EVENT, event => {
      try { steps.value = mergeProgress(steps.value, decodeProgress(event.payload)); }
      catch { /* Ignore payloads this version does not understand. */ }
    }).then(() => undefined).catch(() => { listening = null; });
    return listening;
  }

  function reset() {
    status.value = null;
    release.value = null;
    outcome.value = null;
    uninstallReport.value = null;
    steps.value = [];
    log.value = null;
    error.value = null;
    notice.value = null;
    confirmingUninstall.value = false;
    removeMods.value = false;
  }

  async function loadStatus(path: string) {
    status.value = await smapiApi.status(path);
  }

  /** Reload local state for the selected game; never touches the network. */
  async function refresh(path: string | null) {
    // Several pages watch the same selection, so an unchanged path is not refetched.
    if (path === loadedFor.value) return;
    loadedFor.value = path;
    reset();
    if (!path) return;
    busy.value = true;
    try { await loadStatus(path); }
    catch (cause) { error.value = normalizeError(cause); }
    finally { busy.value = false; }
  }

  async function checkRelease() {
    if (checking.value) return;
    checking.value = true;
    error.value = null;
    try { release.value = await smapiApi.release(); }
    catch (cause) { error.value = normalizeError(cause); }
    finally { checking.value = false; }
  }

  /** Load the log SMAPI wrote during the last game session. */
  async function loadLog() {
    if (logBusy.value) return;
    logBusy.value = true;
    error.value = null;
    try { log.value = await smapiApi.log(); }
    catch (cause) { error.value = normalizeError(cause); }
    finally { logBusy.value = false; }
  }

  function askUninstall() {
    confirmingUninstall.value = true;
  }

  function cancelUninstall() {
    confirmingUninstall.value = false;
    removeMods.value = false;
  }

  async function install(path: string) {
    if (busy.value) return;
    busy.value = true;
    error.value = null;
    notice.value = null;
    steps.value = [];
    await ensureListening();
    try {
      outcome.value = await smapiApi.install(path);
      uninstallReport.value = null;
      notice.value = 'smapi.installedNotice';
      await loadStatus(path);
    } catch (cause) { error.value = normalizeError(cause); }
    finally { busy.value = false; }
  }

  async function uninstall(path: string) {
    if (busy.value) return;
    busy.value = true;
    error.value = null;
    notice.value = null;
    steps.value = [];
    await ensureListening();
    try {
      uninstallReport.value = await smapiApi.uninstall(path, removeMods.value);
      outcome.value = null;
      log.value = null;
      confirmingUninstall.value = false;
      removeMods.value = false;
      notice.value = 'smapi.uninstalledNotice';
      await loadStatus(path);
    } catch (cause) { error.value = normalizeError(cause); }
    finally { busy.value = false; }
  }

  return {
    status, release, outcome, uninstallReport, steps, log, logBusy, busy, checking, error, notice,
    installed, outdated, downloadStep, downloadedPercent, confirmingUninstall, removeMods,
    refresh, checkRelease, loadLog, askUninstall, cancelUninstall, install, uninstall,
  };
});
