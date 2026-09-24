// Lazy image cache for the gallery view (§18, §19).
//
// R2 has no thumbnail service, so a "thumbnail" is a capped GetObject turned into a
// data URL by the backend. Only tiles on screen are fetched.
// ponytail: in-memory LRU, no disk cache and no re-encoding — add a real thumbnail
// cache (downscale + cache.db keyed by ETag) once buckets full of 10 MB images
// start thrashing memory.
import { reactive } from "vue";
import * as api from "../api";

/** Above this the tile shows a placeholder instead of pulling the object. */
export const THUMB_CAP = 4_000_000;

const MAX_ENTRIES = 120;
const cache = reactive<Record<string, string>>({});
const errors = reactive<Record<string, string>>({});
const order: string[] = [];
const pending = new Set<string>();

const keyOf = (profileId: string, bucket: string, key: string) => `${profileId}|${bucket}|${key}`;

export function peek(profileId: string, bucket: string, key: string): string | undefined {
  return cache[keyOf(profileId, bucket, key)];
}

export function errorFor(profileId: string, bucket: string, key: string): string | undefined {
  return errors[keyOf(profileId, bucket, key)];
}

/** Fetch once; later callers hit the cache. */
export function request(profileId: string, bucket: string, key: string) {
  const k = keyOf(profileId, bucket, key);
  if (cache[k] || errors[k] || pending.has(k) || !profileId || !bucket) return;
  pending.add(k);
  api
    .readImage(profileId, bucket, key, THUMB_CAP)
    .then((url) => {
      cache[k] = url;
      order.push(k);
      while (order.length > MAX_ENTRIES) {
        const old = order.shift();
        if (old && old !== k) delete cache[old];
      }
    })
    .catch((e: unknown) => {
      errors[k] = String(e);
    })
    .finally(() => pending.delete(k));
}

export function forgetProfile(profileId: string) {
  for (const k of Object.keys(cache)) if (k.startsWith(`${profileId}|`)) delete cache[k];
  for (const k of Object.keys(errors)) if (k.startsWith(`${profileId}|`)) delete errors[k];
}
