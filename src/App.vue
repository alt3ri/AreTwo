<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import { confirm, message, open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import type { Profile } from "./types";
import {
  basename,
  bucketRows,
  filterRows,
  formatBytes,
  joinPrefix,
  pageRows,
  parentPrefix,
  sortRows,
  type MenuItem,
  type Row,
  type SortKey,
} from "./rows";
import { useTransfers } from "./stores/transfers";
import ContextMenu from "./components/ContextMenu.vue";
import ExplorerTable from "./components/ExplorerTable.vue";
import PreviewPane from "./components/PreviewPane.vue";
import ProfilesDialog from "./components/ProfilesDialog.vue";
import TransfersPanel from "./components/TransfersPanel.vue";

interface Tab {
  id: string;
  profileId: string;
  bucket: string;
  prefix: string;
  rows: Row[];
  selected: string[];
  cursor: number;
  filter: string;
  sortKey: SortKey;
  sortDir: 1 | -1;
  loading: boolean;
  error: string;
  nextToken: string | null;
  folderCount: number;
  fileCount: number;
  totalSize: number;
}

const profiles = ref<Profile[]>([]);
const tabs = ref<Tab[]>([]);
const activeId = ref("");
let tabSeq = 0;

const preview = ref<Row | null>(null);
const previewOpen = ref(true);
const transfersOpen = ref(false);
const menu = ref<{ x: number; y: number; row: Row | null } | null>(null);
const notice = ref("");

const promptState = reactive({ label: "", value: "" });
const promptDialog = ref<HTMLDialogElement | null>(null);
const promptInput = ref<HTMLInputElement | null>(null);
const filterInput = ref<HTMLInputElement | null>(null);
const profilesDialog = ref<{ open: () => void } | null>(null);
let promptResolve: ((value: string | null) => void) | null = null;

const transfers = useTransfers();
const transferBadge = computed(() => transfers.activeCount.value + transfers.queuedCount.value);

const activeTab = computed(() => tabs.value.find((t) => t.id === activeId.value) ?? null);
const viewRows = computed(() => {
  const tab = activeTab.value;
  return tab ? sortRows(filterRows(tab.rows, tab.filter), { key: tab.sortKey, dir: tab.sortDir }) : [];
});
const selectedRows = computed<Row[]>(() => {
  const tab = activeTab.value;
  if (!tab || !tab.selected.length) return [];
  const keys = new Set(tab.selected);
  return viewRows.value.filter((r) => keys.has(r.key));
});
const readOnly = computed(
  () => profiles.value.find((p) => p.id === activeTab.value?.profileId)?.readOnly === true,
);
const canWrite = computed(() => !!activeTab.value?.bucket && !readOnly.value);

const crumbs = computed(() => {
  const tab = activeTab.value;
  if (!tab) return [];
  const out = [
    {
      label: profiles.value.find((p) => p.id === tab.profileId)?.name ?? "No profile",
      bucket: "",
      prefix: "",
    },
  ];
  if (!tab.bucket) return out;
  out.push({ label: tab.bucket, bucket: tab.bucket, prefix: "" });
  let prefix = "";
  for (const part of tab.prefix.split("/").filter(Boolean)) {
    prefix += `${part}/`;
    out.push({ label: part, bucket: tab.bucket, prefix });
  }
  return out;
});

const pathDraft = ref("");
watch(
  () => {
    const tab = activeTab.value;
    return tab ? `${tab.id}|${tab.bucket}|${tab.prefix}` : "";
  },
  () => {
    const tab = activeTab.value;
    pathDraft.value = tab ? `${tab.bucket}${tab.prefix ? `/${tab.prefix}` : ""}` : "";
  },
  { immediate: true },
);

const menuItems = computed<MenuItem[]>(() => {
  const tab = activeTab.value;
  const open = menu.value;
  if (!tab || !open) return [];
  const locked = readOnly.value || !tab.bucket;

  if (!open.row) {
    return [
      { id: "new-folder", label: "New folder…", disabled: locked },
      { id: "new-file", label: "New file…", disabled: locked },
      { id: "upload", label: "Upload…", disabled: locked },
      { id: "refresh", label: "Refresh", disabled: false },
    ];
  }

  const count = selectedRows.value.length;
  if (count > 1) {
    return [
      { id: "download", label: `Download ${count} files…`, disabled: !tab.bucket },
      { id: "copy-to", label: `Copy ${count} items to…`, disabled: locked },
      { id: "delete", label: `Delete ${count} items`, danger: true, disabled: locked },
    ];
  }

  const folder = open.row.kind === "folder";
  return [
    { id: "open", label: folder ? "Open" : "Preview" },
    { id: "download", label: "Download…", disabled: folder || !tab.bucket },
    { id: "copy-to", label: "Copy to…", disabled: locked },
    { id: "rename", label: "Rename…", disabled: locked },
    { id: "delete", label: "Delete", danger: true, disabled: locked },
  ];
});

async function loadProfiles() {
  try {
    profiles.value = await api.listProfiles();
  } catch (e) {
    notice.value = String(e);
  }
}

function newTab(profileId = "", bucket = "", prefix = "") {
  tabs.value.push({
    id: `tab${++tabSeq}`,
    profileId,
    bucket,
    prefix,
    rows: [],
    selected: [],
    cursor: -1,
    filter: "",
    sortKey: "name",
    sortDir: 1,
    loading: false,
    error: "",
    nextToken: null,
    folderCount: 0,
    fileCount: 0,
    totalSize: 0,
  });
  show(tabs.value[tabs.value.length - 1]);
}

function show(tab: Tab) {
  activeId.value = tab.id;
  preview.value = null;
  void refresh(tab);
}

function duplicateTab() {
  const tab = activeTab.value;
  newTab(tab?.profileId ?? "", tab?.bucket ?? "", tab?.prefix ?? "");
}

function closeTab(id: string) {
  const index = tabs.value.findIndex((t) => t.id === id);
  if (index < 0) return;
  tabs.value.splice(index, 1);
  if (activeId.value === id) {
    const next = tabs.value[index] ?? tabs.value[index - 1];
    if (next) show(next);
    else activeId.value = "";
  }
}

async function refresh(tab: Tab, reset = true) {
  tab.error = "";
  if (!tab.profileId) {
    tab.rows = [];
    return;
  }
  tab.loading = true;
  try {
    if (tab.bucket) {
      const page = await api.listObjects(tab.profileId, tab.bucket, tab.prefix);
      tab.rows = pageRows(page);
      tab.nextToken = page.nextToken ?? null;
      tab.folderCount = page.folderCount;
      tab.fileCount = page.fileCount;
      tab.totalSize = page.totalSize;
    } else {
      const buckets = await api.listBuckets(tab.profileId);
      tab.rows = bucketRows(buckets);
      tab.nextToken = null;
      tab.folderCount = buckets.length;
      tab.fileCount = 0;
      tab.totalSize = 0;
    }
    if (reset) {
      tab.selected = [];
      tab.cursor = -1;
      preview.value = null;
    }
  } catch (e) {
    tab.error = String(e);
    tab.rows = [];
    tab.selected = [];
    tab.nextToken = null;
  } finally {
    tab.loading = false;
  }
}

async function more(tab: Tab) {
  if (!tab.bucket || !tab.nextToken || tab.loading) return;
  tab.loading = true;
  try {
    const page = await api.listObjects(tab.profileId, tab.bucket, tab.prefix, tab.nextToken);
    tab.rows.push(...pageRows(page));
    tab.nextToken = page.nextToken ?? null;
    tab.folderCount += page.folderCount;
    tab.fileCount += page.fileCount;
    tab.totalSize += page.totalSize;
  } catch (e) {
    tab.error = String(e);
  } finally {
    tab.loading = false;
  }
}

function navigate(tab: Tab, bucket: string, prefix: string) {
  tab.bucket = bucket;
  tab.prefix = prefix;
  tab.filter = "";
  preview.value = null;
  void refresh(tab);
}

function goUp() {
  const tab = activeTab.value;
  if (!tab) return;
  if (tab.prefix) navigate(tab, tab.bucket, parentPrefix(tab.prefix));
  else if (tab.bucket) navigate(tab, "", "");
}

function gotoPath() {
  const tab = activeTab.value;
  if (!tab) return;
  const raw = pathDraft.value.trim().replace(/^\/+/, "");
  const slash = raw.indexOf("/");
  const bucket = slash < 0 ? raw : raw.slice(0, slash);
  const rest = (slash < 0 ? "" : raw.slice(slash + 1)).replace(/^\/+/, "");
  navigate(tab, bucket, rest && !rest.endsWith("/") ? `${rest}/` : rest);
}

function openRow(row: Row) {
  const tab = activeTab.value;
  if (!tab) return;
  if (row.kind === "folder") {
    navigate(tab, row.bucketName ?? tab.bucket, row.bucketName ? "" : row.prefix);
    return;
  }
  preview.value = row;
  previewOpen.value = true;
}

function selectRow(row: Row, event: MouseEvent) {
  const tab = activeTab.value;
  if (!tab) return;
  const index = viewRows.value.findIndex((r) => r.key === row.key);

  if (event.shiftKey && tab.cursor >= 0 && index >= 0) {
    const [from, to] = tab.cursor < index ? [tab.cursor, index] : [index, tab.cursor];
    tab.selected = viewRows.value.slice(from, to + 1).map((r) => r.key);
  } else if (event.ctrlKey || event.metaKey) {
    tab.selected = tab.selected.includes(row.key)
      ? tab.selected.filter((k) => k !== row.key)
      : [...tab.selected, row.key];
    tab.cursor = index;
  } else {
    tab.selected = [row.key];
    tab.cursor = index;
  }
  preview.value = row.kind === "file" ? row : null;
}

function moveCursor(tab: Tab, delta: number) {
  const rows = viewRows.value;
  if (!rows.length) return;
  const from = tab.cursor < 0 ? (delta > 0 ? -1 : rows.length) : tab.cursor;
  const next = Math.max(0, Math.min(rows.length - 1, from + delta));
  tab.cursor = next;
  tab.selected = [rows[next].key];
  preview.value = rows[next].kind === "file" ? rows[next] : null;
}

function sortBy(key: SortKey) {
  const tab = activeTab.value;
  if (!tab) return;
  if (tab.sortKey === key) tab.sortDir = tab.sortDir === 1 ? -1 : 1;
  else {
    tab.sortKey = key;
    tab.sortDir = 1;
  }
}

async function guard(tab: Tab, work: () => Promise<unknown>) {
  tab.error = "";
  try {
    await work();
    await refresh(tab, false);
  } catch (e) {
    tab.error = String(e);
    await message(String(e), { title: "R2 Explorer", kind: "error" });
  }
}

async function askText(label: string, value = ""): Promise<string | null> {
  promptState.label = label;
  promptState.value = value;
  promptDialog.value?.showModal();
  await nextTick();
  promptInput.value?.focus();
  promptInput.value?.select();
  return new Promise((resolve) => {
    promptResolve = resolve;
  });
}

function submitPrompt() {
  const resolve = promptResolve;
  promptResolve = null;
  const value = promptState.value.trim();
  promptDialog.value?.close();
  resolve?.(value || null);
}

function cancelPrompt() {
  const resolve = promptResolve;
  promptResolve = null;
  promptDialog.value?.close();
  resolve?.(null);
}

async function makeFolder() {
  const tab = activeTab.value;
  if (!tab) return;
  const name = await askText("New folder name");
  if (!name) return;
  const key = joinPrefix(tab.prefix, name.replace(/\/+$/, ""));
  await guard(tab, () => api.createFolder(tab.profileId, tab.bucket, `${key}/`));
}

async function makeFile() {
  const tab = activeTab.value;
  if (!tab) return;
  const name = await askText("New file name");
  if (!name) return;
  await guard(tab, () => api.createFile(tab.profileId, tab.bucket, joinPrefix(tab.prefix, name)));
}

async function upload() {
  const tab = activeTab.value;
  if (!tab || !tab.bucket) return;
  const picked = await openDialog({
    multiple: true,
    directory: false,
    title: `Upload to ${tab.bucket}/${tab.prefix}`,
  });
  if (!picked) return;
  for (const path of Array.isArray(picked) ? picked : [picked]) {
    transfers.upload(tab.profileId, tab.bucket, joinPrefix(tab.prefix, basename(path)), path);
  }
  transfersOpen.value = true;
}

async function download(targets: Row[]) {
  const tab = activeTab.value;
  if (!tab || !tab.bucket) return;
  const files = targets.filter((r) => r.kind === "file");
  if (!files.length) return;
  for (const file of files) {
    const dest = await saveDialog({ defaultPath: file.name });
    if (dest) transfers.download(tab.profileId, tab.bucket, file.key, dest);
  }
  transfersOpen.value = true;
}

async function remove(targets: Row[]) {
  const tab = activeTab.value;
  if (!tab || !tab.bucket || !targets.length) return;
  const files = targets.filter((r) => r.kind === "file");
  const folders = targets.filter((r) => r.kind === "folder");
  const ok = await confirm(
    `Delete ${targets.length} item${targets.length === 1 ? "" : "s"}? Folders go recursively and this cannot be undone.`,
    { title: "Delete", kind: "warning" },
  );
  if (!ok) return;
  await guard(tab, async () => {
    if (files.length) await api.deleteObjects(tab.profileId, tab.bucket, files.map((f) => f.key));
    for (const folder of folders) await api.deletePrefix(tab.profileId, tab.bucket, folder.prefix);
  });
}

async function rename(targets: Row[]) {
  const tab = activeTab.value;
  const row = targets[0];
  if (!tab || !tab.bucket || !row) return;
  const next = await askText(`Rename “${row.name}” to`, row.name);
  if (!next || next === row.name) return;
  const to = joinPrefix(parentPrefix(row.key), next);
  await guard(tab, async () => {
    if (row.kind === "file") {
      await api.copyObject(tab.profileId, tab.bucket, row.key, to, false);
      await api.deleteObjects(tab.profileId, tab.bucket, [row.key]);
    } else {
      await api.copyPrefix(tab.profileId, tab.bucket, row.prefix, to.endsWith("/") ? to : `${to}/`, false);
      await api.deletePrefix(tab.profileId, tab.bucket, row.prefix);
    }
  });
}

async function copyTo(targets: Row[]) {
  const tab = activeTab.value;
  const first = targets[0];
  if (!tab || !tab.bucket || !first) return;
  const many = targets.length > 1;
  const dest = await askText(
    many ? "Copy into prefix" : "Copy to (full key or prefix)",
    many ? tab.prefix : first.key,
  );
  if (!dest) return;
  const base = many && !dest.endsWith("/") ? `${dest}/` : dest;
  await guard(tab, async () => {
    for (const row of targets) {
      if (row.kind === "file") {
        await api.copyObject(tab.profileId, tab.bucket, row.key, many ? joinPrefix(base, row.name) : base, false);
      } else {
        await api.copyPrefix(
          tab.profileId,
          tab.bucket,
          row.prefix,
          many ? `${joinPrefix(base, row.name)}/` : base,
          false,
        );
      }
    }
  });
}

function openMenu(row: Row | null, x: number, y: number) {
  menu.value = {
    row,
    x: Math.min(x, window.innerWidth - 220),
    y: Math.min(y, window.innerHeight - 240),
  };
}

function pickMenu(item: MenuItem) {
  const open = menu.value;
  const tab = activeTab.value;
  menu.value = null;
  if (!open || !tab) return;

  const row = open.row;
  const targets = row ? (tab.selected.includes(row.key) ? selectedRows.value : [row]) : [];

  switch (item.id) {
    case "open": {
      const target = row ?? targets[0];
      if (target) openRow(target);
      break;
    }
    case "refresh":
      void refresh(tab);
      break;
    case "new-folder":
      void makeFolder();
      break;
    case "new-file":
      void makeFile();
      break;
    case "upload":
      void upload();
      break;
    case "download":
      void download(targets);
      break;
    case "copy-to":
      void copyTo(targets);
      break;
    case "rename":
      void rename(targets);
      break;
    case "delete":
      void remove(targets);
      break;
    default:
      break;
  }
}

function onKeyDown(event: KeyboardEvent) {
  const target = event.target as HTMLElement | null;
  if (
    target &&
    (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)
  ) {
    return;
  }
  const tab = activeTab.value;
  if (!tab) return;

  if (event.ctrlKey || event.metaKey) {
    switch (event.key.toLowerCase()) {
      case "t":
        event.preventDefault();
        duplicateTab();
        break;
      case "w":
        event.preventDefault();
        closeTab(tab.id);
        break;
      case "a":
        event.preventDefault();
        tab.selected = viewRows.value.map((r) => r.key);
        break;
      case "f":
        event.preventDefault();
        filterInput.value?.focus();
        break;
      default:
        break;
    }
    return;
  }

  switch (event.key) {
    case "ArrowDown":
      event.preventDefault();
      moveCursor(tab, 1);
      break;
    case "ArrowUp":
      event.preventDefault();
      moveCursor(tab, -1);
      break;
    case "Enter": {
      event.preventDefault();
      const row = viewRows.value[tab.cursor] ?? selectedRows.value[0];
      if (row) openRow(row);
      break;
    }
    case "Backspace":
      event.preventDefault();
      goUp();
      break;
    case "Delete":
      event.preventDefault();
      void remove(selectedRows.value);
      break;
    case "F2":
      event.preventDefault();
      void rename(selectedRows.value);
      break;
    case "F5":
      event.preventDefault();
      void refresh(tab);
      break;
    case "/":
      event.preventDefault();
      filterInput.value?.focus();
      break;
    default:
      break;
  }
}

function setFilter(event: Event) {
  const tab = activeTab.value;
  if (tab) tab.filter = (event.target as HTMLInputElement).value;
}

onMounted(async () => {
  window.addEventListener("keydown", onKeyDown);
  await loadProfiles();
  newTab(profiles.value[0]?.id ?? "", profiles.value[0]?.defaultBucket ?? "");
});

onUnmounted(() => window.removeEventListener("keydown", onKeyDown));
</script>

<template>
  <div class="app">
    <div class="tabbar">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        type="button"
        class="tab"
        :class="{ on: tab.id === activeId }"
        @click="show(tab)"
      >
        <span class="tlabel">
          {{ profiles.find((p) => p.id === tab.profileId)?.name ?? "—" }} ·
          {{ tab.bucket || "buckets" }}{{ tab.prefix ? `/${tab.prefix}` : "" }}
        </span>
        <span class="tclose" @click.stop="closeTab(tab.id)">✕</span>
      </button>
      <button type="button" class="add" title="New tab (Ctrl+T)" @click="duplicateTab">＋</button>
      <span class="grow"></span>
      <button type="button" @click="profilesDialog?.open()">Profiles</button>
      <button type="button" :class="{ on: transfersOpen }" @click="transfersOpen = !transfersOpen">
        Transfers<span v-if="transferBadge"> ({{ transferBadge }})</span>
      </button>
    </div>

    <div class="addr">
      <button type="button" title="Up" :disabled="!activeTab?.bucket" @click="goUp">↑</button>
      <button type="button" title="Refresh (F5)" :disabled="!activeTab" @click="activeTab && refresh(activeTab)">
        ⟳
      </button>
      <nav class="crumbs">
        <button
          v-for="(crumb, i) in crumbs"
          :key="`${crumb.label}-${i}`"
          type="button"
          @click="activeTab && navigate(activeTab, crumb.bucket, crumb.prefix)"
        >
          {{ crumb.label }}
        </button>
      </nav>
      <input
        v-model="pathDraft"
        class="path"
        spellcheck="false"
        placeholder="bucket/prefix"
        @keydown.enter="gotoPath"
      />
    </div>

    <div class="tools">
      <button type="button" :disabled="!canWrite" @click="makeFolder">New folder</button>
      <button type="button" :disabled="!canWrite" @click="makeFile">New file</button>
      <button type="button" :disabled="!canWrite" @click="upload">Upload…</button>
      <button type="button" :disabled="!selectedRows.length" @click="download(selectedRows)">Download…</button>
      <button type="button" :disabled="!canWrite || !selectedRows.length" @click="remove(selectedRows)">
        Delete
      </button>
      <span class="grow"></span>
      <input
        ref="filterInput"
        class="filter"
        spellcheck="false"
        placeholder="Filter (Ctrl+F)"
        :value="activeTab?.filter ?? ''"
        @input="setFilter"
      />
      <button type="button" :class="{ on: previewOpen }" @click="previewOpen = !previewOpen">Preview</button>
    </div>

    <main class="main">
      <ExplorerTable
        :rows="viewRows"
        :selected="activeTab?.selected ?? []"
        :cursor="activeTab?.cursor ?? -1"
        :sort-key="activeTab?.sortKey ?? 'name'"
        :sort-dir="activeTab?.sortDir ?? 1"
        :loading="activeTab?.loading ?? false"
        :has-more="!!activeTab?.nextToken"
        @select="selectRow"
        @open="openRow"
        @menu="openMenu"
        @sort="sortBy"
        @more="activeTab && more(activeTab)"
      />
      <PreviewPane
        v-if="previewOpen"
        :profile-id="activeTab?.profileId ?? ''"
        :bucket="activeTab?.bucket ?? ''"
        :row="preview"
        @close="previewOpen = false"
        @download="download([$event])"
      />
    </main>

    <TransfersPanel v-if="transfersOpen" @close="transfersOpen = false" />

    <footer class="status">
      <span v-if="activeTab?.error" class="err">{{ activeTab.error }}</span>
      <span v-else-if="notice" class="err">{{ notice }}</span>
      <span v-else-if="activeTab?.loading">Working…</span>
      <span v-else-if="activeTab">
        {{ activeTab.folderCount }} folders · {{ activeTab.fileCount }} files ·
        {{ formatBytes(activeTab.totalSize) }}
      </span>
      <span v-else>No tab open.</span>
      <span class="grow"></span>
      <span v-if="activeTab?.selected.length">{{ activeTab.selected.length }} selected</span>
      <span v-if="transferBadge">{{ transferBadge }} transferring</span>
      <span v-if="readOnly" class="warn">read-only</span>
      <span>{{ profiles.length }} profiles</span>
    </footer>

    <ProfilesDialog ref="profilesDialog" :profiles="profiles" @saved="loadProfiles" />
    <ContextMenu v-if="menu" :x="menu.x" :y="menu.y" :items="menuItems" @pick="pickMenu" @close="menu = null" />

    <dialog ref="promptDialog" class="prompt" @close="cancelPrompt">
      <form @submit.prevent="submitPrompt">
        <label>{{ promptState.label }}</label>
        <input ref="promptInput" v-model="promptState.value" spellcheck="false" />
        <div class="pactions">
          <button type="button" @click="cancelPrompt">Cancel</button>
          <button type="submit" class="primary">OK</button>
        </div>
      </form>
    </dialog>
  </div>
</template>

<style>
:root {
  --cols: minmax(0, 1fr) 92px 148px 76px;
  color: #dfe1e6;
  background: #16181c;
  font-family: "Segoe UI", Inter, Avenir, Helvetica, Arial, sans-serif;
  font-size: 14px;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
}

html,
body,
#app {
  height: 100%;
  margin: 0;
}

