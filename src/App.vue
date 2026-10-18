<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch, watchEffect } from 'vue';
import { useRoute } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { navigation } from './app/navigation';
import { useGameStore } from './features/game/store';
import { useNexusStore } from './features/nexus/store';
import { preferenceError } from './shared/preferences';
import AppIcon from './shared/AppIcon.vue';

const { t } = useI18n();
const route = useRoute();
const game = useGameStore();
const nexus = useNexusStore();
const content = ref<HTMLElement | null>(null);
const pageTitle = computed(() => t(navigation.find(item => item.name === route.name)?.label ?? 'title'));
const status = computed(() => game.busy ? 'scanning' : game.error || game.selectionNotice ? 'shell.attention' : game.selected ? 'shell.selected' : 'shell.noSelection');

onMounted(() => { void game.scan(); });
// A link clicked in the browser reaches the app as an event; install it into the
// selected game, which may finish loading after the link arrives.
void nexus.ensureListening();
watch(() => nexus.incoming, () => { void nexus.acceptIncoming(game.selectedPath); });
watchEffect(() => { document.title = `${pageTitle.value} · Lingrid`; });
watch(() => route.path, async () => {
  await nextTick();
  if (content.value) {
    content.value.scrollTop = 0;
    content.value.focus({ preventScroll: true });
  }
});
</script>

<template>
  <div class="desktop-shell">
    <a class="skip-link" href="#page-content">{{ t('shell.skip') }}</a>
    <aside class="sidebar">
      <div class="brand" title="Lingrid">
        <span class="brand-mark"><AppIcon name="leaf" /></span>
        <div class="brand-copy"><strong>Lingrid</strong><span>Stardew Valley</span></div>
      </div>
      <p class="nav-caption">{{ t('nav.workspace') }}</p>
      <nav :aria-label="t('nav.label')" class="navigation">
        <RouterLink v-for="item in navigation" :key="item.name" :to="item.path" class="nav-item" :class="{ 'nav-bottom': item.bottom }" :title="t(item.label)">
          <AppIcon :name="item.icon" /><span class="nav-label">{{ t(item.label) }}</span>
        </RouterLink>
      </nav>
      <div class="sidebar-game" :title="game.selected?.path ?? t('shell.noSelection')">
        <span class="status-dot" :class="{ connected: !!game.selected }"></span>
        <div class="sidebar-game-copy"><span>{{ t('shell.currentGame') }}</span><strong>Stardew Valley</strong></div>
      </div>
    </aside>

    <div class="workspace">
      <header class="workspace-header">
        <div class="breadcrumb"><span>Lingrid</span><span aria-hidden="true">/</span><strong>{{ pageTitle }}</strong></div>
        <span class="workspace-context">Stardew Valley</span>
      </header>
      <main id="page-content" ref="content" class="page-content" tabindex="-1" :aria-label="pageTitle">
        <div v-if="preferenceError" class="notice" role="alert">{{ t('storageError') }}</div>
        <RouterView v-slot="{ Component }">
          <KeepAlive :max="5"><component :is="Component" /></KeepAlive>
        </RouterView>
      </main>
      <footer class="statusbar">
        <span class="statusbar-state" role="status"><span class="status-dot" :class="{ connected: !!game.selected && !game.error && !game.selectionNotice, busy: game.busy }"></span>{{ t(status) }}</span>
        <span class="statusbar-label">{{ t('shell.local') }}</span>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.desktop-shell { display: grid; grid-template-columns: 216px minmax(0, 1fr); height: 100vh; height: 100dvh; overflow: hidden; }
