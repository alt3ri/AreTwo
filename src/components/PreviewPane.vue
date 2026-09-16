<script setup lang="ts">
import { computed, ref, watch } from "vue";
import * as api from "../api";
import type { ObjectInfo } from "../types";
import { formatBytes, previewKind, type Row } from "../rows";

const { profileId, bucket, row } = defineProps<{
  profileId: string;
  bucket: string;
  row: Row | null;
}>();

const emit = defineEmits<{ close: []; download: [row: Row] }>();

const info = ref<ObjectInfo | null>(null);
const text = ref("");
const image = ref("");
const truncated = ref(false);
const error = ref("");
const loading = ref(false);

const metadata = computed(() => Object.entries(info.value?.metadata ?? {}));

async function load() {
  info.value = null;
  text.value = "";
  image.value = "";
  truncated.value = false;
  error.value = "";

  const target = row;
  if (!target || target.kind !== "file" || !profileId || !bucket) return;

  loading.value = true;
  try {
    info.value = await api.headObject(profileId, bucket, target.key);
    const kind = previewKind(target.name);
    if (kind === "text") {
      const preview = await api.readText(profileId, bucket, target.key);
      text.value = preview.text;
      truncated.value = preview.truncated;
    } else if (kind === "image") {
      image.value = await api.readImage(profileId, bucket, target.key);
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

watch(() => `${profileId}|${bucket}|${row?.key ?? ""}`, load, { immediate: true });
</script>

<template>
  <aside class="pane">
    <header class="phead">
      <strong class="pname">{{ row ? row.name : "Preview" }}</strong>
      <button type="button" class="x" title="Close preview" @click="emit('close')">✕</button>
    </header>

    <p v-if="!row || row.kind !== 'file'" class="hint">Select a file to preview it.</p>
    <p v-else-if="loading" class="hint">Loading…</p>
    <p v-else-if="error" class="hint err">{{ error }}</p>

    <div v-else class="pbody">
      <dl class="meta">
        <dt>Size</dt>
        <dd>{{ formatBytes(info?.size ?? row.size) }}</dd>
        <dt>Type</dt>
        <dd>{{ info?.contentType || "—" }}</dd>
        <dt>Modified</dt>
        <dd>{{ info?.lastModified || "—" }}</dd>
        <dt>ETag</dt>
        <dd class="mono">{{ info?.etag || "—" }}</dd>
        <dt>Class</dt>
        <dd>{{ info?.storageClass || "—" }}</dd>
        <template v-for="[key, value] in metadata" :key="key">
          <dt>{{ key }}</dt>
          <dd class="mono">{{ value }}</dd>
        </template>
      </dl>

      <img v-if="image" :src="image" alt="" class="img" />
      <pre v-else-if="text" class="text">{{ text }}</pre>
      <p v-else class="hint">No inline preview for this type.</p>
      <p v-if="truncated" class="hint">Only the start of this file is shown.</p>

      <button type="button" class="download" @click="emit('download', row)">Download…</button>
    </div>
  </aside>
</template>

<style scoped>
.pane {
  display: flex;
  flex: none;
  flex-direction: column;
  width: 340px;
  min-height: 0;
  border-left: 1px solid #34373f;
  background: #1b1d21;
}
.phead {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border-bottom: 1px solid #34373f;
}
.pname {
  flex: 1;
  overflow: hidden;
  font-size: 13px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.x,
.download {
  border: 1px solid #3a3d45;
  border-radius: 4px;
  background: #23252b;
  color: #dfe1e6;
  cursor: pointer;
}
.x {
  padding: 1px 6px;
}
.download {
  margin-top: 10px;
  padding: 6px 10px;
  font: inherit;
  font-size: 13px;
}
.pbody {
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 10px;
  overflow: auto;
}
.meta {
  display: grid;
  grid-template-columns: 78px minmax(0, 1fr);
  gap: 2px 8px;
  margin: 0 0 10px;
  font-size: 12px;
}
.meta dt {
  color: #8a8f99;
}
.meta dd {
  margin: 0;
  overflow-wrap: anywhere;
}
.mono {
  font-family: ui-monospace, Consolas, monospace;
}
.img {
  max-width: 100%;
  border-radius: 4px;
  background: #0004;
}
.text {
  max-height: 46vh;
  margin: 0;
  overflow: auto;
  font-family: ui-monospace, Consolas, monospace;
  font-size: 12px;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.hint {
  margin: 0;
  color: #8a8f99;
  font-size: 13px;
}
.err {
  color: #ff8a8a;
}
</style>
