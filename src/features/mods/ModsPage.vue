<script setup lang="ts">
import { watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { openUrl } from '@tauri-apps/plugin-opener';
import { useGameStore } from '../game/store';
import { useModsStore } from './store';
import type { InstalledMod } from './types';

const { t, te } = useI18n();
const game = useGameStore();
const mods = useModsStore();
const errorText = (code: string) => t(te(`errors.${code}`) ? `errors.${code}` : 'errors.unexpected');

// The list follows the selected game; nothing is scanned until one is chosen.
watch(() => game.selectedPath, path => { void mods.refresh(path); }, { immediate: true });

function refresh() {
  void mods.refresh(game.selectedPath, true);
}

function install() {
  const path = game.selectedPath;
  if (path) void mods.install(path);
}

function toggle(mod: InstalledMod) {
  const path = game.selectedPath;
  if (path) void mods.setEnabled(path, mod.folder, !mod.enabled);
}

function confirmRemove(mod: InstalledMod) {
  const path = game.selectedPath;
  if (path) void mods.remove(path, mod.folder);
}

function openNexus(mod: InstalledMod) {
  if (mod.nexusId) {
    void openUrl(`https://www.nexusmods.com/stardewvalley/mods/${mod.nexusId}`).catch(() => undefined);
  }
}
</script>

<template>
  <section class="page">
    <header class="page-heading">
      <h1>{{ t('mods.title') }}</h1>
      <p class="muted">{{ t('mods.description') }}</p>
    </header>

    <section class="panel" :aria-busy="mods.busy">
      <div class="actions">
        <button class="primary" :disabled="mods.busy || !game.selectedPath" @click="install">{{ t('mods.install') }}</button>
        <button :disabled="mods.busy || !game.selectedPath" @click="refresh">{{ t('mods.refresh') }}</button>
      </div>
      <p v-if="!game.selectedPath" class="muted small">{{ t('mods.noGame') }}</p>
      <p v-else class="muted small">
        {{ t('mods.summary', { enabled: mods.enabled.length, disabled: mods.disabled.length }) }}
      </p>
    </section>

    <div v-if="mods.error" class="notice error" role="alert">
      {{ errorText(mods.error.code) }}
      <details v-if="mods.error.detail"><summary>{{ t('diagnostics') }}</summary><pre>{{ mods.error.detail }}</pre></details>
    </div>
    <div v-if="mods.notice" class="notice success" role="status">{{ t(mods.notice) }}</div>

    <section v-if="mods.mods.length" aria-live="polite">
      <article v-for="mod in mods.mods" :key="mod.folder" class="panel mod-card" :class="{ 'mod-disabled': !mod.enabled }">
        <div class="mod-head">
          <h2 class="mod-name">{{ mod.name }}</h2>
          <span v-if="mod.version" class="badge">{{ mod.version }}</span>
          <span v-if="!mod.enabled" class="badge badge-off">{{ t('mods.disabled') }}</span>
          <span v-if="mod.manifestMissing" class="badge">{{ t('mods.noManifest') }}</span>
        </div>
        <p v-if="mod.author" class="muted small">{{ t('mods.by', { author: mod.author }) }}</p>
        <p v-if="mod.description" class="small">{{ mod.description }}</p>
        <p class="game-path" dir="ltr">{{ mod.folder }}</p>

        <div class="mod-actions">
          <button :disabled="mods.busy" @click="toggle(mod)">
            {{ t(mod.enabled ? 'mods.disable' : 'mods.enable') }}
          </button>
          <button v-if="mod.nexusId" :disabled="mods.busy" @click="openNexus(mod)">{{ t('mods.openNexus') }}</button>
          <button
            v-if="mods.confirmingRemove !== mod.folder"
            class="danger"
            :disabled="mods.busy"
            @click="mods.askRemove(mod.folder)"
          >
            {{ t('mods.remove') }}
          </button>
        </div>

        <div v-if="mods.confirmingRemove === mod.folder" class="confirm">
          <p><strong>{{ t('mods.removeConfirm', { name: mod.name }) }}</strong></p>
          <p class="warning small">{{ t('mods.removeWarning') }}</p>
          <div class="actions">
            <button class="danger" :disabled="mods.busy" @click="confirmRemove(mod)">{{ t('mods.removeConfirmAction') }}</button>
            <button :disabled="mods.busy" @click="mods.cancelRemove()">{{ t('mods.cancel') }}</button>
          </div>
        </div>
      </article>
    </section>
    <section v-else-if="game.selectedPath && !mods.busy" class="empty" role="status">
      <h2>{{ t('mods.empty') }}</h2>
      <p class="muted">{{ t('mods.emptyHint') }}</p>
    </section>
  </section>
</template>

<style scoped>
.mod-card { padding: 18px 20px; }
.mod-head { display: flex; flex-wrap: wrap; align-items: baseline; gap: 10px; }
.mod-name { font-size: 15px; margin: 0; }
.mod-disabled .mod-name { color: var(--text-muted); }
.badge-off { background: var(--bg-subtle); color: var(--text-muted); }
.mod-actions { display: flex; flex-wrap: wrap; gap: 10px; margin-top: 4px; }
.confirm { margin-top: 14px; padding: 14px 16px; border: 1px solid var(--danger-border); background: var(--danger-bg); border-radius: 8px; }
.confirm p { margin-block: 0 8px; }
.warning { color: var(--danger-text); }
button.danger { background: var(--danger-text); border-color: var(--danger-text); color: var(--bg-panel); }
button.danger:hover:not(:disabled) { background: var(--danger-text); border-color: var(--danger-text); opacity: .88; }
</style>
