<script setup lang="ts">
import { computed, ref, watch } from "vue";
import * as api from "../api";
import type { Row } from "../rows";

const { profileId, bucket, row, open } = defineProps<{
  profileId: string;
  bucket: string;
  row: Row | null;
  open: boolean;
}>();

const emit = defineEmits<{ close: [] }>();

const PRESETS = [
  { label: "5 minutes", seconds: 300 },
  { label: "1 hour", seconds: 3600 },
  { label: "24 hours", seconds: 86400 },
  { label: "7 days", seconds: 604800 },
];

const dialog = ref<HTMLDialogElement | null>(null);
const choice = ref(3600);
const custom = ref(3600);
const url = ref("");
const error = ref("");
const busy = ref(false);
const copied = ref("");

const usable = computed(() => !!row && row.kind === "file" && !!profileId && !!bucket);
const seconds = computed(() =>
  Math.min(604800, Math.max(1, choice.value === -1 ? custom.value : choice.value)),
);

async function create() {
  if (!usable.value || !row) return;
  error.value = "";
  copied.value = "";
  url.value = "";
  busy.value = true;
  try {
    url.value = await api.presignGet(profileId, bucket, row.key, seconds.value);
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}

async function copy() {
  try {
    await navigator.clipboard.writeText(url.value);
    copied.value = "Link copied";
  } catch (e) {
    error.value = `clipboard refused: ${String(e)}`;
  }
}

// A new row means the old link is stale.
watch(
  () => `${profileId}|${bucket}|${row?.key ?? ""}`,
  () => {
    url.value = "";
    error.value = "";
    copied.value = "";
  },
);

watch(
  () => open,
  (wanted) => {
    const el = dialog.value;
    if (!el) return;
    if (wanted && !el.open) el.showModal();
    else if (!wanted && el.open) el.close();
  },
);
</script>

<template>
  <dialog ref="dialog" class="sheet" @close="emit('close')" @cancel.prevent="emit('close')">
    <header class="head">
      <strong>Create temporary link</strong>
      <span class="kind">{{ row?.name ?? "" }}</span>
      <button type="button" class="x" @click="emit('close')">✕</button>
    </header>

    <div class="body">
      <p v-if="!usable" class="hint">Pick a single file first — prefixes cannot be presigned.</p>
      <template v-else>
        <fieldset class="expiry">
          <legend>Expires</legend>
          <label v-for="preset in PRESETS" :key="preset.seconds">
            <input v-model.number="choice" type="radio" :value="preset.seconds" />
            {{ preset.label }}
          </label>
          <label>
            <input v-model.number="choice" type="radio" :value="-1" />
            Custom
            <input
              v-model.number="custom"
              class="secs"
              type="number"
              min="1"
              max="604800"
              :disabled="choice !== -1"
            />
            seconds
          </label>
        </fieldset>

        <p class="warn">
          The link works like a password until it expires (R2 allows at most 7 days).
        </p>

        <div class="row">
          <button type="button" class="primary" :disabled="busy" @click="create">
            {{ busy ? "Creating…" : "Create link" }}
          </button>
          <span class="ttl">{{ seconds }}s</span>
        </div>

        <textarea v-if="url" class="url" readonly rows="3" @focus="($event.target as HTMLTextAreaElement).select()">
{{ url }}</textarea
        >
        <div v-if="url" class="row">
          <button type="button" @click="copy">Copy</button>
          <span v-if="copied" class="ok">{{ copied }}</span>
        </div>
      </template>

      <p v-if="error" class="err">{{ error }}</p>
    </div>
  </dialog>
</template>

<style scoped>
.sheet {
  width: 520px;
  max-width: 92vw;
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
.kind {
  overflow: hidden;
  color: var(--color-text-faint);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
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
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px;
}
.expiry {
  display: grid;
  gap: 6px;
  margin: 0;
  padding: 8px 10px;
  border: 1px solid var(--color-line);
  border-radius: 4px;
}
.expiry legend {
  padding: 0 4px;
  color: var(--color-text-faint);
  font-size: 12px;
}
.expiry label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
}
.secs {
  width: 90px;
  padding: 3px 6px;
  border: 1px solid var(--color-line-strong);
  border-radius: 4px;
  background: var(--color-field);
  color: var(--color-text);
  font: inherit;
}
.warn {
  margin: 0;
  color: var(--color-warn);
  font-size: 12px;
}
.row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.row button,
.primary {
  padding: 5px 10px;
  border: 1px solid var(--color-line-strong);
  border-radius: 4px;
  background: var(--color-raised);
  color: var(--color-text);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}
.primary {
  border-color: var(--color-accent);
  background: var(--color-accent-soft);
}
.primary:disabled {
  color: var(--color-disabled);
  cursor: default;
}
.ttl {
  color: var(--color-text-faint);
  font-size: 12px;
}
.url {
  width: 100%;
  padding: 6px 8px;
  border: 1px solid var(--color-line-strong);
  border-radius: 4px;
  background: var(--color-field);
  color: var(--color-text);
  font-family: ui-monospace, Consolas, monospace;
  font-size: 12px;
  overflow-wrap: anywhere;
  resize: vertical;
}
.hint {
  margin: 0;
  color: var(--color-text-faint);
  font-size: 13px;
}
.err {
  margin: 0;
  color: var(--color-danger);
  font-size: 12px;
}
.ok {
  color: var(--color-ok);
  font-size: 12px;
}
</style>