.sidebar { min-height: 0; display: flex; flex-direction: column; background: var(--bg-sidebar); border-inline-end: 1px solid var(--border); padding: 24px 14px 14px; }
.brand { display: flex; align-items: center; gap: 11px; padding-inline: 10px; margin-bottom: 36px; }
.brand-mark { display: grid; place-items: center; width: 37px; height: 37px; flex-shrink: 0; color: var(--accent-contrast); background: var(--accent); border-radius: 11px; }
.brand-mark svg { width: 23px; height: 23px; }
.brand-copy { display: flex; flex-direction: column; }
.brand-copy strong { font-size: 19px; line-height: 1.3; letter-spacing: -.5px; }
.brand-copy span { font-size: 11px; color: var(--text-muted); margin-top: 3px; }
.nav-caption { padding-inline: 12px; font-size: 11px; color: var(--text-muted); margin: 0 0 10px; letter-spacing: .7px; }
.navigation { display: flex; flex-direction: column; gap: 5px; flex: 1; min-height: 0; overflow-y: auto; }
.nav-item { display: flex; align-items: center; gap: 11px; min-height: 43px; flex-shrink: 0; padding: 10px 12px; border-radius: 7px; text-decoration: none; font-size: 13px; font-weight: 500; color: var(--text-muted); border: 1px solid transparent; }
.nav-item svg { width: 18px; height: 18px; flex-shrink: 0; }
.nav-item:hover { background: var(--bg-hover); color: var(--accent-text); }
.nav-item.router-link-exact-active { color: var(--accent-text); background: var(--accent-soft); border-color: var(--accent-soft-border); font-weight: 650; }
.nav-bottom { margin-top: auto; }
.sidebar-game { display: flex; align-items: center; gap: 10px; margin-top: 16px; padding: 15px 10px 2px; border-top: 1px solid var(--border); }
.sidebar-game-copy { display: flex; flex-direction: column; min-width: 0; }
.sidebar-game-copy span { font-size: 10px; color: var(--text-muted); }
.sidebar-game-copy strong { font-size: 12px; font-weight: 550; }
.workspace { display: flex; flex-direction: column; min-width: 0; min-height: 0; background: var(--bg-app); }
.workspace-header { display: flex; align-items: center; justify-content: space-between; gap: 16px; min-height: 55px; flex-shrink: 0; padding: 12px 28px; border-bottom: 1px solid var(--border); background: var(--bg-header); font-size: 12px; }
.breadcrumb { display: flex; align-items: center; gap: 12px; color: var(--text-muted); }
.breadcrumb strong { color: var(--text); font-weight: 500; }
.workspace-context { color: var(--text-muted); font-size: 11px; }
.page-content { flex: 1; overflow: auto; min-height: 0; padding: 28px; scrollbar-gutter: stable; }
.page-content:focus { outline: none; }
.statusbar { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 6px 20px; border-top: 1px solid var(--border); font-size: 11px; color: var(--text-muted); flex-shrink: 0; background: var(--bg-statusbar); }
.statusbar-state { display: flex; gap: 7px; align-items: center; }
.status-dot { width: 6px; height: 6px; flex-shrink: 0; border-radius: 50%; background: var(--border-strong); }
.status-dot.connected { background: var(--accent); }
.status-dot.busy { background: var(--warning); }
.skip-link { position: fixed; inset-block-start: -100px; inset-inline-start: 10px; z-index: 10; padding: 8px 16px; background: var(--bg-panel); color: var(--text); }
.skip-link:focus { inset-block-start: 10px; }
@media (max-width: 760px) {
  .desktop-shell { grid-template-columns: 68px minmax(0, 1fr); }
  .sidebar { padding-inline: 9px; }
  .brand { justify-content: center; padding-inline: 0; }
  .brand-copy, .nav-caption, .sidebar-game-copy { display: none; }
  .nav-label { position: absolute; width: 1px; height: 1px; padding: 0; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; }
  .nav-item { justify-content: center; padding-inline: 0; }
  .sidebar-game { justify-content: center; padding-inline: 0; }
  .page-content { padding: 20px; }
  .workspace-header { padding-inline: 20px; }
}
@media (max-width: 460px) {
  .page-content { padding: 14px; }
  .workspace-context, .statusbar-label { display: none; }
}
</style>

<style src="./styles.css"></style>
