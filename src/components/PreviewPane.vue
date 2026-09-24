<script setup lang="ts">
// Hallmark · component: preview pane · genre: utility/desktop · theme: existing dark shell
// states: default · hover · focus-visible · active · disabled · loading · error · success
import { computed, ref, watch } from "vue";
import * as api from "../api";
import type { ObjectInfo } from "../types";
import { formatBytes, formatDate, previewKind, typeLabel, type Row } from "../rows";
import { peek, request } from "../stores/folderStats";

const { profileId, bucket, row } = defineProps<{
  profileId: string;
  bucket: string;
  row: Row | null;
}>();

const emit = defineEmits<{ close: []; download: [row: Row] }>();

const info = ref<ObjectInfo | null>(null);
const raw = ref("");
const pretty = ref(false);
const image = ref("");
const truncated = ref(false);
const error = ref("");
const loading = ref(false);
const copied = ref(false);
/** Long files wrap by default: a 2 MB single-line log is unreadable otherwise. */
const wrap = ref(true);
const fit = ref(true);
const PANE_KEY = "r2explorer.previewWidth";
const width = ref(Number(localStorage.getItem(PANE_KEY)) || 460);

const MAX_LINES = 4000;

const stats = computed(() =>
  row?.kind === "folder" && !row.bucketName && profileId && bucket
    ? peek(profileId, bucket, row.prefix)
    : undefined,
);

/** "…" while scanning, "!" when the scan failed, otherwise the number. */
function folderCell(field: "folders" | "files" | "size"): string {
  const entry = stats.value;
  if (!entry || entry.state === "loading") return "…";
  if (entry.state === "error") return "!";
  return field === "size" ? formatBytes(entry.size) : String(entry[field]);
}

function folderSummary(): string {
  const entry = stats.value;
  if (!entry || entry.state === "loading") return "Scanning the prefix…";
  if (entry.state === "error") return "Folder scan failed.";
  return `${entry.folders} folders, ${entry.files} files underneath.`;
}

const text = computed(() => (pretty.value ? prettyJson(raw.value) : raw.value));
const lines = computed(() => (text.value ? text.value.split("\n") : []));
const gutters = computed(() => lines.value.length <= MAX_LINES);
const language = computed(() => {
  if (row?.kind !== "file") return "";
  const kind = previewKind(row.name);
  if (kind === "image") return "Image";
  if (kind === "text") return typeLabel(row.name);
  return "File";
});
const meta = computed(() => Object.entries(info.value?.metadata ?? {}));
const kind = computed(() => (row ? previewKind(row.name) : "none"));
const tooBigForCode = computed(() => lines.value.length > MAX_LINES);

function prettyJson(source: string): string {
  try {
    return JSON.stringify(JSON.parse(source), null, 2);
  } catch {
    return source;
  }
}

function startResize(event: MouseEvent) {
  const from = event.clientX;
  const base = width.value;
  const move = (e: MouseEvent) => {
    width.value = Math.max(260, Math.min(900, base + (from - e.clientX)));
  };
  const stop = () => {
    window.removeEventListener("mousemove", move);
    window.removeEventListener("mouseup", stop);
    try {
      localStorage.setItem(PANE_KEY, String(width.value));
    } catch {
      /* private mode: the width just does not stick */
    }
  };
  window.addEventListener("mousemove", move);
  window.addEventListener("mouseup", stop);
}

async function copyText() {
  try {
    await navigator.clipboard.writeText(text.value);
    copied.value = true;
    setTimeout(() => (copied.value = false), 1500);
  } catch (e) {
    error.value = `clipboard refused: ${String(e)}`;
  }
}

