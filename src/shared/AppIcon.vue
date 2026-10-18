<script setup lang="ts">
defineProps<{ name: 'overview' | 'folder' | 'settings' | 'leaf' | 'mods' | 'search' | 'arrow' }>();
</script>

<template>
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.65" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">
    <template v-if="name === 'overview'">
      <rect x="3" y="3" width="7" height="7" rx="1.5" /><rect x="14" y="3" width="7" height="7" rx="1.5" />
      <rect x="3" y="14" width="7" height="7" rx="1.5" /><rect x="14" y="14" width="7" height="7" rx="1.5" />
    </template>
    <path v-else-if="name === 'folder'" d="M3 7V5a2 2 0 0 1 2-2h5l2 3h7a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7h18" />
    <template v-else-if="name === 'mods'">
      <path d="M20 7.5 12 3 4 7.5v9L12 21l8-4.5v-9Z" /><path d="m4 7.5 8 4.5 8-4.5" /><path d="M12 21v-9" />
    </template>
    <template v-else-if="name === 'search'">
      <circle cx="11" cy="11" r="7" /><path d="m20.5 20.5-3.8-3.8" />
    </template>
    <template v-else-if="name === 'settings'">
      <circle cx="12" cy="12" r="3" />
      <path d="m10 3-.6 2.2-2 .9L5.3 5.5 3.5 8.6 5 10.2v3.6l-1.5 1.6 1.8 3.1 2.1-.6 2 .9L10 21h4l.6-2.2 2-.9 2.1.6 1.8-3.1-1.5-1.6v-3.6l1.5-1.6-1.8-3.1-2.1.6-2-.9L14 3z" />
    </template>
    <template v-else-if="name === 'leaf'">
      <path d="M20 4c-8-2-15 2-15 8a6 6 0 0 0 6 6c6 0 9-6 9-14Z" /><path d="M4 21 15 10" />
    </template>
    <path v-else d="M5 12h14m-5-5 5 5-5 5" />
  </svg>
</template>
