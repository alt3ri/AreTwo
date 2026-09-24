<script setup lang="ts">
import UiIcon from "./UiIcon.vue";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import {
  ROW_HEIGHT,
  extensionOf,
  formatBytes,
  formatDate,
  typeLabel,
  type MenuItem,
  type Row,
  type SortKey,
} from "../rows";
import { peek, request } from "../stores/folderStats";
import ContextMenu from "./ContextMenu.vue";

const props = defineProps<{
  rows: Row[];
  selected: string[];
  cursor: number;
  sortKey: SortKey;
  sortDir: 1 | -1;
  loading: boolean;
  hasMore: boolean;
  profileId: string;
  bucket: string;
}>();

const emit = defineEmits<{
  select: [row: Row, event: MouseEvent];
  open: [row: Row];
  menu: [row: Row | null, x: number, y: number];
  sort: [key: SortKey];
  more: [];
}>();

// ---------- columns (§4) ----------

type ColKey =
  | "name"
  | "type"
  | "lastModified"
  | "folders"
  | "files"
  | "size"
  | "etag"
  | "storageClass"
  | "extension"
  | "fullKey";

interface Col {
  key: ColKey;
  label: string;
  width: number;
  /** Sortable through rows.ts. */
  sort?: SortKey;
  /** Hidden until the header menu turns it on. */
  optional?: boolean;
}

const COLUMNS: Col[] = [
  { key: "name", label: "Name", width: 320, sort: "name" },
  { key: "type", label: "Type", width: 132, sort: "type" },
  { key: "lastModified", label: "Date Modified", width: 150, sort: "lastModified" },
  { key: "folders", label: "Folders", width: 78 },
  { key: "files", label: "Files", width: 78 },
  { key: "size", label: "Size", width: 96, sort: "size" },
  { key: "etag", label: "ETag", width: 150, optional: true },
  { key: "storageClass", label: "Storage Class", width: 110, optional: true, sort: "storageClass" },
  { key: "extension", label: "Extension", width: 90, optional: true },
  { key: "fullKey", label: "Full Key", width: 260, optional: true },
];

// ponytail: one global column layout, not per-folder; split the storage key by
// prefix when anybody actually wants per-folder column sets.
const STORE_KEY = "r2explorer.columns.v1";
const widths = ref<Record<string, number>>({});
const hidden = ref<string[]>(COLUMNS.filter((c) => c.optional).map((c) => c.key));
const colMenu = ref<{ x: number; y: number } | null>(null);

const visibleCols = computed(() => COLUMNS.filter((c) => !hidden.value.includes(c.key)));
const widthOf = (col: Col) => widths.value[col.key] ?? col.width;
const gridTemplate = computed(() =>
  visibleCols.value
    .map((c) => (c.key === "name" ? "minmax(200px, 1fr)" : `${widthOf(c)}px`))
    .join(" "),
);

function persist() {
  try {
    localStorage.setItem(STORE_KEY, JSON.stringify({ widths: widths.value, hidden: hidden.value }));
  } catch {
    /* private mode / quota: the layout just does not stick */
  }
}

const colMenuItems = computed<MenuItem[]>(() =>
  COLUMNS.filter((c) => c.optional).map((c) => ({
    id: c.key,
    label: `${hidden.value.includes(c.key) ? "○" : "●"} ${c.label}`,
  })),
);

function pickColumn(item: MenuItem) {
  colMenu.value = null;
  hidden.value = hidden.value.includes(item.id)
    ? hidden.value.filter((k) => k !== item.id)
    : [...hidden.value, item.id];
  persist();
}

function startResize(col: Col, event: MouseEvent) {
  const from = event.clientX;
  const base = widthOf(col);
  const move = (e: MouseEvent) => {
    widths.value = { ...widths.value, [col.key]: Math.max(60, base + (e.clientX - from)) };
  };
  const stop = () => {
    window.removeEventListener("mousemove", move);
    window.removeEventListener("mouseup", stop);
    persist();
  };
  window.addEventListener("mousemove", move);
  window.addEventListener("mouseup", stop);
}

// ---------- virtual window ----------