async function load() {
  info.value = null;
  raw.value = "";
  pretty.value = false;
  image.value = "";
  truncated.value = false;
  error.value = "";
  copied.value = false;

  const target = row;
  if (!target || !profileId || !bucket) return;
  if (target.kind === "folder" && target.bucketName) return; // bucket row: nothing to scan

  loading.value = true;
  try {
    if (target.kind === "folder") {
      request(profileId, bucket, target.prefix);
      return;
    }
    info.value = await api.headObject(profileId, bucket, target.key);
    if (kind.value === "text") {
      const preview = await api.readText(profileId, bucket, target.key);
      raw.value = preview.text;
      truncated.value = preview.truncated;
      const type = info.value.contentType ?? "";
      const looksJson =
        type.includes("json") || /\.(json|jsonc|json5|webmanifest|ipynb)$/i.test(target.name);
      if (looksJson) pretty.value = true;
    } else if (kind.value === "image") {
      image.value = await api.readImage(profileId, bucket, target.key);
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

watch(() => `${profileId}|${bucket}|${row?.key ?? ""}`, load, { immediate: true });

defineExpose({ load });
</script>

<template>
  <aside class="pane" :style="{ width: width + 'px' }">
    <div class="grip" title="Drag to resize" @mousedown.prevent="startResize"></div>

    <header class="head">
      <strong class="name" :title="row?.key ?? ''">{{ row?.name ?? "Preview" }}</strong>
      <span v-if="row" class="chip">{{ row.kind === "folder" ? "Prefix" : language }}</span>
      <button
        v-if="kind === 'text'"
        type="button"
        class="icon"
        :class="{ on: wrap }"
        :disabled="tooBigForCode"
        title="Toggle word wrap"
        @click="wrap = !wrap"
      >
        ↵
      </button>
      <button
        v-if="kind === 'text' && !tooBigForCode"
        type="button"
        class="icon"
        :class="{ on: pretty }"
        title="Pretty-print (JSON, JSONC)"
        @click="pretty = !pretty"
      >
        { }
      </button>
      <button type="button" class="icon" aria-label="Close preview" title="Close preview" @click="emit('close')">✕</button>
    </header>

    <p v-if="!row" class="hint">Select a file or folder to preview it.</p>
    <p v-else-if="loading" class="hint">Loading…</p>
    <p v-else-if="error" class="hint err">{{ error }}</p>
    <p v-else-if="stats?.state === 'error'" class="hint err">{{ stats.error }}</p>

    <div v-else class="body">
      <!-- code / text -->
      <!-- `numbered`, not `gutter`: the gutter rule is right-aligned and must not
           be inherited by the source block. -->
      <div v-if="kind === 'text' && raw" class="code" :class="{ wrap, numbered: gutters }">
        <ol v-if="gutters" class="gutter">
          <li v-for="(_, i) in lines" :key="i">{{ i + 1 }}</li>
        </ol>
        <pre class="source">{{ text }}</pre>
      </div>

      <!-- image -->
      <div v-else-if="image" class="shot" :class="{ fit }">
        <img :src="image" alt="" />
        <button type="button" class="icon overlay" :class="{ on: fit }" @click="fit = !fit">
          {{ fit ? "Fit" : "100%" }}
        </button>
      </div>

      <!-- folder -->
      <dl v-else-if="row.kind === 'folder'" class="facts">
        <dt>Folders</dt>
        <dd>{{ folderCell("folders") }}</dd>
        <dt>Files</dt>
        <dd>{{ folderCell("files") }}</dd>
        <dt>Size</dt>
        <dd>{{ folderCell("size") }}</dd>
        <dt>Prefix</dt>
        <dd class="mono">{{ row.prefix }}</dd>
        <dt>Scanned</dt>
        <dd>{{ stats?.scannedAt ? formatDate(stats.scannedAt) : "—" }}</dd>
      </dl>

      <p v-else class="hint">No inline preview for this type.</p>

      <p v-if="row.kind === 'folder'" class="note">{{ folderSummary() }}</p>
      <p v-if="tooBigForCode" class="note">
        {{ lines.length }} lines — line numbers and wrap are off for files this long.
      </p>
    </div>

    <footer v-if="row && !loading && !error" class="foot">
      <span class="stat">
        <template v-if="kind === 'text' && raw">{{ lines.length }} lines · </template>
        {{ formatBytes(info?.size ?? row.size) }}
        <template v-if="info?.contentType"> · {{ info.contentType }}</template>
        <template v-if="truncated"> · first 2 MB only</template>
      </span>
      <button v-if="kind === 'text' && raw" type="button" class="act" @click="copyText">
        {{ copied ? "Copied" : "Copy" }}
      </button>
      <button v-if="row.kind === 'file'" type="button" class="act" @click="emit('download', row)">
        Download…
      </button>
    </footer>

    <details v-if="row?.kind === 'file' && info" class="more">
      <summary>Details</summary>
      <dl class="facts">
        <dt>Key</dt>
        <dd class="mono">{{ row.key }}</dd>
        <dt>Type</dt>
        <dd>{{ info.contentType ?? "—" }}</dd>
        <dt>Modified</dt>
        <dd>{{ info.lastModified ? formatDate(info.lastModified) : "—" }}</dd>
        <dt>ETag</dt>
        <dd class="mono">{{ info.etag ?? "—" }}</dd>
        <dt>Class</dt>
        <dd>{{ info.storageClass ?? "—" }}</dd>
        <dt v-if="info.cacheControl">Cache</dt>
        <dd v-if="info.cacheControl" class="mono">{{ info.cacheControl }}</dd>
        <template v-for="[k, v] in meta" :key="k">
          <dt>{{ k }}</dt>
          <dd class="mono">{{ v }}</dd>
        </template>
      </dl>
    </details>
  </aside>
</template>

<style scoped>
/* Hallmark · component: preview pane · genre: utility/desktop · theme: existing dark shell
 * states: default · hover · focus · active · disabled · loading · error · success
 * contrast: pass (tokens in :root)
 */
.pane {
  position: relative;
  display: flex;
  flex: none;
  flex-direction: column;
  min-height: 0;
  border-left: 1px solid var(--color-line);
  background: var(--color-panel);
  color: var(--color-text);
}
.grip {
  position: absolute;
  top: 0;
  left: -3px;
  z-index: 3;
  width: 6px;
  height: 100%;
  cursor: col-resize;
}
.grip:hover {
  background: var(--color-accent);
}

.head {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-3);
  padding: 6px var(--space-3);
  border-bottom: 1px solid var(--color-line);
}
.name {
  overflow: hidden;
  font-size: var(--text-m);
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.chip {
  flex: none;
  padding: 1px 6px;
  border: 1px solid var(--color-line);
  border-radius: var(--radius-s);
  color: var(--color-text-faint);
  font-size: var(--text-xs);
}
.icon {
  flex: none;
  padding: 2px 7px;
  border: 1px solid var(--color-line-strong);
  border-radius: var(--radius-s);
  background: var(--color-raised);
  color: var(--color-text);
  font: inherit;
  font-size: var(--text-s);
  cursor: pointer;
  transition: background var(--dur-fast) var(--ease-out);
}
.icon:first-of-type {
  margin-left: auto;
}
.icon:hover:not(:disabled) {
  background: var(--color-line);
}
.icon:focus-visible,
.act:focus-visible,
.more summary:focus-visible {
  outline: 2px solid var(--color-focus);
  outline-offset: 1px;
}
.icon.on {
  border-color: var(--color-accent);
  background: var(--color-accent-soft);
}
.icon:disabled {
  color: var(--color-disabled);
  cursor: default;
}

.body {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  overflow: auto;
}

/* --- code --- */
.code {
  display: grid;
  flex: 1;
  grid-template-columns: minmax(0, 1fr);
  min-height: 0;
  overflow: auto;
  background: var(--color-field);
}
.code.numbered {
  grid-template-columns: auto minmax(0, 1fr);
}
ol.gutter {
  margin: 0;
  padding: 8px 6px 8px 10px;
  border-right: 1px solid var(--color-line);
  color: var(--color-text-faint);
  font-family: var(--font-mono);
  font-size: var(--text-s);
  line-height: 18px;
  text-align: right;
  user-select: none;
  list-style: none;
}
.source {
  margin: 0;
  padding: 8px 10px;
  color: var(--color-text);
  font-family: var(--font-mono);
  font-size: var(--text-s);
  line-height: 18px;
  tab-size: 2;
  white-space: pre;
}
.code.wrap .source {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

/* --- image --- */
.shot {
  position: relative;
  display: grid;
  flex: 1;
  place-items: center;
  min-height: 0;
  padding: var(--space-3);
  overflow: auto;
  background: repeating-conic-gradient(var(--color-bg) 0% 25%, var(--color-raised) 0% 50%) 0 0/16px 16px;
}
.shot img {
  max-width: 100%;
  max-height: 100%;
}
.shot:not(.fit) img {
  max-width: none;
  max-height: none;
}
.overlay {
  position: absolute;
  right: var(--space-3);
  bottom: var(--space-3);
  opacity: 0.9;
}

/* --- facts --- */
.facts {
  display: grid;
  grid-template-columns: 84px minmax(0, 1fr);
  gap: var(--space-1) var(--space-3);
  margin: 0;
  padding: var(--space-4);
  font-size: var(--text-s);
}
.facts dt {
  color: var(--color-text-faint);
}
.facts dd {
  margin: 0;
  overflow-wrap: anywhere;
}
.mono {
  font-family: var(--font-mono);
}

.hint,
.note {
  margin: 0;
  padding: var(--space-3) var(--space-4);
  color: var(--color-text-faint);
  font-size: var(--text-m);
}
.note {
  padding-top: 0;
  font-size: var(--text-s);
}
.err {
  color: var(--color-danger);
}

.foot {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-3);
  padding: 5px var(--space-3);
  border-top: 1px solid var(--color-line);
  color: var(--color-text-faint);
  font-size: var(--text-xs);
}
.stat {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.act {
  flex: none;
  padding: 3px 9px;
  border: 1px solid var(--color-line-strong);
  border-radius: var(--radius-s);
  background: var(--color-raised);
  color: var(--color-text);
  font: inherit;
  font-size: var(--text-s);
  cursor: pointer;
}
.act:hover {
  background: var(--color-line);
}
.act:active {
  transform: translateY(1px);
}

.more {
  flex: none;
  border-top: 1px solid var(--color-line);
}
.more summary {
  padding: 5px var(--space-4);
  color: var(--color-text-faint);
  font-size: var(--text-s);
  cursor: pointer;
}
.more summary:hover {
  background: var(--color-raised);
}
.more .facts {
  padding-top: var(--space-2);
  max-height: 40vh;
  overflow: auto;
}

.pane { max-width: 55%; }
.head { min-height: 40px; padding-inline: var(--space-4); }
.hint { margin: var(--space-6); color: var(--color-text-dim); line-height: 1.6; }
.hint.err { color: var(--color-danger); }
@media (max-width: 900px) {
  .pane { position: absolute; inset: 0 0 0 auto; z-index: 10; max-width: 100%; box-shadow: -8px 0 24px var(--color-backdrop); }
}

</style>
