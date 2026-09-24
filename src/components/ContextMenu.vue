<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import type { MenuItem } from "../rows";

const { x, y, items } = defineProps<{ x: number; y: number; items: MenuItem[] }>();
const emit = defineEmits<{ pick: [item: MenuItem]; close: [] }>();

function onPointerDown(event: MouseEvent) {
  if (!(event.target as HTMLElement).closest(".ctx")) emit("close");
}

function onKey(event: KeyboardEvent) {
  if (event.key === "Escape") emit("close");
}

onMounted(() => {
  window.addEventListener("mousedown", onPointerDown, true);
  window.addEventListener("keydown", onKey);
});
onUnmounted(() => {
  window.removeEventListener("mousedown", onPointerDown, true);
  window.removeEventListener("keydown", onKey);
});
</script>

<template>
  <ul class="ctx" :style="{ left: x + 'px', top: y + 'px' }">
    <li v-for="item in items" :key="item.id">
      <button
        type="button"
        :class="{ danger: item.danger }"
        :disabled="item.disabled"
        @click="emit('pick', item)"
      >
        {{ item.label }}
      </button>
    </li>
  </ul>
</template>

<style scoped>
.ctx {
  position: fixed;
  z-index: 60;
  min-width: 200px;
  margin: 0;
  padding: 4px;
  list-style: none;
  background: var(--color-raised);
  border: 1px solid var(--color-line-strong);
  border-radius: 6px;
  box-shadow: 0 10px 28px var(--color-backdrop);
}
.ctx button {
  display: block;
  width: 100%;
  padding: 6px 10px;
  border: 0;
  border-radius: 4px;
  background: none;
  color: var(--color-text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.ctx button:hover:not(:disabled) {
  background: var(--color-line);
}
.ctx button:disabled {
  color: var(--color-disabled);
  cursor: default;
}
.ctx button.danger {
  color: var(--color-danger);
}
</style>