button {
  font: inherit;
}

button:disabled {
  color: #6b6f78;
  cursor: default;
}

.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
}

.tabbar,
.addr,
.tools,
.status {
  display: flex;
  flex: none;
  align-items: center;
  gap: 6px;
  padding: 5px 8px;
}

.tabbar {
  overflow-x: auto;
  border-bottom: 1px solid #34373f;
  background: #1b1d21;
}

.tabbar button,
.addr button,
.tools button {
  padding: 4px 9px;
  border: 1px solid #3a3d45;
  border-radius: 4px;
  background: #23252b;
  color: #dfe1e6;
  cursor: pointer;
}

.tabbar button:hover:not(:disabled),
.addr button:hover:not(:disabled),
.tools button:hover:not(:disabled) {
  border-color: #4a6ea9;
}

.tab {
  display: flex;
  flex: none;
  align-items: center;
  gap: 6px;
  max-width: 260px;
}

.tab.on {
  border-color: #4a6ea9;
  background: #2c3a55;
}

.tlabel {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tclose {
  flex: none;
  color: #9aa0aa;
}

.tclose:hover {
  color: #ff8a8a;
}

.add {
  flex: none;
}

.grow {
  flex: 1;
}

.tools button.on,
.tabbar button.on {
  border-color: #4a6ea9;
  background: #2c3a55;
}

.addr {
  border-bottom: 1px solid #2a2d33;
}

.crumbs {
  display: flex;
  align-items: center;
  overflow: hidden;
}

.crumbs button {
  padding: 3px 6px;
  border: 0;
  background: none;
  color: #9dc0ff;
  cursor: pointer;
}

.crumbs button:hover {
  text-decoration: underline;
}

.crumbs button + button::before {
  padding-right: 6px;
  color: #6b6f78;
  content: "/";
}

.path,
.filter {
  padding: 4px 8px;
  border: 1px solid #3a3d45;
  border-radius: 4px;
  background: #14161a;
  color: #e6e8ec;
  font: inherit;
  font-size: 13px;
}

.path {
  flex: 1;
  min-width: 180px;
}

.filter {
  width: 200px;
}

.tools {
  border-bottom: 1px solid #34373f;
}

.main {
  display: flex;
  flex: 1;
  min-height: 0;
}

.status {
  border-top: 1px solid #34373f;
  background: #1b1d21;
  color: #8a8f99;
  font-size: 12px;
}

.err {
  color: #ff8a8a;
}

.warn {
  color: #e0b872;
}

.prompt {
  padding: 0;
  border: 1px solid #3a3d45;
  border-radius: 8px;
  background: #1b1d21;
  color: #dfe1e6;
}

.prompt::backdrop {
  background: #000a;
}

.prompt form {
  display: flex;
  flex-direction: column;
  gap: 10px;
  width: 380px;
  padding: 14px;
}

.prompt label {
  color: #9aa0aa;
  font-size: 12px;
}

.prompt input {
  padding: 6px 8px;
  border: 1px solid #3a3d45;
  border-radius: 4px;
  background: #14161a;
  color: #e6e8ec;
  font: inherit;
}

.pactions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.pactions button {
  padding: 6px 12px;
  border: 1px solid #3a3d45;
  border-radius: 4px;
  background: #23252b;
  color: #dfe1e6;
  cursor: pointer;
}

.pactions .primary {
  border-color: #4a6ea9;
  background: #2c3a55;
}
</style>
