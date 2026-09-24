<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import appIcon from "../src-tauri/icons/32x32.png";
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
import { refresh as refreshStats, request as requestStats } from "./stores/folderStats";
import { useUploadDrop } from "./stores/dnd";
import UiIcon from "./components/UiIcon.vue";
import ContextMenu from "./components/ContextMenu.vue";
import ExplorerTable from "./components/ExplorerTable.vue";
import GalleryView from "./components/GalleryView.vue";
import PreviewPane from "./components/PreviewPane.vue";
import ProfilesDialog from "./components/ProfilesDialog.vue";
import PropertiesDialog from "./components/PropertiesDialog.vue";
import PresignDialog from "./components/PresignDialog.vue";
import TransfersPanel from "./components/TransfersPanel.vue";

interface Tab {
  id: string;
  profileId: string;
  bucket: string;
  prefix: string;
  /** Back/forward trail of locations inside this tab. */
  history: { bucket: string; prefix: string }[];
  historyIndex: number;
  /** Active bucket-wide search (§17); empty when showing a folder listing. */
  search: string;
  searchNote: string;
  /** Bumped per listing request so a slow response cannot overwrite a newer one. */
  seq: number;
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
const closedTabs: { profileId: string; bucket: string; prefix: string }[] = [];
let tabSeq = 0;

const preview = ref<Row | null>(null);
const previewOpen = ref(false);
const transfersOpen = ref(false);
const propsRow = ref<Row | null>(null);
const propsOpen = ref(false);
const linkRow = ref<Row | null>(null);
const linkOpen = ref(false);
/** §19: details is the default; gallery is the one that matters for image buckets. */
const view = ref<"details" | "gallery">(
  localStorage.getItem("r2explorer.view") === "gallery" ? "gallery" : "details",
);

function setView(next: "details" | "gallery") {
  view.value = next;
  try {
    localStorage.setItem("r2explorer.view", next);
  } catch {
    /* private mode: the choice just does not stick */
  }
}
const menu = ref<{ x: number; y: number; row: Row | null } | null>(null);
const notice = ref("");

const promptState = reactive({ label: "", value: "" });
const pathFocused = ref(false);
const searchDraft = ref("");
const searchOpen = ref(false);
const promptDialog = ref<HTMLDialogElement | null>(null);
const promptInput = ref<HTMLInputElement | null>(null);
const filterInput = ref<HTMLInputElement | null>(null);
const pathInput = ref<HTMLInputElement | null>(null);
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

// Drops arrive as native Tauri events (WebView2 gives HTML5 drags no paths), so the
// overlay is driven by the composable instead of DOM drag handlers. Spec §12.
const { dragging: uploadDragging, start: startUploadDrop, upload: uploadPaths } = useUploadDrop(() => {
  const tab = activeTab.value;
  if (!tab || !tab.bucket || readOnly.value) return null;
  return { profileId: tab.profileId, bucket: tab.bucket, prefix: tab.prefix };
});

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

/** §16: the address bar shows an r2:// URI for the current location. */
const pathDisplay = computed(() => {
  if (pathFocused.value) return pathDraft.value;
  const tab = activeTab.value;
  if (!tab) return "";
  const name = profiles.value.find((p) => p.id === tab.profileId)?.name ?? tab.profileId;
  return `r2://${name || "no-profile"}/${tab.bucket}${tab.prefix ? `/${tab.prefix}` : "/"}`;
});

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
      { id: "copy-key", label: `Copy ${count} keys`, disabled: false },
      { id: "copy-to", label: `Copy ${count} items to…`, disabled: locked },
      { id: "delete", label: `Delete ${count} items`, danger: true, disabled: locked },
    ];
  }

  const folder = open.row.kind === "folder";
  const statsItems: MenuItem[] = folder && tab.bucket
    ? [
        { id: "calc-size", label: "Calculate folder size", disabled: false },
        { id: "refresh-stats", label: "Refresh folder statistics", disabled: false },
      ]
    : [];
  return [
    { id: "open", label: folder ? "Open" : "Preview" },
    ...statsItems,
    { id: "download", label: folder ? "Download folder…" : "Download…", disabled: !tab.bucket },
    { id: "copy-key", label: "Copy object key", disabled: false },
    { id: "copy-uri", label: "Copy R2 URI", disabled: false },
    { id: "temp-link", label: "Create temporary link…", disabled: folder || !tab.bucket },
    { id: "properties", label: "Properties", disabled: false },
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
    history: [{ bucket, prefix }],
    historyIndex: 0,
    search: "",
    searchNote: "",
    seq: 0,
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
  const [gone] = tabs.value.splice(index, 1);
  if (gone) closedTabs.push({ profileId: gone.profileId, bucket: gone.bucket, prefix: gone.prefix });
  if (activeId.value === id) {
    const next = tabs.value[index] ?? tabs.value[index - 1];
    if (next) show(next);
    else activeId.value = "";
  }
}

