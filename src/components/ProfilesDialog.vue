<script setup lang="ts">
import { reactive, ref } from "vue";
import * as api from "../api";
import type { Profile } from "../types";

const { profiles } = defineProps<{ profiles: Profile[] }>();
const emit = defineEmits<{ saved: [] }>();

const dialog = ref<HTMLDialogElement | null>(null);
const editingId = ref("");
const original = ref<Profile | null>(null);
const secret = ref("");
const error = ref("");
const probe = ref("");

const draft = reactive({
  name: "",
  accountId: "",
  accessKeyId: "",
  endpoint: "",
  defaultBucket: "",
  readOnly: false,
});

function clear() {
  editingId.value = "";
  original.value = null;
  secret.value = "";
  error.value = "";
  probe.value = "";
  draft.name = "";
  draft.accountId = "";
  draft.accessKeyId = "";
  draft.endpoint = "";
  draft.defaultBucket = "";
  draft.readOnly = false;
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
  if (!draft.name.trim() || !draft.accountId.trim() || !draft.accessKeyId.trim()) {
    error.value = "Name, account id and access key id are required.";
    return;
  }
  const profile: Profile = {
    id: editingId.value || `p${Date.now().toString(36)}${Math.random().toString(36).slice(2, 7)}`,
    name: draft.name.trim(),
    accountId: draft.accountId.trim(),
    accessKeyId: draft.accessKeyId.trim(),
    endpoint: draft.endpoint.trim() || null,
    defaultBucket: draft.defaultBucket.trim() || null,
    readOnly: draft.readOnly,
    createdAt: original.value?.createdAt || new Date().toISOString(),
    lastUsedAt: original.value?.lastUsedAt ?? null,
  };
  try {
    await api.saveProfile(profile, secret.value || undefined);
    edit(profile);
    emit("saved");
  } catch (e) {
    error.value = String(e);
  }
}

async function remove(profile: Profile) {
  error.value = "";
  try {
    await api.deleteProfile(profile.id);
    if (editingId.value === profile.id) clear();
    emit("saved");
  } catch (e) {
    error.value = String(e);
  }
}

async function test() {
  probe.value = "";
  if (!editingId.value) {
    probe.value = "Save the profile first.";
    return;
  }
  try {
    probe.value = await api.testConnection(editingId.value);
  } catch (e) {
    probe.value = String(e);
  }
}

function open() {
  clear();
  dialog.value?.showModal();
}

function close() {
  dialog.value?.close();
}

defineExpose({ open, close });
</script>

<template>
  <dialog ref="dialog" class="profiles">
    <header class="dhead">
      <strong>Profiles</strong>
      <button type="button" class="x" @click="close">✕</button>
    </header>

    <div class="grid">
      <ul class="list">
        <li v-for="profile in profiles" :key="profile.id" :class="{ on: profile.id === editingId }">
          <button type="button" class="pick" @click="edit(profile)">
            <span>{{ profile.name || profile.id }}</span>
            <small>{{ profile.accountId }}{{ profile.readOnly ? " · read-only" : "" }}</small>
          </button>
          <button type="button" class="del" title="Delete profile" @click="remove(profile)">✕</button>
        </li>
        <li v-if="!profiles.length" class="none">No profiles yet — fill the form and save.</li>
      </ul>

      <div class="form">
        <label>Name<input v-model="draft.name" placeholder="Production" /></label>
        <label>Account ID<input v-model="draft.accountId" placeholder="abc123…" /></label>
        <label>Access key ID<input v-model="draft.accessKeyId" /></label>
        <label>
          Secret access key
          <input
            v-model="secret"
            type="password"
            :placeholder="editingId ? 'leave blank to keep the saved key' : ''"
          />
        </label>
        <label>Endpoint<input v-model="draft.endpoint" placeholder="https://account.r2.cloudflarestorage.com" /></label>
        <label>Default bucket<input v-model="draft.defaultBucket" /></label>
        <label class="check">
          <input v-model="draft.readOnly" type="checkbox" />
          Read-only — blocks uploads, deletes, copies and renames
        </label>

        <p v-if="error" class="err">{{ error }}</p>
        <p v-if="probe" class="ok">{{ probe }}</p>

        <div class="actions">
          <button type="button" @click="clear">New</button>
          <button type="button" :disabled="!editingId" @click="test">Test</button>
          <button type="button" class="primary" @click="save">Save</button>
        </div>
      </div>
    </div>
  </dialog>
</template>

<style scoped>
.profiles {
  width: 720px;
  max-width: 92vw;
  padding: 0;
  border: 1px solid #3a3d45;
  border-radius: 8px;
  background: #1b1d21;
  color: #dfe1e6;
}
.profiles::backdrop {
  background: #000a;
}
.dhead {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  border-bottom: 1px solid #34373f;
}
.x,
.pick,
.del,
.actions button {
  border: 1px solid #3a3d45;
  border-radius: 4px;
  background: #23252b;
  color: #dfe1e6;
  font: inherit;
  cursor: pointer;
}
.x,
.del {
  padding: 1px 7px;
}
.grid {
  display: grid;
  grid-template-columns: 230px minmax(0, 1fr);
  min-height: 340px;
}
.list {
  margin: 0;
  padding: 8px;
  border-right: 1px solid #34373f;
  list-style: none;
}
.list li {
  display: flex;
  align-items: stretch;
  gap: 4px;
  margin-bottom: 4px;
}
.list li.on .pick {
  border-color: #4a6ea9;
  background: #2c3a55;
}
.pick {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 2px;
  padding: 6px 8px;
  text-align: left;
}
.pick small {
  color: #8a8f99;
}
.none {
  padding: 8px;
  color: #8a8f99;
  font-size: 13px;
}
.form {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px;
}
.form label {
  display: flex;
  flex-direction: column;
  gap: 3px;
  color: #9aa0aa;
  font-size: 12px;
}
.form input {
  padding: 6px 8px;
  border: 1px solid #3a3d45;
  border-radius: 4px;
  background: #14161a;
  color: #e6e8ec;
  font: inherit;
}
.form label.check {
  flex-direction: row;
  align-items: center;
  gap: 8px;
  color: #dfe1e6;
}
.actions {
  display: flex;
  gap: 8px;
  margin-top: auto;
}
.actions button {
  padding: 6px 12px;
}
.actions button:disabled {
  color: #6b6f78;
  cursor: default;
}
.actions .primary {
  border-color: #4a6ea9;
  background: #2c3a55;
}
.err {
  margin: 0;
  color: #ff8a8a;
  font-size: 13px;
}
.ok {
  margin: 0;
  color: #8fd694;
  font-size: 13px;
}
</style>
