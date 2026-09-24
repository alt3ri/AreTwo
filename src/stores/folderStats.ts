// Recursive prefix scans behind the Folders / Files / Size columns (§6).
//
// R2 has no folders, so these numbers cost a paginated ListObjectsV2 per prefix.
// Requests are lazy (only rows on screen), de-duped, capped at 3 in flight, and
// cached for 30 minutes.
// ponytail: in-memory cache only, gone on restart; a SQLite index (task-005) is
// the upgrade path when these scans start to hurt on million-object buckets.
import { reactive } from "vue";
import * as api from "../api";

export interface StatsEntry {
  state: "loading" | "ready" | "error";
  files: number;
  folders: number;
  size: number;
  scannedAt: string;
  error: string;
  /** ms epoch of the last successful scan, for the TTL. */
  at: number;
}

const TTL = 30 * 60 * 1000;
const MAX_PARALLEL = 3;

const cache = reactive<Record<string, StatsEntry>>({});
const waiting: string[] = [];
let running = 0;

const keyOf = (profileId: string, bucket: string, prefix: string) =>
  `${profileId}|${bucket}|${prefix}`;

/** Cached numbers only — never triggers a scan. */
export function peek(profileId: string, bucket: string, prefix: string): StatsEntry | undefined {
  return cache[keyOf(profileId, bucket, prefix)];
}

function fresh(entry: StatsEntry | undefined): boolean {
  return !!entry && entry.state === "ready" && Date.now() - entry.at < TTL;
}

async function run(k: string) {
  const [profileId, bucket, prefix] = k.split("|");
  running++;
  try {
    const stats = await api.folderStats(profileId, bucket, prefix);
    cache[k] = {
      state: "ready",
      files: stats.files,
      folders: stats.folders,
      size: stats.size,
      scannedAt: stats.scannedAt,
      error: "",
      at: Date.now(),
    };
  } catch (e) {
    cache[k] = {
      state: "error",
      files: 0,
      folders: 0,
      size: 0,
      scannedAt: "",
      error: String(e),
      at: 0,
    };
  } finally {
    running--;
    pump();
  }
}

function pump() {
  while (running < MAX_PARALLEL && waiting.length) {
    const k = waiting.shift();
    if (k) void run(k);
  }
}

/** Scan `prefix` once, unless it is fresh or already queued. */
export function request(profileId: string, bucket: string, prefix: string) {
  if (!profileId || !bucket) return;
  const k = keyOf(profileId, bucket, prefix);
  const entry = cache[k];
  if (fresh(entry) || entry?.state === "loading") return;
  if (waiting.includes(k)) return;
  cache[k] = {
    state: "loading",
    files: 0,
    folders: 0,
    size: 0,
    scannedAt: "",
    error: "",
    at: 0,
  };
  waiting.push(k);
  pump();
}

/** Throw the cached numbers away and scan again (right-click → Refresh statistics). */
export function refresh(profileId: string, bucket: string, prefix: string) {
  if (!profileId || !bucket) return;
  delete cache[keyOf(profileId, bucket, prefix)];
  request(profileId, bucket, prefix);
}

export function forgetProfile(profileId: string) {
  for (const k of Object.keys(cache)) {
    if (k.startsWith(`${profileId}|`)) delete cache[k];
  }
}

export function useFolderStats() {
  return { peek, request, refresh, forgetProfile };
}
