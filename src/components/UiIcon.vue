<script setup lang="ts">
defineProps<{ name: keyof typeof paths }>();

const paths = {
  folder: 'M3 7V5h6l2 2h10v13H3Z',
  file: 'M6 3h8l4 4v14H6Z M14 3v5h4',
  bucket: 'M4 5h16l-2 15H6Z M4 9h16',
  back: 'm14 5-7 7 7 7 M7 12h14',
  forward: 'm10 5 7 7-7 7 M3 12h14',
  up: 'm5 12 7-7 7 7 M12 5v16',
  refresh: 'M20 10a8 8 0 1 0-1 8 M20 4v6h-6',
  plus: 'M12 5v14 M5 12h14',
  close: 'm6 6 12 12 M6 18 18 6',
  chevron: 'm8 10 4 4 4-4',
  upload: 'm8 8 4-4 4 4 M12 4v12 M4 15v5h16v-5',
  download: 'm8 12 4 4 4-4 M12 4v12 M4 15v5h16v-5',
  search: 'M10 3a7 7 0 1 0 0 14 7 7 0 0 0 0-14 M15 15l6 6',
  list: 'M8 6h13 M8 12h13 M8 18h13 M3 6h.01 M3 12h.01 M3 18h.01',
  grid: 'M3 3h7v7H3Z M14 3h7v7h-7Z M3 14h7v7H3Z M14 14h7v7h-7Z',
  preview: 'M3 4h18v16H3Z M14 4v16',
  trash: 'M3 6h18 M9 6V3h6v3 M5 6l1 15h12l1-15 M10 10v7 M14 10v7',
};
</script>

<template>
  <svg class="ui-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path :d="paths[name]" />
  </svg>
</template>

<style scoped>
.ui-icon { width: 16px; height: 16px; flex: none; }
</style>
