<script setup lang="ts">
// Hallmark · component: connection manager modal · genre: utility/desktop · theme: existing dark shell
// states: default · hover · focus-visible · active · disabled · loading · error · success
import { computed, nextTick, reactive, ref } from "vue";
import * as api from "../api";
import type { Profile } from "../types";

const { profiles } = defineProps<{ profiles: Profile[] }>();
const emit = defineEmits<{ saved: [] }>();

const dialog = ref<HTMLDialogElement | null>(null);
const editingId = ref("");
const original = ref<Profile | null>(null);
const secret = ref("");
const revealed = ref(false);
const error = ref("");
const probe = ref("");
const testing = ref(false);
const saving = ref(false);
const confirmDelete = ref("");
const nameInput = ref<HTMLInputElement | null>(null);

const draft = reactive({
  name: "",
  accountId: "",
  accessKeyId: "",
  endpoint: "",
  defaultBucket: "",
  readOnly: false,
});

/** Account id is 32 hex chars; anything shorter is almost certainly a paste error. */
const accountLooksOff = computed(
  () => draft.accountId.trim().length > 0 && !/^[0-9a-f]{32}$/i.test(draft.accountId.trim()),
);
const canSave = computed(
  () => !!draft.name.trim() && !!draft.accountId.trim() && !!draft.accessKeyId.trim(),
);
const endpointHint = computed(
  () =>
    draft.endpoint.trim() ||
    (draft.accountId.trim()
      ? `https://${draft.accountId.trim().slice(0, 6)}….r2.cloudflarestorage.com`
      : "https://<account-id>.r2.cloudflarestorage.com"),
);
const list = computed(() => profiles);
const editing = computed(() => profiles.find((p) => p.id === editingId.value) ?? null);

function clear() {
  editingId.value = "";
  original.value = null;
  secret.value = "";
  revealed.value = false;
  error.value = "";
  probe.value = "";
  confirmDelete.value = "";
  draft.name = "";
  draft.accountId = "";
  draft.accessKeyId = "";
  draft.endpoint = "";
  draft.defaultBucket = "";
  draft.readOnly = false;
}

/** R2's endpoint is bare: a pasted `.../bucket-name` tail is a bucket, not a path. */
function normalizeEndpoint(raw: string): string | null {
  const value = raw.trim();
  if (!value) return null;
  try {
    const url = new URL(value);
    url.pathname = "";
    url.search = "";
    url.hash = "";
    return url.toString().replace(/\/$/, "");
  } catch {
    return value.replace(/\/+$/, "") || null;
  }
}

function edit(profile: Profile) {
  clear();
  editingId.value = profile.id;
  original.value = profile;
  draft.name = profile.name;
  draft.accountId = profile.accountId;
  draft.accessKeyId = profile.accessKeyId;
  draft.endpoint = profile.endpoint ?? "";
  draft.defaultBucket = profile.defaultBucket ?? "";
  draft.readOnly = profile.readOnly;
}

async function save() {
  error.value = "";
  probe.value = "";
  if (!canSave.value) {
    error.value = "Name, account ID and access key ID are required.";
    return;
  }
  if (!editingId.value && !secret.value) {
    error.value = "New connections need the secret access key.";
    return;
  }
  const profile: Profile = {
    id: editingId.value || `p${Date.now().toString(36)}${Math.random().toString(36).slice(2, 7)}`,
    name: draft.name.trim(),
    accountId: draft.accountId.trim(),
    accessKeyId: draft.accessKeyId.trim(),
    endpoint: normalizeEndpoint(draft.endpoint),
    defaultBucket: draft.defaultBucket.trim() || null,
    readOnly: draft.readOnly,
    createdAt: original.value?.createdAt || new Date().toISOString(),
    lastUsedAt: original.value?.lastUsedAt ?? null,
  };
  saving.value = true;
  try {
    await api.saveProfile(profile, secret.value || undefined);
    edit(profile);
    secret.value = "";
    emit("saved");
  } catch (e) {
    error.value = String(e);
  } finally {
    saving.value = false;
  }
}

