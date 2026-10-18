export const navigation = [
  { name: 'overview', path: '/overview', label: 'nav.overview', icon: 'overview', bottom: false },
  { name: 'game', path: '/game', label: 'title', icon: 'folder', bottom: false },
  { name: 'mods', path: '/mods', label: 'mods.title', icon: 'mods', bottom: false },
  { name: 'nexus', path: '/browse', label: 'nexus.title', icon: 'search', bottom: false },
  { name: 'settings', path: '/settings', label: 'nav.settings', icon: 'settings', bottom: true },
] as const;
