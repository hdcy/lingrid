<script setup lang="ts">
import { ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { openUrl } from '@tauri-apps/plugin-opener';
import { useGameStore } from '../game/store';
import { useNexusStore } from './store';
import type { NexusModSummary } from './types';

const { t, te } = useI18n();
const game = useGameStore();
const nexus = useNexusStore();
const query = ref('');
const nxmUrl = ref('');
const errorText = (code: string) => t(te(`errors.${code}`) ? `errors.${code}` : 'errors.unexpected');

// Whether a key is stored decides if downloading from a link can work at all.
void nexus.loadKeyStatus();

function search() {
  void nexus.search(query.value);
}

function open(mod: NexusModSummary) {
  void openUrl(mod.url).catch(() => undefined);
}

function downloads(value: number | null): string {
  return value === null ? '' : value.toLocaleString();
}

/** A link that names a mod but no file: `nxm://…/mods/123` or the website URL. */
const MOD_LINK = /^(?:nxm:\/\/stardewvalley\/mods\/(\d+)|https?:\/\/(?:www\.)?nexusmods\.com\/stardewvalley\/mods\/(\d+))/i;

function installFromNxm() {
  const value = nxmUrl.value.trim();
  if (!value) return;
  // A link that only names the mod cannot be downloaded from — open its page instead so
  // the user can pick a file there.
  const page = MOD_LINK.exec(value);
  if (page && !/\/files\/\d+/.test(value)) {
    void openUrl(`https://www.nexusmods.com/stardewvalley/mods/${page[1] ?? page[2]}`).catch(() => undefined);
    return;
  }
  const path = game.selectedPath;
  if (path) void nexus.install(path, value);
}
</script>

<template>
  <section class="page">
    <header class="page-heading">
      <h1>{{ t('nexus.title') }}</h1>
      <p class="muted">{{ t('nexus.description') }}</p>
    </header>

    <section class="panel" :aria-busy="nexus.busy">
      <form class="search-form" @submit.prevent="search">
        <label for="nexus-query">{{ t('nexus.searchLabel') }}</label>
        <div class="path-form">
          <input
            id="nexus-query"
            v-model="query"
            type="search"
            dir="ltr"
            autocomplete="off"
            spellcheck="false"
            :disabled="nexus.busy"
            :placeholder="t('nexus.searchPlaceholder')"
          />
          <button class="primary" type="submit" :disabled="nexus.busy">{{ t(nexus.busy ? 'nexus.searching' : 'nexus.search') }}</button>
        </div>
      </form>
      <p class="muted small">{{ t('nexus.hint') }}</p>
    </section>

    <section class="panel">
      <h2 class="section-title">{{ t('nexus.nxmTitle') }}</h2>
      <p class="muted small">{{ t('nexus.nxmHint') }}</p>
      <form class="search-form" @submit.prevent="installFromNxm">
        <div class="path-form">
          <input
            v-model="nxmUrl"
            type="text"
            dir="ltr"
            autocomplete="off"
            spellcheck="false"
            :disabled="nexus.busy"
            :placeholder="t('nexus.nxmPlaceholder')"
            :aria-label="t('nexus.nxmTitle')"
          />
          <button class="primary" type="submit" :disabled="nexus.busy || !nxmUrl.trim() || !game.selectedPath">
            {{ t(nexus.busy ? 'nexus.installing' : 'nexus.nxmInstall') }}
          </button>
        </div>
      </form>
      <p v-if="!game.selectedPath" class="muted small">{{ t('mods.noGame') }}</p>
      <p v-else-if="!nexus.hasKey" class="warning small">{{ t('nexus.keyRequired') }}</p>
    </section>

    <div v-if="nexus.error" class="notice error" role="alert">
      {{ errorText(nexus.error.code) }}
      <details v-if="nexus.error.detail"><summary>{{ t('diagnostics') }}</summary><pre>{{ nexus.error.detail }}</pre></details>
    </div>
    <div v-if="nexus.notice === 'nexus.installedNotice'" class="notice success" role="status">
      <strong>{{ t('nexus.installedNotice') }}</strong>
      <p v-if="nexus.installed.length" class="small" dir="ltr">{{ nexus.installed.join(', ') }}</p>
    </div>
    <div v-else-if="nexus.notice" class="notice success" role="status">{{ t(nexus.notice) }}</div>

    <section v-if="nexus.results.length" aria-live="polite">
      <article v-for="mod in nexus.results" :key="mod.modId" class="panel nexus-result">
        <div class="nexus-head">
          <h2 class="nexus-name">{{ mod.name }}</h2>
          <span v-if="mod.version" class="badge">{{ mod.version }}</span>
        </div>
        <p class="muted small">
          <span v-if="mod.author">{{ t('mods.by', { author: mod.author }) }}</span>
          <span v-if="mod.downloads !== null"> · {{ t('nexus.downloads', { count: downloads(mod.downloads) }) }}</span>
          <span> · {{ t('nexus.id', { id: mod.modId }) }}</span>
        </p>
        <p v-if="mod.summary" class="small">{{ mod.summary }}</p>
        <div class="nexus-actions">
          <button :disabled="nexus.busy" @click="open(mod)">{{ t('nexus.open') }}</button>
        </div>
      </article>
    </section>
    <section v-else-if="nexus.searched && !nexus.busy" class="empty" role="status">
      <h2>{{ t('nexus.empty') }}</h2>
      <p class="muted">{{ t('nexus.emptyHint') }}</p>
    </section>
  </section>
</template>

<style scoped>
.search-form { margin-top: 0; }
.nexus-result { padding: 18px 20px; }
.nexus-head { display: flex; flex-wrap: wrap; align-items: baseline; gap: 10px; }
.nexus-name { font-size: 15px; margin: 0; }
.nexus-actions { display: flex; flex-wrap: wrap; gap: 10px; margin-top: 4px; }
.warning { color: var(--danger-text); }
</style>
