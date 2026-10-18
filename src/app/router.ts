import { createRouter, createWebHashHistory } from 'vue-router';
import GameDirectoryPage from '../features/game/GameDirectoryPage.vue';
import ModsPage from '../features/mods/ModsPage.vue';
import NexusPage from '../features/nexus/NexusPage.vue';
import OverviewPage from '../features/overview/OverviewPage.vue';
import SettingsPage from '../features/settings/SettingsPage.vue';

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', redirect: '/game' },
    { path: '/overview', name: 'overview', component: OverviewPage },
    { path: '/game', name: 'game', component: GameDirectoryPage },
    { path: '/mods', name: 'mods', component: ModsPage },
    { path: '/browse', name: 'nexus', component: NexusPage },
    { path: '/settings', name: 'settings', component: SettingsPage },
    { path: '/:pathMatch(.*)*', redirect: '/game' },
  ],
});