function restoreTab() {
  const last = closedTabs.pop();
  if (last) newTab(last.profileId, last.bucket, last.prefix);
}

function cycleTab(delta: number) {
  if (tabs.value.length < 2) return;
  const index = tabs.value.findIndex((t) => t.id === activeId.value);
  const next = tabs.value[(index + delta + tabs.value.length) % tabs.value.length];
  if (next) show(next);
}

async function refresh(tab: Tab, reset = true) {
  tab.error = "";
  if (!tab.profileId) {
    tab.rows = [];
    return;
  }
  const seq = ++tab.seq;
  tab.loading = true;
  try {
    if (tab.bucket) {
      const page = await api.listObjects(tab.profileId, tab.bucket, tab.prefix);
      if (seq !== tab.seq) return; // a newer navigation already answered
      tab.rows = pageRows(page);
      tab.nextToken = page.nextToken ?? null;
      tab.folderCount = page.folderCount;
      tab.fileCount = page.fileCount;
      tab.totalSize = page.totalSize;
    } else {
      const buckets = await api.listBuckets(tab.profileId);
      if (seq !== tab.seq) return;
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
    if (seq === tab.seq) tab.loading = false;
  }
}

async function more(tab: Tab) {
  if (!tab.bucket || !tab.nextToken || tab.loading) return;
  const seq = tab.seq;
  tab.loading = true;
  try {
    const page = await api.listObjects(tab.profileId, tab.bucket, tab.prefix, tab.nextToken);
    if (seq !== tab.seq) return;
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

/** Whole-bucket search (§17). Lists server-side; no local index yet. */
async function runSearch() {
  const tab = activeTab.value;
  const query = searchDraft.value.trim();
  if (!tab || !tab.bucket || !query) return;
  tab.error = "";
  tab.loading = true;
  try {
    const found = await api.searchObjects(tab.profileId, tab.bucket, "", query, 500);
    tab.search = query;
    tab.searchNote = found.truncated
      ? `${found.files.length} shown (scan stopped early)`
      : `${found.files.length} of ${found.scanned} objects`;
    tab.rows = pageRows({
      folders: [],
      files: found.files,
      nextToken: null,
      truncated: found.truncated,
      folderCount: 0,
      fileCount: found.files.length,
      totalSize: found.files.reduce((sum, f) => sum + f.size, 0),
    });
    tab.nextToken = null;
    tab.folderCount = 0;
    tab.fileCount = found.files.length;
    tab.totalSize = found.files.reduce((sum, f) => sum + f.size, 0);
    tab.selected = [];
    tab.cursor = -1;
    preview.value = null;
  } catch (e) {
    tab.error = String(e);
  } finally {
    tab.loading = false;
  }
}

function clearSearch() {
  const tab = activeTab.value;
  if (!tab) return;
  tab.search = "";
  tab.searchNote = "";
  searchDraft.value = "";
  void refresh(tab);
}

function navigate(tab: Tab, bucket: string, prefix: string, push = true) {
  tab.bucket = bucket;
  tab.prefix = prefix;
  tab.filter = "";
  tab.search = "";
  tab.searchNote = "";
  preview.value = null;
  if (push) {
    tab.history.splice(tab.historyIndex + 1);
    tab.history.push({ bucket, prefix });
    tab.historyIndex = tab.history.length - 1;
  }
  void refresh(tab);
}

function walk(tab: Tab, delta: number) {
  const next = tab.historyIndex + delta;
  if (next < 0 || next >= tab.history.length) return;
  tab.historyIndex = next;
  navigate(tab, tab.history[next].bucket, tab.history[next].prefix, false);
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
  const parts = pathDraft.value
    .trim()
    .replace(/^r2:\/\//i, "")
    .split("/")
    .filter(Boolean);
  // Accept `r2://profile-name/bucket/prefix` as well as `bucket/prefix`.
  const named = profiles.value.find((p) => p.name === parts[0] || p.id === parts[0]);
  if (named) {
    tab.profileId = named.id;
    parts.shift();
  }
  const bucket = parts.shift() ?? "";
  const rest = parts.join("/");
  navigate(tab, bucket, rest && !rest.endsWith("/") ? `${rest}/` : rest);
  pathFocused.value = false;
  (document.activeElement as HTMLElement | null)?.blur();
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
  preview.value = row;
}

function moveCursor(tab: Tab, delta: number) {
  const rows = viewRows.value;
  if (!rows.length) return;
  const from = tab.cursor < 0 ? (delta > 0 ? -1 : rows.length) : tab.cursor;
  const next = Math.max(0, Math.min(rows.length - 1, from + delta));
  tab.cursor = next;
  tab.selected = [rows[next].key];
  preview.value = rows[next];
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

/** Spec §12 "Upload folder": the picked tree keeps its folder name as a key prefix. */
async function uploadFolder() {
  const tab = activeTab.value;
  if (!tab || !tab.bucket) return;
  const picked = await openDialog({
    multiple: true,
    directory: true,
    title: `Upload folders to ${tab.bucket}/${tab.prefix}`,
  });
  if (!picked) return;
  try {
    await uploadPaths(Array.isArray(picked) ? picked : [picked]);
  } catch (e) {
    tab.error = String(e);
  }
  transfersOpen.value = true;
}

async function download(targets: Row[]) {
  const tab = activeTab.value;
  if (!tab || !tab.bucket) return;
  const files = targets.filter((r) => r.kind === "file");
  if (!files.length) return;
  if (files.length === 1) {
    const dest = await saveDialog({ defaultPath: files[0].name });
    if (dest) transfers.download(tab.profileId, tab.bucket, files[0].key, dest);
  } else {
    const dir = await openDialog({ directory: true, title: `Download ${files.length} files into…` });
    if (!dir || Array.isArray(dir)) return;
    for (const file of files) {
      transfers.download(tab.profileId, tab.bucket, file.key, joinPath(dir, file.name));
    }
  }
  transfersOpen.value = true;
}

function joinPath(dir: string, rest: string): string {
  const sep = dir.includes("\\") ? "\\" : "/";
  return `${dir.replace(/[\\/]+$/, "")}${sep}${rest.split("/").join(sep)}`;
}

/** §12: download a whole prefix, keeping its sub-folder structure. */
async function downloadFolder(row: Row) {
  const tab = activeTab.value;
  if (!tab || !tab.bucket) return;
  const dir = await openDialog({ directory: true, title: `Download “${row.name}” into…` });
  if (!dir || Array.isArray(dir)) return;
  let token: string | undefined;
  let count = 0;
  do {
    const page = await api.listObjects(tab.profileId, tab.bucket, row.prefix, token);
    for (const file of page.files) {
      const rel = file.key.slice(row.prefix.length);
      transfers.download(tab.profileId, tab.bucket, file.key, joinPath(dir, rel));
      count++;
    }
    token = page.nextToken ?? undefined;
    // ponytail: one page-burst per click, no resumable job; switch to a Rust-side
    // listing pump if people queue 100k-file prefixes.
  } while (token && count < 5000);
  transfersOpen.value = true;
}

async function copyText(value: string, what: string) {
  try {
    await navigator.clipboard.writeText(value);
    notice.value = `${what} copied`;
  } catch (e) {
    notice.value = `clipboard refused: ${String(e)}`;
  }
}

function r2Uri(row: Row): string {
  const tab = activeTab.value;
  const name = profiles.value.find((p) => p.id === tab?.profileId)?.name ?? tab?.profileId ?? "";
  return `r2://${name}/${tab?.bucket ?? ""}/${row.key}`;
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
    case "download": {
      const folder = targets.find((r) => r.kind === "folder");
      if (folder) void downloadFolder(folder);
      else void download(targets);
      break;
    }
    case "copy-key":
      void copyText(
        targets.map((r) => r.key).join("\r\n"),
        targets.length > 1 ? `${targets.length} keys` : "Key",
      );
      break;
    case "copy-uri":
      if (targets[0]) void copyText(r2Uri(targets[0]), "R2 URI");
      break;
    case "calc-size":
      if (row) requestStats(tab.profileId, tab.bucket, row.prefix);
      break;
    case "refresh-stats":
      if (row) refreshStats(tab.profileId, tab.bucket, row.prefix);
      break;
    case "temp-link":
      linkRow.value = targets[0] ?? null;
      linkOpen.value = true;
      break;
    case "properties":
      propsRow.value = targets[0] ?? null;
      propsOpen.value = true;
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

function closeActionMenus(event: MouseEvent) {
  const target = event.target as HTMLElement;
  if (target.closest("summary")) return;
  document.querySelectorAll("details.action-menu[open]").forEach((el) => el.removeAttribute("open"));
}

function onKeyDown(event: KeyboardEvent) {
  const target = event.target as HTMLElement | null;
  if (
    target &&
    (target.matches("input, textarea, select") || target.closest("dialog") || target.isContentEditable ||
      (target.matches("button, summary") && !event.ctrlKey && !event.metaKey && !event.altKey))
  ) {
    return;
  }
  const tab = activeTab.value;
  if (!tab) return;

  if (event.ctrlKey || event.metaKey) {
    switch (event.key.toLowerCase()) {
      case "t":
        event.preventDefault();
        if (event.shiftKey) restoreTab();
        else duplicateTab();
        break;
      case "w":
        event.preventDefault();
        closeTab(tab.id);
        break;
      case "tab":
        event.preventDefault();
        cycleTab(event.shiftKey ? -1 : 1);
        break;
      case "a":
        event.preventDefault();
        tab.selected = viewRows.value.map((r) => r.key);
        break;
      case "f":
        event.preventDefault();
        filterInput.value?.focus();
        break;
      case "l":
        event.preventDefault();
        pathInput.value?.focus();
        pathInput.value?.select();
        break;
      default:
        break;
    }
    return;
  }

  if (event.altKey) {
    switch (event.key) {
      case "Enter": {
        event.preventDefault();
        const row = selectedRows.value[0];
        if (row) {
          propsRow.value = row;
          propsOpen.value = true;
        }
        break;
      }
      case "ArrowLeft":
        event.preventDefault();
        walk(tab, -1);
        break;
      case "ArrowRight":
        event.preventDefault();
        walk(tab, 1);
        break;
      case "ArrowUp":
        event.preventDefault();
        goUp();
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
  await startUploadDrop();
  await loadProfiles();
  newTab(profiles.value[0]?.id ?? "", profiles.value[0]?.defaultBucket ?? "");
  // §3: first launch opens the "Add R2 Connection" form instead of an empty grid.
  if (!profiles.value.length) profilesDialog.value?.open();
});

onUnmounted(() => window.removeEventListener("keydown", onKeyDown));
</script>

<template>
  <div class="app" @click="closeActionMenus">
    <div v-if="uploadDragging" class="dropzone">Drop files or folders to upload</div>
    <header class="tabbar">
      <span class="app-name"><img :src="appIcon" alt="" width="24" height="24" /> R2 <span>Explorer</span></span>
      <div class="tabs" aria-label="Open locations">
        <div v-for="tab in tabs" :key="tab.id" class="tab" :class="{ on: tab.id === activeId }">
          <button type="button" class="tab-select" :aria-current="tab.id === activeId ? 'page' : undefined"
            :title="[profiles.find((p) => p.id === tab.profileId)?.name, tab.bucket, tab.prefix].filter(Boolean).join(' / ')"
            @click="show(tab)" @auxclick="$event.button === 1 && closeTab(tab.id)">
            <UiIcon name="bucket" />
            <span class="tlabel">{{ tab.prefix ? basename(tab.prefix.replace(/\/$/, '')) : tab.bucket || 'Buckets' }}</span>
          </button>
          <button type="button" class="tclose" aria-label="Close tab" title="Close tab (Ctrl+W)" @click="closeTab(tab.id)"><UiIcon name="close" /></button>
        </div>
      </div>
      <button type="button" class="icon-button add" aria-label="New tab" title="New tab (Ctrl+T)" @click="duplicateTab"><UiIcon name="plus" /></button>
      <span class="grow"></span>
      <button type="button" @click="profilesDialog?.open()">Connections</button>
      <button type="button" :class="{ on: transfersOpen }" :aria-pressed="transfersOpen" @click="transfersOpen = !transfersOpen">
        Transfers<span v-if="transferBadge" class="count">{{ transferBadge }}</span>
      </button>
    </header>

    <div class="addr">
      <div class="navigation">
        <button type="button" class="icon-button" aria-label="Back" title="Back (Alt+Left)" :disabled="!activeTab || activeTab.historyIndex <= 0" @click="activeTab && walk(activeTab, -1)"><UiIcon name="back" /></button>
        <button type="button" class="icon-button" aria-label="Forward" title="Forward (Alt+Right)" :disabled="!activeTab || activeTab.historyIndex >= activeTab.history.length - 1" @click="activeTab && walk(activeTab, 1)"><UiIcon name="forward" /></button>
        <button type="button" class="icon-button" aria-label="Up one folder" title="Up (Alt+Up)" :disabled="!activeTab?.bucket" @click="goUp"><UiIcon name="up" /></button>
      </div>
      <div class="location" :class="{ editing: pathFocused }">
        <nav class="crumbs" aria-label="Current location">
          <button v-for="(crumb, i) in crumbs" :key="i" type="button" :title="crumb.label" @click="activeTab && navigate(activeTab, crumb.bucket, crumb.prefix)">{{ crumb.label }}</button>
        </nav>
        <input ref="pathInput" :value="pathDisplay" class="path" aria-label="Location" title="Edit location (Ctrl+L)" spellcheck="false" placeholder="Enter a bucket or path"
          @focus="pathFocused = true; pathDraft = (activeTab?.bucket ?? '') + (activeTab?.prefix ? '/' + activeTab.prefix : '')"
          @blur="pathFocused = false" @input="pathDraft = ($event.target as HTMLInputElement).value"
          @keydown.enter="gotoPath" @keydown.esc="pathFocused = false; ($event.target as HTMLInputElement).blur()" />
        <button type="button" class="icon-button refresh" aria-label="Refresh" title="Refresh (F5)" :disabled="!activeTab || activeTab.loading" @click="activeTab && refresh(activeTab)"><UiIcon name="refresh" /></button>
      </div>
      <label class="filter-field">
        <UiIcon name="search" />
        <input ref="filterInput" class="filter" aria-label="Filter this folder" spellcheck="false" placeholder="Filter this folder" :value="activeTab?.filter ?? ''" @input="setFilter" />
        <kbd>Ctrl F</kbd>
      </label>
    </div>

    <div class="tools">
      <div class="action-group">
        <details class="action-menu" @keydown.esc="($event.currentTarget as HTMLElement).removeAttribute('open')">
          <summary><UiIcon name="plus" /> New <UiIcon name="chevron" /></summary>
          <div class="action-options">
            <button type="button" :disabled="!canWrite" @click="makeFolder"><UiIcon name="folder" /> New folder</button>
            <button type="button" :disabled="!canWrite" @click="makeFile"><UiIcon name="file" /> New file</button>
          </div>
        </details>
        <details class="action-menu" @keydown.esc="($event.currentTarget as HTMLElement).removeAttribute('open')">
          <summary><UiIcon name="upload" /> Upload <UiIcon name="chevron" /></summary>
          <div class="action-options">
            <button type="button" :disabled="!canWrite" @click="upload">Upload files…</button>
            <button type="button" :disabled="!canWrite" @click="uploadFolder">Upload folder…</button>
          </div>
        </details>
        <span class="tool-divider"></span>
        <button type="button" class="download-action" aria-label="Download selected items" title="Download selected items" :disabled="!selectedRows.length" @click="download(selectedRows)"><UiIcon name="download" /><span>Download</span></button>
        <button type="button" class="icon-button delete-action" aria-label="Delete selected items" title="Delete selected items" :disabled="!canWrite || !selectedRows.length" @click="remove(selectedRows)"><UiIcon name="trash" /></button>
      </div>
      <div class="view-actions">
        <button type="button" class="search-toggle" :class="{ on: searchOpen || activeTab?.search }" :aria-expanded="searchOpen" :disabled="!activeTab?.bucket" @click="searchOpen = !searchOpen"><UiIcon name="search" /><span>Search bucket</span></button>
        <span class="tool-divider"></span>
        <span class="views" role="group" aria-label="File view">
          <button type="button" class="icon-button" :class="{ on: view === 'details' }" :aria-pressed="view === 'details'" aria-label="Details view" title="Details view" @click="setView('details')"><UiIcon name="list" /></button>
          <button type="button" class="icon-button" :class="{ on: view === 'gallery' }" :aria-pressed="view === 'gallery'" aria-label="Gallery view" title="Gallery view" @click="setView('gallery')"><UiIcon name="grid" /></button>
        </span>
        <button type="button" class="preview-toggle" :class="{ on: previewOpen }" :aria-pressed="previewOpen" title="Toggle preview" @click="previewOpen = !previewOpen"><UiIcon name="preview" /><span>Preview</span></button>
      </div>
    </div>
    <form v-if="searchOpen || activeTab?.search" class="bucket-search" @submit.prevent="runSearch">
      <label for="bucket-search">Search bucket</label>
      <input id="bucket-search" v-model="searchDraft" class="filter search" spellcheck="false" placeholder="Name or pattern, e.g. *.webp" :disabled="!activeTab?.bucket" />
      <button type="submit" :disabled="!activeTab?.bucket || activeTab.loading">Search</button>
      <button type="button" @click="clearSearch(); searchOpen = false">Close</button>
    </form>

    <main class="main">
      <ExplorerTable
        v-if="view === 'details'"
        :rows="viewRows"
        :selected="activeTab?.selected ?? []"
        :cursor="activeTab?.cursor ?? -1"
        :sort-key="activeTab?.sortKey ?? 'name'"
        :sort-dir="activeTab?.sortDir ?? 1"
        :loading="activeTab?.loading ?? false"
        :has-more="!!activeTab?.nextToken"
        :profile-id="activeTab?.profileId ?? ''"
        :bucket="activeTab?.bucket ?? ''"
        @select="selectRow"
        @open="openRow"
        @menu="openMenu"
        @sort="sortBy"
        @more="activeTab && more(activeTab)"
      />
      <GalleryView
        v-else
        :rows="viewRows"
        :selected="activeTab?.selected ?? []"
        :profile-id="activeTab?.profileId ?? ''"
        :bucket="activeTab?.bucket ?? ''"
        :loading="activeTab?.loading ?? false"
        :has-more="!!activeTab?.nextToken"
        @select="selectRow"
        @open="openRow"
        @menu="openMenu"
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

    <footer class="status" role="status">
      <span v-if="activeTab?.error" class="err">{{ activeTab.error }}</span>
      <span v-else-if="notice">{{ notice }}</span>
      <span v-else-if="activeTab?.loading">Working…</span>
      <span v-else-if="activeTab?.search">
        Search “{{ activeTab.search }}” · {{ activeTab.searchNote }}
      </span>
      <span v-else-if="activeTab">
        {{ activeTab.folderCount }} folders · {{ activeTab.fileCount }} files ·
        {{ formatBytes(activeTab.totalSize) }}
      </span>
      <span v-else>No tab open.</span>
      <span class="grow"></span>
      <span v-if="activeTab?.selected.length">{{ activeTab.selected.length }} selected</span>
      <span v-if="transferBadge">{{ transferBadge }} transferring</span>
      <span v-if="readOnly" class="warn">read-only</span>
      <span class="connection-status">{{ profiles.find((p) => p.id === activeTab?.profileId)?.name || "No connection" }}</span>
    </footer>

    <ProfilesDialog ref="profilesDialog" :profiles="profiles" @saved="loadProfiles" />
    <PropertiesDialog
      :profile-id="activeTab?.profileId ?? ''"
      :bucket="activeTab?.bucket ?? ''"
      :row="propsRow"
      :open="propsOpen"
      @close="propsOpen = false"
    />
    <PresignDialog
      :profile-id="activeTab?.profileId ?? ''"
      :bucket="activeTab?.bucket ?? ''"
      :row="linkRow"
      :open="linkOpen"
      @close="linkOpen = false"
    />
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
  /* Hallmark · component scope · genre: utility/desktop · theme: existing dark shell.
     Tokens live here so the panes and dialogs stop re-deriving the same greys. */
  --color-bg: #16181c;
  --color-panel: #1b1d21;
  --color-raised: #23252b;
  --color-field: #14161a;
  --color-line: #34373f;
  --color-line-strong: #3a3d45;
  --color-text: #dfe1e6;
  --color-text-dim: #9aa0aa;
  --color-text-faint: #8a8f99;
  --color-accent: #4a6ea9;
  --color-accent-soft: #2c3a55;
  --color-danger: #ff8a8a;
  --color-ok: #8fd694;
  --color-focus: #7aa2f7;
  --space-1: 2px;
  --space-2: 4px;
  --space-3: 8px;
  --space-4: 12px;
  --radius-s: 3px;
  --radius-m: 5px;
  --text-xs: 11px;
  --text-s: 12px;
  --text-m: 13px;
  --font-ui: "Segoe UI", Inter, Avenir, Helvetica, Arial, sans-serif;
  --font-mono: ui-monospace, "Cascadia Mono", Consolas, monospace;
  --dur-fast: 90ms;
  --ease-out: cubic-bezier(0.2, 0.8, 0.3, 1);
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

.dropzone {
  position: fixed;
  inset: 0;
  z-index: 80;
  display: grid;
  place-items: center;
  border: 2px dashed #4a6ea9;
  background: #16181cd9;
  color: #9dc0ff;
  font-size: 15px;
  pointer-events: none;
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
