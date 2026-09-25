<script setup lang="ts">
import { fileIconName } from "../fileIcons";

defineProps<{ name: string }>();
const base = `${import.meta.env.BASE_URL}icons/material/`;
</script>

<template>
  <img class="file-icon" :src="`${base}${fileIconName(name)}.svg`" alt="" aria-hidden="true" draggable="false" />
</template>

<style scoped>
.file-icon { width: 16px; height: 16px; flex: none; object-fit: contain; }
</style>
