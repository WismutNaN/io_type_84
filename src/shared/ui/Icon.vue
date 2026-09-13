<script setup lang="ts">
defineProps<{ name: string }>();
const paths: Record<string, string> = {
  keyboard: 'M3 6h18v12H3z M6 9h1m3 0h1m3 0h1m3 0h1M6 12h1m3 0h1m3 0h1m3 0h1M7 15h10',
  light: 'M9 18h6m-5 3h4 M8 14a6 6 0 1 1 8 0c-1 1-1 2-1 2H9s0-1-1-2z',
  macro: 'M5 5h14M5 12h14M5 19h8 M3 5h0m0 7h0m0 7h0m14-1 3 2-3 2',
  profile: 'M4 5h6l2 3h8v12H4z',
  settings: 'M4 6h16M4 12h16M4 18h16 M8 3v6m8 0v6M9 15v6',
  undo: 'M8 5 3 10l5 5M3 10h10a6 6 0 0 1 0 12',
  redo: 'm16 5 5 5-5 5m5-5H11a6 6 0 0 0 0 12',
  monitor: 'M3 4h18v13H3zM8 21h8m-4-4v4M6 11h3l2-4 3 7 2-3h2',
  volume: 'M4 9h4l5-4v14l-5-4H4zM16 8a6 6 0 0 1 0 8m3-11a10 10 0 0 1 0 14',
  chevron: 'm9 5 7 7-7 7',
  close: 'm6 6 12 12M18 6 6 18',
  search: 'M16 10a6 6 0 1 1-12 0 6 6 0 0 1 12 0m-2 4 6 6',
  plus: 'M12 4v16M4 12h16',
};
</script>
<template>
  <svg
    class="icon"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.6"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path :d="paths[name] ?? paths.keyboard" />
  </svg>
</template>