const OVERSCAN = 6;
const scroller = ref<HTMLDivElement | null>(null);
const scrollTop = ref(0);
const headerScroller = ref<HTMLDivElement | null>(null);
const viewport = ref(480);
let observer: ResizeObserver | null = null;

const start = computed(() => Math.max(0, Math.floor(scrollTop.value / ROW_HEIGHT) - OVERSCAN));
const end = computed(() =>
  Math.min(props.rows.length, Math.ceil((scrollTop.value + viewport.value) / ROW_HEIGHT) + OVERSCAN),
);
const windowRows = computed(() =>
  props.rows.slice(start.value, end.value).map((row, i) => ({ row, index: start.value + i })),
);
const padTop = computed(() => start.value * ROW_HEIGHT);
const padBottom = computed(() => Math.max(0, (props.rows.length - end.value) * ROW_HEIGHT));

function measure() {
  const el = scroller.value;
  if (!el) return;
  viewport.value = el.clientHeight;
  scrollTop.value = el.scrollTop;
}

function onScroll() {
  const el = scroller.value;
  if (!el) return;
  if (headerScroller.value) headerScroller.value.scrollLeft = el.scrollLeft;
  scrollTop.value = el.scrollTop;
  if (props.hasMore && !props.loading && el.scrollTop + el.clientHeight >= el.scrollHeight - 160) {
    emit("more");
  }
}

onMounted(() => {
  try {
    const saved = JSON.parse(localStorage.getItem(STORE_KEY) ?? "");
    if (saved?.widths) widths.value = saved.widths;
    if (Array.isArray(saved?.hidden)) hidden.value = saved.hidden;
  } catch {
    /* no saved layout yet */
  }
  measure();
  observer = new ResizeObserver(measure);
  if (scroller.value) observer.observe(scroller.value);
});
onUnmounted(() => observer?.disconnect());

watch(
  () => props.cursor,
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

// ---------- folder statistics ----------

/** Only folders that live in a real bucket have anything to scan. */
function scannable(row: Row): boolean {
  return row.kind === "folder" && !!props.bucket && !row.bucketName;
}

function statsFor(row: Row) {
  return scannable(row) ? peek(props.profileId, props.bucket, row.prefix) : undefined;
}

function folderCell(row: Row, field: "folders" | "files"): string {
  if (row.kind !== "folder") return "";
  const entry = statsFor(row);
  if (!entry) return "";
  if (entry.state === "loading") return "…";
  if (entry.state === "error") return "!";
  return String(entry[field]);
}

function sizeCell(row: Row): string {
  if (row.kind === "file") return formatBytes(row.size);
  const entry = statsFor(row);
  if (!entry) return "";
  if (entry.state === "loading") return "…";
  if (entry.state === "error") return "!";
  return formatBytes(entry.size);
}

function cellText(row: Row, col: ColKey): string {
  switch (col) {
    case "name":
      return row.name;
    case "type":
      return typeLabel(row.name, row.kind);
    case "lastModified":
      return row.kind === "file" ? formatDate(row.lastModified) : "";
    case "folders":
      return folderCell(row, "folders");
    case "files":
      return folderCell(row, "files");
    case "size":
      return sizeCell(row);
    case "etag":
      return row.kind === "file" ? (row.etag ?? "") : "";
    case "storageClass":
      return row.kind === "file" ? row.storageClass : "";
    case "extension":
      return row.kind === "file" ? extensionOf(row.name) : "";
    case "fullKey":
      return row.key;
    default:
      return "";
  }
}

/** Only rows on screen ask for a scan; scrolling pulls the next window in. */
watch(
  windowRows,
  (list) => {
    if (!props.bucket) return;
    for (const { row } of list) {
      if (scannable(row)) request(props.profileId, props.bucket, row.prefix);
    }
  },
  { immediate: true },
);
</script>

<template>
  <div class="wrap">
    <div ref="headerScroller" class="head" :style="{ gridTemplateColumns: gridTemplate }">
      <div
        v-for="col in visibleCols"
        :key="col.key"
        class="th"
        @contextmenu.prevent="colMenu = { x: $event.clientX, y: $event.clientY }"
      >
        <button
          type="button"
          class="thlabel"
          :class="{ active: col.sort && sortKey === col.sort }"
          :disabled="!col.sort"
          @click="col.sort && emit('sort', col.sort)"
        >
          {{ col.label }}
          <span class="mark">{{
            col.sort && sortKey === col.sort ? (sortDir === 1 ? "▲" : "▼") : ""
          }}</span>
        </button>
        <span class="grip" title="Drag to resize" @mousedown.prevent="startResize(col, $event)"></span>
      </div>
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
        :style="{ height: ROW_HEIGHT + 'px', gridTemplateColumns: gridTemplate }"
        @click="emit('select', item.row, $event)"
        @dblclick="emit('open', item.row)"
        @contextmenu.prevent.stop="emit('menu', item.row, $event.clientX, $event.clientY)"
      >
        <span
          v-for="col in visibleCols"
          :key="col.key"
          class="cell"
          :class="[
            `c-${col.key}`,
            { num: col.key === 'size' || col.key === 'folders' || col.key === 'files' },
          ]"
          :title="cellText(item.row, col.key)"
        >
          <template v-if="col.key === 'name'">
            <UiIcon class="ico" :class="{ folder: item.row.kind === 'folder' }" :name="item.row.bucketName ? 'bucket' : item.row.kind === 'folder' ? 'folder' : 'file'" />
            <span class="nm">{{ item.row.name }}</span>
          </template>
          <template v-else>{{ cellText(item.row, col.key) }}</template>
        </span>
      </div>

      <div :style="{ height: padBottom + 'px' }"></div>

      <p v-if="loading" class="note">Loading…</p>
      <p v-else-if="!rows.length" class="note">No items to show.</p>
    </div>

    <ContextMenu
      v-if="colMenu"
      :x="colMenu.x"
      :y="colMenu.y"
      :items="colMenuItems"
      @pick="pickColumn"
      @close="colMenu = null"
    />
  </div>
