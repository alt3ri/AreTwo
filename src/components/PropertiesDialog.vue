<script setup lang="ts">
import { computed, ref, watch } from "vue";
import * as api from "../api";
import type { FolderStats, ObjectInfo } from "../types";
import { formatBytes, formatDate, typeLabel, type Row } from "../rows";

const { profileId, bucket, row, open } = defineProps<{
  profileId: string;
  bucket: string;
  row: Row | null;
  open: boolean;
}>();

const emit = defineEmits<{ close: [] }>();

const dialog = ref<HTMLDialogElement | null>(null);
const info = ref<ObjectInfo | null>(null);
const stats = ref<FolderStats | null>(null);
const error = ref("");
const loading = ref(false);
const copied = ref("");

const isFolder = computed(() => row?.kind === "folder");
const metadata = computed(() => Object.entries(info.value?.metadata ?? {}));
const r2Uri = computed(() => `r2://${profileId}/${bucket}/${row?.key ?? ""}`);

async function load() {
  info.value = null;
  stats.value = null;
  error.value = "";
  copied.value = "";
  const target = row;
  if (!target || !profileId || !bucket) return;
  loading.value = true;
  try {
    if (target.kind === "folder") {
      stats.value = await api.folderStats(profileId, bucket, target.prefix);
    } else {
      info.value = await api.headObject(profileId, bucket, target.key);
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function copy(value: string, what: string) {
  try {
    await navigator.clipboard.writeText(value);
    copied.value = `${what} copied`;
  } catch (e) {
    error.value = `clipboard refused: ${String(e)}`;
  }
}

watch(() => `${profileId}|${bucket}|${row?.key ?? ""}`, load, { immediate: true });

watch(
  () => open,
  (wanted) => {
    const el = dialog.value;
    if (!el) return;
    if (wanted && !el.open) el.showModal();
    else if (!wanted && el.open) el.close();
  },
);

function onClose() {
  emit("close");
}
</script>

<template>
  <dialog ref="dialog" class="sheet" @close="onClose" @cancel.prevent="emit('close')">
    <header class="head">
      <strong>{{ row?.name ?? "Properties" }}</strong>
      <span class="kind">{{ row ? typeLabel(row.name, row.kind) : "" }}</span>
      <button type="button" class="x" @click="emit('close')">✕</button>
    </header>

    <p v-if="loading" class="hint">Loading…</p>
    <p v-else-if="error" class="err">{{ error }}</p>

    <div v-else class="body">
      <template v-if="isFolder">
        <dl class="meta">
          <dt>Prefix</dt>
          <dd class="mono">{{ row?.prefix }}</dd>
          <dt>Folders</dt>
          <dd>{{ stats?.folders ?? "—" }}</dd>
          <dt>Files</dt>
          <dd>{{ stats?.files ?? "—" }}</dd>
          <dt>Total size</dt>
          <dd>{{ stats ? formatBytes(stats.size) : "—" }}</dd>
          <dt>Last scanned</dt>
          <dd>{{ stats?.scannedAt ? formatDate(stats.scannedAt) : "—" }}</dd>
        </dl>
      </template>

      <template v-else>
        <dl class="meta">
          <dt>Name</dt>
          <dd>{{ row?.name }}</dd>
          <dt>Full key</dt>
          <dd class="mono">{{ row?.key }}</dd>
          <dt>Bucket</dt>
          <dd>{{ bucket }}</dd>
          <dt>Type</dt>
          <dd>{{ info?.contentType ?? "—" }}</dd>
          <dt>Size</dt>
          <dd>{{ info ? formatBytes(info.size) : "—" }}</dd>
          <dt>Last modified</dt>
          <dd>{{ info?.lastModified ? formatDate(info.lastModified) : "—" }}</dd>
          <dt>ETag</dt>
          <dd class="mono">{{ info?.etag ?? row?.etag ?? "—" }}</dd>
          <dt>Storage class</dt>
          <dd>{{ info?.storageClass ?? "—" }}</dd>
        </dl>

        <h4>HTTP metadata</h4>
        <dl class="meta">
          <dt>Content-Type</dt>
          <dd>{{ info?.contentType ?? "—" }}</dd>
          <dt>Cache-Control</dt>
          <dd class="mono">{{ info?.cacheControl ?? "—" }}</dd>
        </dl>

        <template v-if="metadata.length">
          <h4>Custom metadata</h4>
          <dl class="meta">
            <template v-for="[key, value] in metadata" :key="key">
              <dt>{{ key }}</dt>
              <dd class="mono">{{ value }}</dd>
            </template>
          </dl>
        </template>
      </template>

      <p v-if="copied" class="ok">{{ copied }}</p>
    </div>

    <footer class="actions">
      <button type="button" @click="row && copy(row.key, 'Key')">Copy key</button>
      <button type="button" @click="copy(r2Uri, 'R2 URI')">Copy R2 URI</button>
      <span class="grow"></span>
      <button v-if="isFolder" type="button" :disabled="loading" @click="load">Refresh</button>
      <button type="button" class="primary" @click="emit('close')">Close</button>
    </footer>
  </dialog>
</template>

<style scoped>
.sheet {
  width: 560px;
  max-width: 92vw;
  max-height: 84vh;
  padding: 0;
  border: 1px solid var(--color-line-strong);
  border-radius: 6px;
  background: var(--color-panel);
  color: var(--color-text);
}
.sheet::backdrop {
  background: var(--color-backdrop);
}
.head {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 12px;
  border-bottom: 1px solid var(--color-line);
  font-size: 13px;
}
.head strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.kind {
  color: var(--color-text-faint);
  font-size: 12px;
}
.x {
  margin-left: auto;
  padding: 1px 7px;
  border: 1px solid var(--color-line-strong);
  border-radius: 4px;
  background: var(--color-raised);
  color: var(--color-text);
  font: inherit;
  cursor: pointer;
}
.body {
  max-height: 62vh;
  padding: 12px;
  overflow: auto;
}
.meta {
  display: grid;
  grid-template-columns: 130px minmax(0, 1fr);
  gap: 3px 10px;
  margin: 0 0 12px;
  font-size: 12px;
}
.meta dt {
  color: var(--color-text-faint);
}
.meta dd {
  margin: 0;
  overflow-wrap: anywhere;
}
h4 {
  margin: 0 0 6px;
  color: var(--color-text-dim);
  font-size: 12px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.mono {
  font-family: ui-monospace, Consolas, monospace;
}
.actions {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 9px 12px;
  border-top: 1px solid var(--color-line);
}
.actions button {
  padding: 5px 10px;
  border: 1px solid var(--color-line-strong);
  border-radius: 4px;
  background: var(--color-raised);
  color: var(--color-text);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}
.actions button.primary {
  border-color: var(--color-accent);
  background: var(--color-accent-soft);
}
.actions button:disabled {
  color: var(--color-disabled);
  cursor: default;
}
.grow {
  flex: 1;
}
.hint {
  margin: 12px;
  color: var(--color-text-faint);
  font-size: 13px;
}
.err {
  margin: 12px;
  color: var(--color-danger);
  font-size: 12px;
}
.ok {
  margin: 0;
  color: var(--color-ok);
  font-size: 12px;
}
</style>
