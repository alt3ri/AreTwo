// Transfer queue: real byte progress from Rust (`transfer://progress`), 3 concurrent jobs.
// ponytail: one global queue, no per-profile lanes; add lanes only if a slow profile starves a fast one.
import { computed, reactive, ref, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import {
  ProgressBarStatus,
  getCurrentWindow,
  type ProgressBarState,
} from "@tauri-apps/api/window";
import * as api from "../api";
import type { TransferProgress } from "../types";

export type TransferStatus = "queued" | "active" | "done" | "failed" | "canceled";

export interface Transfer {
  id: string;
  kind: "upload" | "download";
  profileId: string;
  bucket: string;
  key: string;
  name: string;
  localPath: string;
  total: number;
  transferred: number;
  status: TransferStatus;
  error?: string;
  speed: number;
  eta: number;
  replace: boolean;
  startedAt?: number;
}

const MAX_PARALLEL = 3;

const items = reactive<Transfer[]>([]);
const queuePaused = ref(false);
let seq = 0;
let listening = false;
const last = new Map<string, { at: number; bytes: number }>();

function progress(p: TransferProgress) {
  const t = items.find((x) => x.id === p.id);
  if (!t) return;
  const now = performance.now();
  const prev = last.get(p.id);
  if (prev && now > prev.at) {
    const instant = ((p.transferred - prev.bytes) * 1000) / (now - prev.at);
    t.speed = t.speed ? t.speed * 0.7 + instant * 0.3 : instant;
  }
  last.set(p.id, { at: now, bytes: p.transferred });
  t.transferred = p.transferred;
  if (p.total) t.total = p.total;
  t.eta = t.speed > 0 ? Math.max(0, (t.total - t.transferred) / t.speed) : 0;
}

async function ensureListener() {
  if (listening) return;
  listening = true;
  await listen<TransferProgress>("transfer://progress", (e) => progress(e.payload));
}

/** Windows taskbar progress for the whole queue (§23). Progress is 0..100. */
function taskbar() {
  // `getCurrentWindow()` throws outside the shell (plain `pnpm dev` in a browser).
  if (!("__TAURI_INTERNALS__" in window)) return;
  const active = items.some((t) => t.status === "active");
  const done = items.reduce((sum, t) => sum + t.transferred, 0);
  const total = items.reduce((sum, t) => sum + (t.total || 0), 0);
  const percent = total ? Math.min(100, (done / total) * 100) : 0;

  let state: ProgressBarState;
  if (!active) state = { status: ProgressBarStatus.None, progress: 0 };
  else if (queuePaused.value) state = { status: ProgressBarStatus.Paused, progress: percent };
  else if (!total) state = { status: ProgressBarStatus.Indeterminate, progress: 0 };
  else state = { status: ProgressBarStatus.Normal, progress: percent };

  void getCurrentWindow()
    .setProgressBar(state)
    .catch(() => {});
}

watch(() => items.map((t) => `${t.status}:${t.transferred}:${t.total}`).join(), taskbar);
watch(queuePaused, taskbar);

async function run(t: Transfer) {
  t.status = "active";
  t.startedAt = Date.now();
  t.error = undefined;
  if (!t.replace) t.transferred = 0;
  last.delete(t.id);
  try {
    if (t.kind === "upload") {
      await api.uploadObject(t.profileId, t.bucket, t.key, t.localPath, t.id, t.replace);
    } else {
      await api.downloadObject(t.profileId, t.bucket, t.key, t.localPath, t.id);
    }
    t.status = "done";
    if (t.total) t.transferred = t.total;
    t.speed = 0;
    t.eta = 0;
  } catch (e) {
    const msg = String(e);
    t.status = /cancel/i.test(msg) ? "canceled" : "failed";
    if (t.status === "failed") t.error = msg;
    t.speed = 0;
  }
  pump();
}

function pump() {
  if (queuePaused.value) return;
  let slots = MAX_PARALLEL - items.filter((t) => t.status === "active").length;
  for (const t of items) {
    if (slots <= 0) break;
    if (t.status !== "queued") continue;
    slots--;
    void run(t);
  }
}

function enqueue(t: Omit<Transfer, "id" | "status" | "transferred" | "total" | "speed" | "eta">) {
  void ensureListener();
  const item: Transfer = {
    ...t,
    id: `t${++seq}-${Date.now()}`,
    status: "queued",
    transferred: 0,
    total: 0,
    speed: 0,
    eta: 0,
  };
  items.unshift(item);
  pump();
  return item;
}

export function useTransfers() {
  return {
    items,
    queuePaused,
    activeCount: computed(() => items.filter((t) => t.status === "active").length),
    queuedCount: computed(() => items.filter((t) => t.status === "queued").length),
    failedCount: computed(() => items.filter((t) => t.status === "failed").length),
    upload: (profileId: string, bucket: string, key: string, path: string, replace = false) =>
      enqueue({ kind: "upload", profileId, bucket, key, name: key.split("/").pop() || key, localPath: path, replace }),
    download: (profileId: string, bucket: string, key: string, destPath: string) =>
      enqueue({ kind: "download", profileId, bucket, key, name: key.split("/").pop() || key, localPath: destPath, replace: true }),
    cancel: async (id: string) => {
      const t = items.find((x) => x.id === id);
      if (!t) return;
      if (t.status === "active") {
        try {
          await api.cancelTransfer(id);
        } catch {
          /* already finished */
        }
      } else if (t.status === "queued") {
        t.status = "canceled";
      }
    },
    retry: (id: string, replace?: boolean) => {
      const t = items.find((x) => x.id === id);
      if (!t) return;
      if (replace !== undefined) t.replace = replace;
      t.status = "queued";
      t.error = undefined;
      pump();
    },
    remove: (id: string) => {
      const i = items.findIndex((x) => x.id === id);
      if (i >= 0 && items[i].status !== "active") items.splice(i, 1);
    },
    clearCompleted: () => {
      for (let i = items.length - 1; i >= 0; i--) {
        if (items[i].status === "done" || items[i].status === "canceled") items.splice(i, 1);
      }
    },
    togglePause: () => {
      queuePaused.value = !queuePaused.value;
      if (!queuePaused.value) pump();
    },
  };
}
