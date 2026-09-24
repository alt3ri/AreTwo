// Upload entry points: drag & drop plus folder-picker expansion.
//
// WebView2 gives HTML5 drag events no filesystem paths, so drops are read from
// Tauri's native drag-drop events instead of DOM handlers.
import { ref } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import * as api from "../api";
import { joinPrefix } from "../rows";
import { useTransfers } from "./transfers";

export interface UploadTarget {
  profileId: string;
  bucket: string;
  prefix: string;
}

/** Expands dropped/picked paths into upload candidates; folders are walked recursively. */
export const expandPaths = (paths: string[]) => api.expandUploadPaths(paths);

export function useUploadDrop(target: () => UploadTarget | null) {
  const dragging = ref(false);
  const transfers = useTransfers();
  let started = false;

  /** Enqueues every file under `paths`. Returns how many were queued. */
  async function upload(paths: string[]): Promise<number> {
    const dest = target();
    if (!dest || !paths.length) return 0;
    const files = await expandPaths(paths);
    for (const file of files) {
      transfers.upload(dest.profileId, dest.bucket, joinPrefix(dest.prefix, file.rel), file.path);
    }
    return files.length;
  }

  async function start() {
    if (started) return;
    started = true;
    try {
      await getCurrentWebview().onDragDropEvent((event) => {
        const payload = event.payload;
        if (payload.type === "enter" || payload.type === "over") {
          dragging.value = target() !== null;
        } else if (payload.type === "leave") {
          dragging.value = false;
        } else if (payload.type === "drop") {
          dragging.value = false;
          void upload(payload.paths);
        }
      });
    } catch {
      // Not inside a Tauri webview (plain `pnpm dev` in a browser): drops just never fire.
    }
  }

  return { dragging, start, upload };
}