async function remove(profile: Profile) {
  if (confirmDelete.value !== profile.id) {
    confirmDelete.value = profile.id;
    return;
  }
  error.value = "";
  try {
    await api.deleteProfile(profile.id);
    if (editingId.value === profile.id) clear();
    confirmDelete.value = "";
    emit("saved");
  } catch (e) {
    error.value = String(e);
  }
}

async function test() {
  probe.value = "";
  error.value = "";
  if (!editingId.value) {
    probe.value = "Save the connection first, then test it.";
    return;
  }
  testing.value = true;
  try {
    probe.value = await api.testConnection(editingId.value);
  } catch (e) {
    error.value = String(e);
  } finally {
    testing.value = false;
  }
}

function open() {
  clear();
  dialog.value?.showModal();
  void nextTick(() => nameInput.value?.focus());
}

function close() {
  dialog.value?.close();
}

defineExpose({ open, close });
</script>

<template>
  <dialog ref="dialog" class="sheet">
    <header class="head">
      <strong>Connections</strong>
      <span class="sub">Cloudflare R2 accounts this app can browse</span>
      <button type="button" class="icon" aria-label="Close" title="Close" @click="close">✕</button>
    </header>

    <div class="grid">
      <ul class="list">
        <li v-for="profile in list" :key="profile.id">
          <button
            type="button"
            class="pick"
            :class="{ on: profile.id === editingId }"
            @click="edit(profile)"
          >
            <span class="pname">
              {{ profile.name || profile.id }}
              <span v-if="profile.readOnly" class="badge">read-only</span>
            </span>
            <small>{{ profile.accountId }}</small>
            <small v-if="profile.defaultBucket" class="bucket">→ {{ profile.defaultBucket }}</small>
          </button>
          <button
            type="button"
            class="del"
            :class="{ armed: confirmDelete === profile.id }"
            :title="confirmDelete === profile.id ? 'Click again to delete' : 'Delete connection'"
            @click="remove(profile)"
          >
            {{ confirmDelete === profile.id ? "Delete?" : "✕" }}
          </button>
        </li>
        <li v-if="!list.length" class="none">No connections yet — fill the form and save.</li>
        <li class="spacer"></li>
        <li>
          <button type="button" class="pick new" @click="clear">＋ New connection</button>
        </li>
      </ul>

      <form class="form" @submit.prevent="save">
        <p class="who">
          {{ editing ? `Editing “${editing.name}”` : "New connection" }}
          <span v-if="editing && !editing.readOnly" class="badge ok">writable</span>
        </p>

        <div class="fields">
        <label>
          <span>Profile name</span>
          <input
            ref="nameInput"
            v-model="draft.name"
            placeholder="Production R2"
            autocomplete="off"
          />
        </label>

        <label>
          <span>Account ID</span>
          <input
            v-model="draft.accountId"
            class="mono"
            :class="{ warn: accountLooksOff }"
            placeholder="32 hex characters"
            spellcheck="false"
          />
        </label>
        <p v-if="accountLooksOff" class="tip">Cloudflare account IDs are 32 hex characters.</p>

        <label>
          <span>Access key ID</span>
          <input v-model="draft.accessKeyId" class="mono" autocomplete="off" spellcheck="false" />
        </label>

        <label>
          <span>Secret access key</span>
          <span class="secret">
            <input
              v-model="secret"
              class="mono"
              :type="revealed ? 'text' : 'password'"
              :placeholder="editingId ? 'saved — leave blank to keep' : 'required for a new connection'"
              autocomplete="new-password"
              spellcheck="false"
            />
            <button type="button" class="icon" @click="revealed = !revealed">
              {{ revealed ? "Hide" : "Show" }}
            </button>
          </span>
        </label>

        <label>
          <span>Endpoint</span>
          <input
            v-model="draft.endpoint"
            class="mono"
            :placeholder="endpointHint"
            spellcheck="false"
          />
        </label>
        <p class="tip">Leave blank to use the Cloudflare endpoint for this account.</p>

        <label>
          <span>Default bucket</span>
          <input v-model="draft.defaultBucket" placeholder="optional" spellcheck="false" />
        </label>

        <label class="check">
          <input v-model="draft.readOnly" type="checkbox" />
          <span>
            Read-only
            <small>Blocks uploads, deletes, copies and renames for this connection.</small>
          </span>
        </label>
        </div>

        <p v-if="error" class="err">{{ error }}</p>
        <p v-else-if="probe" class="ok">{{ probe }}</p>

        <div class="actions">
          <button type="button" :disabled="!editingId || testing" @click="test">
            {{ testing ? "Testing…" : "Test connection" }}
          </button>
          <span class="grow"></span>
          <button type="button" @click="clear">Reset</button>
          <button type="submit" class="primary" :disabled="saving">
            {{ saving ? "Saving…" : "Save" }}
          </button>
        </div>
      </form>
    </div>
  </dialog>
