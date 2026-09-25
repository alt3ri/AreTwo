<script setup lang="ts">
import UiIcon from "./UiIcon.vue";
import FileIcon from "./FileIcon.vue";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { formatBytes, previewKind, typeLabel, type Row } from "../rows";
import { errorFor, peek, request } from "../stores/thumbs";

const props = defineProps<{
  rows: Row[];
  selected: string[];
  profileId: string;
  bucket: string;
  loading: boolean;
  hasMore: boolean;
}>();

const emit = defineEmits<{
  select: [row: Row, event: MouseEvent];
  open: [row: Row];
  menu: [row: Row | null, x: number, y: number];
  more: [];
}>();

const TILE_W = 168;
const TILE_H = 196;

const scroller = ref<HTMLDivElement | null>(null);
const scrollTop = ref(0);
const width = ref(900);
const height = ref(600);
let observer: ResizeObserver | null = null;

const perRow = computed(() => Math.max(1, Math.floor((width.value - 32 + 12) / (TILE_W + 12))));
const rowCount = computed(() => Math.ceil(props.rows.length / perRow.value));
const rowH = TILE_H + 12;
const firstRow = computed(() => Math.max(0, Math.floor(scrollTop.value / rowH) - 2));
const lastRow = computed(() =>
  Math.min(rowCount.value, Math.ceil((scrollTop.value + height.value) / rowH) + 2),
);

/** Only tiles inside the window exist in the DOM, so only they fetch. */
const tiles = computed(() => {
  const out: { row: Row; index: number }[] = [];
  for (let r = firstRow.value; r < lastRow.value; r++) {
    for (let c = 0; c < perRow.value; c++) {
      const index = r * perRow.value + c;
      if (index >= props.rows.length) break;
      out.push({ row: props.rows[index], index });
    }
  }
  return out;
});

const padTop = computed(() => firstRow.value * rowH);
const padBottom = computed(() => Math.max(0, (rowCount.value - lastRow.value) * rowH));

function measure() {
  const el = scroller.value;
  if (!el) return;
  width.value = el.clientWidth;
  height.value = el.clientHeight;
  scrollTop.value = el.scrollTop;
}

function onScroll() {
  const el = scroller.value;
  if (!el) return;
  scrollTop.value = el.scrollTop;
  if (props.hasMore && !props.loading && el.scrollTop + el.clientHeight >= el.scrollHeight - 400) {
    emit("more");
  }
}

function isImage(row: Row): boolean {
  return row.kind === "file" && previewKind(row.name) === "image";
}

function thumb(row: Row): string | undefined {
  return props.bucket ? peek(props.profileId, props.bucket, row.key) : undefined;
}

function failure(row: Row): string | undefined {
  return props.bucket ? errorFor(props.profileId, props.bucket, row.key) : undefined;
}

watch(
  tiles,
  (list) => {
    if (!props.bucket) return;
    for (const { row } of list) if (isImage(row)) request(props.profileId, props.bucket, row.key);
  },
  { immediate: true },
);

onMounted(() => {
  measure();
  observer = new ResizeObserver(measure);
  if (scroller.value) observer.observe(scroller.value);
});
onUnmounted(() => observer?.disconnect());
</script>

<template>
  <div
    ref="scroller"
    class="gallery"
    @scroll="onScroll"
    @contextmenu.prevent="emit('menu', null, $event.clientX, $event.clientY)"
  >
    <div :style="{ height: padTop + 'px' }"></div>
    <div class="grid" :style="{ gridTemplateColumns: `repeat(${perRow}, ${TILE_W}px)` }">
      <button
        v-for="tile in tiles"
        :key="tile.row.key"
        type="button"
        class="tile"
        :class="{ sel: selected.includes(tile.row.key) }"
        :title="`${tile.row.name} — ${typeLabel(tile.row.name, tile.row.kind)}`"
        @click="emit('select', tile.row, $event)"
        @dblclick="emit('open', tile.row)"
        @contextmenu.prevent.stop="emit('menu', tile.row, $event.clientX, $event.clientY)"
      >
        <span class="art">
          <img v-if="thumb(tile.row)" :src="thumb(tile.row)" alt="" loading="lazy" />
          <span v-else-if="failure(tile.row)" class="ph warn">no preview</span>
          <span v-else-if="isImage(tile.row)" class="ph">…</span>
          <UiIcon v-else-if="tile.row.kind === 'folder'" class="file-glyph folder" :name="tile.row.bucketName ? 'bucket' : 'folder'" />
          <FileIcon v-else class="file-glyph" :name="tile.row.name" />
        </span>
        <span class="cap">{{ tile.row.name }}</span>
        <span class="sub">{{
          tile.row.kind === "folder" ? "Folder" : formatBytes(tile.row.size)
        }}</span>
      </button>
    </div>
    <div :style="{ height: padBottom + 'px' }"></div>
    <p v-if="loading" class="note">Loading…</p>
    <p v-else-if="!rows.length" class="note">No items to show.</p>
  </div>
</template>

<style scoped>
.gallery {
  flex: 1;
  min-height: 0;
  padding: 8px;
  overflow: auto;
}
.grid {
  display: grid;
  justify-content: start;
  gap: 12px;
}
.tile {
  display: flex;
  flex-direction: column;
  gap: 3px;
  width: 168px;
  padding: 3px;
  border: 1px solid transparent;
  border-radius: 4px;
  background: none;
  color: var(--color-text);
  font: inherit;
  font-size: 12px;
  text-align: left;
  cursor: default;
}
.tile:hover {
  background: var(--color-raised);
}
.tile.sel {
  border-color: var(--color-accent);
  background: var(--color-accent-soft);
}
.art {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 156px;
  border: 1px solid var(--color-line);
  background: var(--color-field);
  overflow: hidden;
}
.art img {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
}
.ph {
  color: var(--color-disabled);
  font-size: 12px;
}
.ph.glyph {
  font-size: 30px;
}
.ph.warn {
  color: var(--color-warn);
}
.cap {
  overflow: hidden;
  line-height: 1.35;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sub {
  color: var(--color-text-faint);
  font-size: 11px;
}
.note {
  margin: 10px;
  color: var(--color-text-faint);
  font-size: 13px;
}

.gallery { min-width: 0; padding: var(--space-5); }
.file-glyph { width: 40px; height: 40px; color: var(--color-text-faint); }
.file-glyph.folder { color: var(--color-folder); }
.tile { height: 196px; }
.tile:focus-visible { outline: 2px solid var(--color-focus); outline-offset: -2px; }
.note { margin: var(--space-7) var(--space-5); text-align: center; }

</style>
