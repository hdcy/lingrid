<script setup lang="ts">
import { ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { useGameStore } from './store';
import SmapiPanel from '../smapi/SmapiPanel.vue';

const { t, te } = useI18n();
const game = useGameStore();
const manualPath = ref('');
const errorText = (code: string) => t(te(`errors.${code}`) ? `errors.${code}` : 'errors.unexpected');
</script>

<template>
  <section class="page">
    <header class="page-heading">
      <h1>{{ t('title') }}</h1>
      <p class="muted">{{ t('subtitle') }}</p>
    </header>
    <section class="panel" :aria-busy="game.busy">
      <div class="actions">
        <button class="primary" :disabled="game.busy" @click="game.scan()">{{ t(game.busy ? 'scanning' : 'scan') }}</button>
        <button :disabled="game.busy" @click="game.addDirectory()">{{ t('browse') }}</button>
      </div>
      <p class="muted small">{{ t('coverage') }}</p>
      <form @submit.prevent="game.addDirectory(manualPath)">
        <label for="game-path">{{ t('manualLabel') }}</label>
        <div class="path-form">
          <input id="game-path" v-model="manualPath" type="text" dir="ltr" autocomplete="off" spellcheck="false" :disabled="game.busy" aria-describedby="path-hint" />
          <button :disabled="game.busy || !manualPath.trim()" type="submit">{{ t('validate') }}</button>
        </div>
        <p id="path-hint" class="muted small">{{ t('manualHint') }}</p>
      </form>
    </section>

    <div v-if="game.error" class="notice error" role="alert">
      {{ errorText(game.error.code) }}
      <details v-if="game.error.detail"><summary>{{ t('diagnostics') }}</summary><pre>{{ game.error.detail }}</pre></details>
    </div>
    <div v-if="game.selectionNotice" class="notice" role="status">{{ t('selectionMissing') }}</div>

    <section v-if="game.installations.length" aria-live="polite">
      <h2>{{ t('found', { count: game.installations.length }) }}</h2>
      <article v-for="installation in game.installations" :key="installation.path" class="panel installation" :class="{ active: game.selectedPath === installation.path }">
        <div class="badges">
          <span v-for="source in installation.sources" :key="source" class="badge">{{ t(`sources.${source}`) }}</span>
          <span class="muted small">{{ t(`runtimes.${installation.runtime}`) }}</span>
        </div>
        <p class="game-path" dir="ltr">{{ installation.path }}</p>
        <div class="installation-footer">
          <span class="small muted">{{ t(installation.smapiPath ? 'smapiFound' : 'smapiMissing') }}</span>
          <button :disabled="game.busy || game.selectedPath === installation.path" :aria-pressed="game.selectedPath === installation.path" @click="game.select(installation)">
            {{ t(game.selectedPath === installation.path ? 'selected' : 'select') }}
          </button>
        </div>
      </article>
      <p class="muted small">{{ t('smapiHint') }}</p>
    </section>
    <section v-else-if="game.scanned && !game.busy" class="empty" role="status">
      <h2>{{ t('empty') }}</h2><p class="muted">{{ t('emptyHint') }}</p>
    </section>

    <section v-if="game.selected" class="notice success" role="status">
      <strong>{{ t('ready') }}</strong><p class="small">{{ t('readyHint') }}</p>
    </section>
    <SmapiPanel v-if="game.selected" />
    <details v-if="game.warnings.length" class="panel warning-list">
      <summary>{{ t('warnings', { count: game.warnings.length }) }}</summary>
      <ul><li v-for="(warning, index) in game.warnings" :key="index">
        <p>{{ errorText(warning.code) }}</p><code dir="ltr">{{ warning.path }}</code>
        <details v-if="warning.detail"><summary>{{ t('diagnostics') }}</summary><pre>{{ warning.detail }}</pre></details>
      </li></ul>
    </details>
  </section>
</template>
