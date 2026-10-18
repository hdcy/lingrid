<script setup lang="ts">
import { computed, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { useGameStore } from '../game/store';
import { useSmapiStore } from './store';
import type { SmapiJob } from './types';

const { t, te } = useI18n();
const game = useGameStore();
const smapi = useSmapiStore();
const errorText = (code: string) => t(te(`errors.${code}`) ? `errors.${code}` : 'errors.unexpected');

// Local state follows the selected directory; the network is only touched on request.
watch(() => game.selectedPath, path => { void smapi.refresh(path); }, { immediate: true });

const statusText = computed(() => {
  if (!smapi.installed) return t('smapi.notInstalled');
  const version = smapi.status?.version;
  return version ? t('smapi.installed', { version }) : t('smapi.installedUnknown');
});

const installLabel = computed(() => {
  if (smapi.busy) return t('smapi.installing');
  if (smapi.outdated && smapi.release) return t('smapi.update', { version: smapi.release.version });
  return t('smapi.install');
});

// Windows keeps the game launcher, so starting through Steam needs a manual step.
const launcherNotice = computed(() =>
  game.selected?.runtime === 'windows' ? 'smapi.launcherKept' : 'smapi.launcherReplaced');

function mib(value: number): string {
  return `${(value / (1024 * 1024)).toFixed(1)} MiB`;
}

function kib(value: number): string {
  return value < 1024 ? `${value} B` : `${(value / 1024).toFixed(1)} KiB`;
}

/** What to show next to a step: live byte counts, a cache note, or how long it took. */
function stepDetail(job: SmapiJob): string {
  if (job.state === 'running') {
    return job.step === 'download' && job.total
      ? t('smapi.steps.downloading', { done: mib(job.downloaded), total: mib(job.total) })
      : t('smapi.steps.running');
  }
  if (job.state === 'failed') return t('smapi.steps.failed');
  if (job.detail === 'cached') return t('smapi.steps.cached');
  return job.elapsedMs >= 100
    ? t('smapi.steps.elapsed', { seconds: (job.elapsedMs / 1000).toFixed(1) })
    : t('smapi.steps.done');
}

const logMeta = computed(() => {
  const log = smapi.log;
  if (!log?.exists) return '';
  const time = log.modifiedMs ? new Date(log.modifiedMs).toLocaleString() : '';
  return t('smapi.log.meta', { time, size: kib(log.size) });
});

function run(action: 'install' | 'confirm' | 'check' | 'ask') {
  if (action === 'check') { void smapi.checkRelease(); return; }
  if (action === 'ask') { smapi.askUninstall(); return; }
  const path = game.selectedPath;
  if (!path) return;
  void (action === 'install' ? smapi.install(path) : smapi.uninstall(path));
}
</script>

<template>
  <section class="panel smapi-panel" :aria-busy="smapi.busy">
    <div class="smapi-heading">
      <h2 class="section-title">{{ t('smapi.title') }}</h2>
      <span class="badge" :class="{ current: smapi.installed }">{{ statusText }}</span>
      <span v-if="smapi.release" class="muted small">{{ t('smapi.latest', { version: smapi.release.version }) }}</span>
    </div>
    <p class="muted small">{{ t('smapi.statusHint') }}</p>
    <div class="actions">
      <button class="primary" :disabled="smapi.busy || !game.selectedPath" @click="run('install')">{{ installLabel }}</button>
      <button :disabled="smapi.busy || smapi.checking || !game.selectedPath" @click="run('check')">
        {{ t(smapi.checking ? 'smapi.checking' : 'smapi.check') }}
      </button>
      <button v-if="smapi.installed && !smapi.confirmingUninstall" :disabled="smapi.busy" @click="run('ask')">
        {{ t('smapi.uninstall') }}
      </button>
    </div>
    <p v-if="!game.selectedPath" class="muted small">{{ t('smapi.noGame') }}</p>

    <div v-if="smapi.confirmingUninstall" class="confirm">
      <p><strong>{{ t('smapi.uninstallConfirm.title') }}</strong></p>
      <p class="muted small">{{ t('smapi.uninstallConfirm.hint') }}</p>
      <label class="checkbox">
        <input v-model="smapi.removeMods" type="checkbox" :disabled="smapi.busy" />
        <span>{{ t('smapi.uninstallConfirm.removeMods') }}</span>
      </label>
      <p v-if="smapi.removeMods" class="warning small">{{ t('smapi.uninstallConfirm.removeModsWarning') }}</p>
      <div class="actions">
        <button class="danger" :disabled="smapi.busy" @click="run('confirm')">
          {{ t(smapi.busy ? 'smapi.uninstalling' : 'smapi.uninstallConfirm.confirm') }}
        </button>
        <button :disabled="smapi.busy" @click="smapi.cancelUninstall()">{{ t('smapi.uninstallConfirm.cancel') }}</button>
      </div>
    </div>

    <div v-if="smapi.steps.length" class="job" aria-live="polite">
      <h3 class="job-title">{{ t('smapi.steps.title') }}</h3>
      <ol class="job-steps">
        <li v-for="job in smapi.steps" :key="job.step" :class="job.state">
          <span class="step-dot" aria-hidden="true"></span>
          <span class="step-name">{{ t(`smapi.steps.${job.step}`) }}</span>
          <span class="step-detail muted small">{{ stepDetail(job) }}</span>
        </li>
      </ol>
      <div
        v-if="smapi.downloadedPercent !== null && smapi.downloadStep?.state === 'running'"
        class="progress"
        role="progressbar"
        :aria-label="t('smapi.steps.download')"
        :aria-valuenow="smapi.downloadedPercent"
        aria-valuemin="0"
        aria-valuemax="100"
      >
        <span :style="{ width: `${smapi.downloadedPercent}%` }"></span>
      </div>
    </div>

    <div v-if="smapi.notice === 'smapi.installedNotice'" class="notice success" role="status">
      <strong>{{ t('smapi.installedNotice', { version: smapi.outcome?.version }) }}</strong>
      <p class="small">{{ t(launcherNotice) }}</p>
    </div>
    <div v-else-if="smapi.notice === 'smapi.uninstalledNotice'" class="notice success" role="status">
      <strong>{{ t('smapi.uninstalledNotice') }}</strong>
      <p v-if="smapi.uninstallReport" class="small">
        {{ t('smapi.uninstallSummary', { removed: smapi.uninstallReport.removed, mods: smapi.uninstallReport.modsRemoved }) }}
      </p>
    </div>
    <div v-else-if="smapi.notice" class="notice success" role="status">{{ t(smapi.notice) }}</div>
    <div v-if="smapi.error" class="notice error" role="alert">
      {{ errorText(smapi.error.code) }}
      <details v-if="smapi.error.detail"><summary>{{ t('diagnostics') }}</summary><pre>{{ smapi.error.detail }}</pre></details>
    </div>

    <div v-if="game.selectedPath" class="log-panel">
      <div class="log-head">
        <h3 class="job-title">{{ t('smapi.log.title') }}</h3>
        <button :disabled="smapi.logBusy" @click="smapi.loadLog()">
          {{ t(smapi.logBusy ? 'smapi.log.loading' : 'smapi.log.load') }}
        </button>
      </div>
      <p v-if="smapi.log" class="muted small log-path" dir="ltr">{{ smapi.log.path }}</p>
      <p v-if="!smapi.log" class="muted small">{{ t('smapi.log.hint') }}</p>
      <p v-else-if="!smapi.log.exists" class="muted small">{{ t('smapi.log.missing') }}</p>
      <template v-else>
        <p class="muted small">{{ logMeta }}</p>
        <p v-if="smapi.log.truncated" class="muted small">{{ t('smapi.log.truncated') }}</p>
        <pre class="log-body" dir="ltr" tabindex="0">{{ smapi.log.content }}</pre>
      </template>
    </div>
  </section>
</template>

<style scoped>
.smapi-heading { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; margin-bottom: 14px; }
.smapi-heading .section-title { margin: 0; }
.badge.current { background: var(--accent-soft); color: var(--accent-text); }
.job, .log-panel { margin-top: 18px; padding-top: 14px; border-top: 1px solid var(--border); }
.job-title { font-size: 11px; font-weight: 700; letter-spacing: .7px; color: var(--text-muted); margin: 0 0 11px; }
.job-steps { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
.job-steps li { display: flex; align-items: baseline; gap: 10px; font-size: 13px; }
.step-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--border-strong); flex-shrink: 0; align-self: center; }
.job-steps li.running .step-dot { background: var(--notice-border); }
.job-steps li.done .step-dot { background: var(--accent); }
.job-steps li.failed .step-dot { background: var(--danger-text); }
.step-name { flex: 1; min-width: 0; }
.step-detail { flex-shrink: 0; }
.progress { margin-top: 12px; height: 6px; border-radius: 3px; background: var(--bg-subtle); overflow: hidden; }
.progress span { display: block; height: 100%; background: var(--accent); transition: width .25s ease; }
.confirm { margin-top: 16px; padding: 16px 18px; border: 1px solid var(--danger-border); background: var(--danger-bg); border-radius: 8px; }
.confirm p { margin-block: 0 8px; }
.checkbox { display: flex; align-items: center; gap: 9px; font-size: 13px; margin-block: 10px; }
.checkbox input { width: 16px; height: 16px; accent-color: var(--accent); }
.warning { color: var(--danger-text); }
button.danger { background: var(--danger-text); border-color: var(--danger-text); color: var(--bg-panel); }
button.danger:hover:not(:disabled) { background: var(--danger-text); border-color: var(--danger-text); opacity: .88; }
.log-head { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 10px; }
.log-path { font-family: ui-monospace, monospace; overflow-wrap: anywhere; }
.log-body { max-height: 320px; overflow: auto; background: var(--bg-subtle); border: 1px solid var(--border); border-radius: 6px; padding: 12px; margin: 0; }
</style>