</template>

<style scoped>
/* Hallmark · component: connection manager modal · genre: utility/desktop · theme: existing dark shell
 * states: default · hover · focus · active · disabled · loading · error · success
 * contrast: pass (tokens in :root)
 */
.sheet {
  width: 780px;
  max-width: 94vw;
  padding: 0;
  border: 1px solid var(--color-line-strong);
  border-radius: var(--radius-m);
  background: var(--color-panel);
  color: var(--color-text);
  font-size: var(--text-m);
}
.sheet::backdrop {
  background: var(--color-backdrop);
}

.head {
  display: flex;
  align-items: baseline;
  gap: var(--space-3);
  padding: 9px var(--space-4);
  border-bottom: 1px solid var(--color-line);
}
.head strong {
  font-size: var(--text-m);
}
.head .icon {
  border-color: transparent;
  background: none;
  color: var(--color-text-dim);
}
.head .icon:hover {
  background: var(--color-raised);
  color: var(--color-text);
}
.sub {
  flex: 1;
  color: var(--color-text-faint);
  font-size: var(--text-s);
}

.grid {
  display: grid;
  grid-template-columns: 236px minmax(0, 1fr);
  height: min(560px, 68vh);
}

.list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: var(--space-3);
  overflow: auto;
  border-right: 1px solid var(--color-line);
  list-style: none;
}
.list li {
  display: flex;
  align-items: stretch;
  gap: var(--space-2);
  min-width: 0;
}
.list .spacer {
  flex: 1;
}
.pick {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
  padding: 6px var(--space-3);
  border: 1px solid transparent;
  border-radius: var(--radius-s);
  background: none;
  color: var(--color-text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.pick:hover {
  background: var(--color-raised);
}
.pick.on {
  border-color: var(--color-accent);
  background: var(--color-accent-soft);
}
.pick:focus-visible {
  outline: 2px solid var(--color-focus);
  outline-offset: -1px;
}
.pick small {
  overflow: hidden;
  color: var(--color-text-faint);
  font-size: var(--text-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pname {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  overflow: hidden;
  font-size: var(--text-m);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.new {
  flex-direction: row;
  align-items: center;
  justify-content: center;
  border: 1px dashed var(--color-line-strong);
  color: var(--color-text-dim);
  text-align: center;
}
.none {
  color: var(--color-text-faint);
  font-size: var(--text-s);
}
.badge {
  flex: none;
  padding: 0 5px;
  border: 1px solid var(--color-line-strong);
  border-radius: var(--radius-s);
  color: var(--color-text-faint);
  font-size: var(--text-xs);
}
.badge.ok {
  border-color: var(--color-line-strong);
  color: var(--color-ok);
}

.form {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto auto;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4) var(--space-4);
  min-height: 0;
}
.fields {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-height: 0;
  overflow: auto;
  padding-right: var(--space-2);
}
.who {
  margin: 0 0 var(--space-1);
  color: var(--color-text-dim);
  font-size: var(--text-s);
}
.form label {
  display: flex;
  flex-direction: column;
  gap: 3px;
}
.form label > span:first-child {
  color: var(--color-text-dim);
  font-size: var(--text-s);
}
.form input:not([type="checkbox"]) {
  height: 26px;
  padding: 0 var(--space-3);
  border: 1px solid var(--color-line-strong);
  border-radius: var(--radius-s);
  background: var(--color-field);
  color: var(--color-text);
  font: inherit;
  font-size: var(--text-s);
}
.form input:hover {
  border-color: var(--color-line-strong);
}
.form input:focus-visible {
  outline: 2px solid var(--color-focus);
  outline-offset: -1px;
  border-color: var(--color-focus);
}
.form input.warn {
  border-color: var(--color-warn);
}
.mono {
  font-family: var(--font-mono);
}
.secret {
  display: flex;
  gap: var(--space-2);
}
.secret input {
  flex: 1;
  min-width: 0;
}
.tip {
  margin: calc(var(--space-1) * -1) 0 0;
  color: var(--color-text-faint);
  font-size: var(--text-xs);
}
.form label.check {
  flex-direction: row;
  align-items: flex-start;
  gap: var(--space-3);
  margin-top: var(--space-1);
  font-size: var(--text-s);
}
.check span {
  display: flex;
  flex-direction: column;
}
.check small {
  color: var(--color-text-faint);
  font-size: var(--text-xs);
}

.icon,
.actions button,
.del {
  border: 1px solid var(--color-line-strong);
  border-radius: var(--radius-s);
  background: var(--color-raised);
  color: var(--color-text);
  font: inherit;
  font-size: var(--text-s);
  cursor: pointer;
  transition: background var(--dur-fast) var(--ease-out);
}
.icon {
  flex: none;
  padding: 2px 8px;
}
.del {
  flex: none;
  padding: 0 7px;
  color: var(--color-text-faint);
}
.del:hover {
  border-color: var(--color-danger);
  color: var(--color-danger);
}
.del.armed {
  border-color: var(--color-danger);
  background: var(--color-danger-soft);
  color: var(--color-danger);
}
.icon:hover,
.actions button:hover:not(:disabled) {
  background: var(--color-line);
}
.icon:active,
.actions button:active:not(:disabled) {
  transform: translateY(1px);
}
.actions button:focus-visible,
.del:focus-visible {
  outline: 2px solid var(--color-focus);
  outline-offset: 1px;
}
.actions button:disabled {
  color: var(--color-disabled);
  cursor: default;
}
.actions {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  margin: var(--space-2) calc(var(--space-4) * -1) calc(var(--space-4) * -1);
  padding: var(--space-3) var(--space-4);
  border-top: 1px solid var(--color-line);
  background: var(--color-panel);
}
.actions button {
  padding: 5px 12px;
}
.actions .primary {
  border-color: var(--color-accent);
  background: var(--color-accent-soft);
}
.grow {
  flex: 1;
}
.err {
  margin: 0;
  color: var(--color-danger);
  font-size: var(--text-s);
}
.ok {
  margin: 0;
  color: var(--color-ok);
  font-size: var(--text-s);
}

.head { padding: var(--space-5); align-items: center; }
.head strong { font-size: var(--text-l); }
.form { padding-top: var(--space-5); }
.fields { gap: var(--space-3); }
.form input:not([type="checkbox"]) { height: 32px; }
.sheet { max-height: calc(100dvh - 32px); }
@media (max-width: 640px) {
  .sheet { max-width: calc(100vw - 24px); }
  .head .sub { display: none; }
  .head .icon { margin-left: auto; }
  .grid { display: flex; flex-direction: column; height: min(680px, 80dvh); }
  .list { flex: none; max-height: 112px; border-right: 0; border-bottom: 1px solid var(--color-line); }
  .list .spacer { display: none; }
  .form { flex: 1; min-width: 0; }
  .actions { gap: var(--space-2); flex-wrap: wrap; }
  .actions button { padding-inline: var(--space-3); }
}

</style>