</template>

<style scoped>
.wrap {
  position: relative;
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
}
.head,
.row {
  display: grid;
  align-items: center;
  gap: 8px;
  padding: 0 10px;
}
.head {
  border-bottom: 1px solid var(--color-line);
  background: var(--color-panel);
}
.th {
  position: relative;
  display: flex;
  align-items: center;
  min-width: 0;
}
.thlabel {
  flex: 1;
  overflow: hidden;
  padding: 6px 0;
  border: 0;
  background: none;
  color: var(--color-text-dim);
  font: inherit;
  font-size: 12px;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
}
.thlabel:disabled {
  cursor: default;
}
.thlabel.active {
  color: var(--color-text);
}
.mark {
  color: var(--color-focus);
}
.grip {
  position: absolute;
  top: 0;
  right: -5px;
  z-index: 2;
  width: 8px;
  height: 100%;
  cursor: col-resize;
}
.grip:hover {
  background: var(--color-accent);
}
.body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.row {
  color: var(--color-text);
  font-size: 13px;
  white-space: nowrap;
  cursor: default;
  user-select: none;
}
.row:hover {
  background: var(--color-raised);
}
.row.sel {
  background: var(--color-accent-soft);
}
.row.cur {
  outline: 1px solid var(--color-accent);
  outline-offset: -1px;
}
.cell {
  display: flex;
  align-items: center;
  gap: 6px;
  overflow: hidden;
  color: var(--color-text-dim);
  font-size: 12px;
  text-overflow: ellipsis;
}
.cell.num {
  justify-content: flex-end;
  font-variant-numeric: tabular-nums;
}
.c-name {
  color: var(--color-text);
  font-size: 13px;
}
.nm {
  overflow: hidden;
  text-overflow: ellipsis;
}
.ico {
  flex: none;
}
.note {
  margin: 10px;
  color: var(--color-text-faint);
  font-size: 13px;
}

.wrap { min-width: 0; }
.head { flex: none; min-height: 36px; overflow: hidden; }
.head, .row { padding-inline: var(--space-5); }
.body { padding-top: var(--space-2); }
.row.cur { outline-color: var(--color-line-strong); }
.row.sel.cur { outline-color: var(--color-accent); }
.ico { color: var(--color-text-faint); margin-right: var(--space-2); }
.ico.folder { color: var(--color-folder); }
.mark { font-size: 9px; color: var(--color-text-dim); }
.note { margin: var(--space-7) var(--space-5); text-align: center; }

</style>
