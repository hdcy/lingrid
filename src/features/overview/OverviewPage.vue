<script setup lang="ts">
import { computed, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { useGameStore } from '../game/store';
import { useSmapiStore } from '../smapi/store';
import AppIcon from '../../shared/AppIcon.vue';

const { t } = useI18n();
const game = useGameStore();
const smapi = useSmapiStore();
// Shares the store with the directory page, so only one page performs the lookup.
watch(() => game.selectedPath, path => { void smapi.refresh(path); }, { immediate: true });

const smapiStatus = computed(() => {
  if (!game.selected) return t('shell.noSelection');
  if (!smapi.installed) return t('smapi.notInstalled');
  const version = smapi.status?.version;
  return version ? t('smapi.installed', { version }) : t('smapi.installedUnknown');
});
</script>

<template>
  <section class="page">
    <header class="page-heading">
      <h1>{{ t('nav.overview') }}</h1>
      <p class="muted">{{ t('overview.description') }}</p>
    </header>
    <section class="panel overview-game">
      <div class="game-cover"><AppIcon name="leaf" /></div>
      <div class="overview-game-info">
        <p class="eyebrow">{{ t('shell.currentGame') }}</p>
        <h2>Stardew Valley</h2>
        <p class="muted small">{{ t(game.selected ? 'ready' : 'shell.noSelection') }}</p>
      </div>
      <RouterLink class="button-link" to="/game">{{ t('overview.manage') }}<AppIcon name="arrow" /></RouterLink>
    </section>
    <div class="overview-grid">
      <section class="panel">
        <h2 class="section-title">{{ t('overview.installations') }}</h2>
        <p class="stat-value">{{ game.scanned ? game.installations.length : '—' }}</p>
        <p class="muted small">{{ t(game.busy ? 'scanning' : game.scanned ? 'overview.scanComplete' : 'overview.scanPending') }}</p>
      </section>
      <section class="panel">
        <h2 class="section-title">SMAPI</h2>
        <p>{{ smapiStatus }}</p>
        <p class="muted small">{{ t('smapiHint') }}</p>
      </section>
    </div>
    <section class="panel">
      <h2 class="section-title">{{ t('selected') }}</h2>
      <p v-if="game.selected" class="game-path" dir="ltr">{{ game.selected.path }}</p>
      <p v-else class="muted small">{{ t('overview.chooseHint') }}</p>
    </section>
  </section>
</template>
