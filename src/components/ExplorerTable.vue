<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { ROW_HEIGHT, formatBytes, formatDate, type Row, type SortKey } from "../rows";

const { rows, selected, cursor, sortKey, sortDir, loading, hasMore } = defineProps<{
  rows: Row[];
  selected: string[];
  cursor: number;
  sortKey: SortKey;
  sortDir: 1 | -1;
  loading: boolean;
  hasMore: boolean;
}>();

const emit = defineEmits<{
  select: [row: Row, event: MouseEvent];
  open: [row: Row];
  menu: [row: Row | null, x: number, y: number];
  sort: [key: SortKey];
  more: [];
}>();

const COLUMNS: { key: SortKey; label: string }[] = [
  { key: "name", label: "Name" },
  { key: "size", label: "Size" },
  { key: "lastModified", label: "Modified" },
  { key: "storageClass", label: "Class" },
];

const OVERSCAN = 6;
const scroller = ref<HTMLDivElement | null>(null);
const scrollTop = ref(0);
const viewport = ref(480);
let observer: ResizeObserver | null = null;

const start = computed(() => Math.max(0, Math.floor(scrollTop.value / ROW_HEIGHT) - OVERSCAN));
const end = computed(() =>
  Math.min(rows.length, Math.ceil((scrollTop.value + viewport.value) / ROW_HEIGHT) + OVERSCAN),
);
const windowRows = computed(() =>
  rows.slice(start.value, end.value).map((row, i) => ({ row, index: start.value + i })),
);
const padTop = computed(() => start.value * ROW_HEIGHT);
const padBottom = computed(() => Math.max(0, (rows.length - end.value) * ROW_HEIGHT));

function measure() {
  const el = scroller.value;
  if (!el) return;
  viewport.value = el.clientHeight;
  scrollTop.value = el.scrollTop;
}

function onScroll() {
  const el = scroller.value;
  if (!el) return;
  scrollTop.value = el.scrollTop;
  if (hasMore && !loading && el.scrollTop + el.clientHeight >= el.scrollHeight - 160) emit("more");
}

onMounted(() => {
  measure();
  observer = new ResizeObserver(measure);
  if (scroller.value) observer.observe(scroller.value);
});
onUnmounted(() => observer?.disconnect());

// Keep the keyboard cursor on screen.
watch(
  () => cursor,
  (index) => {
    const el = scroller.value;
    if (!el || index < 0) return;
    const top = index * ROW_HEIGHT;
    if (top < el.scrollTop) el.scrollTop = top;
    else if (top + ROW_HEIGHT > el.scrollTop + el.clientHeight) {
      el.scrollTop = top + ROW_HEIGHT - el.clientHeight;
    }
  },
);
</script>

<template>
  <div class="wrap">
    <div class="head">
      <button
        v-for="col in COLUMNS"
        :key="col.key"
        type="button"
        class="th"
        :class="{ active: sortKey === col.key }"
        @click="emit('sort', col.key)"
      >
        {{ col.label }}<span class="mark">{{ sortKey === col.key ? (sortDir === 1 ? " ▲" : " ▼") : "" }}</span>
      </button>
    </div>

    <div
      ref="scroller"
      class="body"
      @scroll="onScroll"
      @contextmenu.prevent="emit('menu', null, $event.clientX, $event.clientY)"
    >
      <div :style="{ height: padTop + 'px' }"></div>

      <div
        v-for="item in windowRows"
        :key="item.row.key"
        class="row"
        :class="{ sel: selected.includes(item.row.key), cur: item.index === cursor }"
        :style="{ height: ROW_HEIGHT + 'px' }"
        @click="emit('select', item.row, $event)"
        @dblclick="emit('open', item.row)"
        @contextmenu.prevent.stop="emit('menu', item.row, $event.clientX, $event.clientY)"
      >
        <span class="c-name">
          <span class="ico">{{ item.row.kind === "folder" ? "📁" : "📄" }}</span>{{ item.row.name }}
        </span>
        <span class="c-size">{{ item.row.kind === "folder" ? "" : formatBytes(item.row.size) }}</span>
        <span class="c-mod">{{ formatDate(item.row.lastModified) }}</span>
        <span class="c-class">{{ item.row.storageClass }}</span>
      </div>

      <div :style="{ height: padBottom + 'px' }"></div>

      <p v-if="loading" class="note">Loading…</p>
      <p v-else-if="!rows.length" class="note">Nothing here.</p>
    </div>
  </div>
</template>

<style scoped>
.wrap {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
}
.head,
.row {
  display: grid;
  grid-template-columns: var(--cols);
  align-items: center;
  gap: 8px;
  padding: 0 10px;
}
.head {
  border-bottom: 1px solid #34373f;
  background: #202227;
}
.th {
  padding: 6px 0;
  border: 0;
  background: none;
  color: #9aa0aa;
  font: inherit;
  font-size: 12px;
  text-align: left;
  cursor: pointer;
}
.th.active {
  color: #e6e8ec;
}
.mark {
  color: #7aa2f7;
}
.body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.row {
  color: #d7dae0;
  font-size: 13px;
  white-space: nowrap;
  cursor: default;
  user-select: none;
}
.row:hover {
  background: #262930;
}
.row.sel {
  background: #2c3a55;
}
.row.cur {
  outline: 1px solid #4a6ea9;
  outline-offset: -1px;
}
.c-name {
  display: flex;
  align-items: center;
  gap: 6px;
  overflow: hidden;
  text-overflow: ellipsis;
}
.ico {
  flex: none;
}
.c-size,
.c-mod,
.c-class {
  overflow: hidden;
  color: #9aa0aa;
  font-size: 12px;
  text-overflow: ellipsis;
}
.note {
  margin: 10px;
  color: #8a8f99;
  font-size: 13px;
}
</style>
