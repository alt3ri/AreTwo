<script setup lang="ts">
import * as api from "../api";
import { formatBytes } from "../rows";
import { useTransfers } from "../stores/transfers";

const emit = defineEmits<{ close: [] }>();

const {
  items,
  queuePaused,
  activeCount,
  queuedCount,
  failedCount,
  cancel,
  retry,
  remove,
  clearCompleted,
  togglePause,
} = useTransfers();

function percent(transferred: number, total: number): number {
  return total > 0 ? Math.min(100, Math.round((transferred / total) * 100)) : 0;
}

function left(total: number, transferred: number, speed: number): string {
  if (!speed) return "";
  const seconds = Math.round(Math.max(0, total - transferred) / speed);
  return seconds < 60 ? `${seconds}s left` : `${Math.floor(seconds / 60)}m ${seconds % 60}s left`;
}

async function reveal(path: string) {
  try {
    await api.revealInExplorer(path);
  } catch {
    /* the file was moved or deleted since the transfer finished */
  }
}
</script>

<template>
  <section class="panel">
    <header class="thead">
      <strong>Transfers</strong>
      <span class="counts">{{ activeCount }} active · {{ queuedCount }} queued · {{ failedCount }} failed</span>
      <button type="button" :class="{ on: queuePaused }" @click="togglePause">
        {{ queuePaused ? "Resume" : "Pause" }}
      </button>
      <button type="button" @click="clearCompleted">Clear finished</button>
      <button type="button" class="x" @click="emit('close')">✕</button>
    </header>

    <p v-if="!items.length" class="none">No transfers yet.</p>

    <ul class="list">
      <li v-for="t in items" :key="t.id" :class="t.status">
        <div class="line">
          <span class="arrow">{{ t.kind === "upload" ? "↑" : "↓" }}</span>
          <span class="name">{{ t.name }}</span>
          <span class="status">{{ t.status }}</span>
        </div>
        <div class="bar">
          <div class="fill" :style="{ width: percent(t.transferred, t.total) + '%' }"></div>
        </div>
        <div class="line sub">
          <span>
            {{ formatBytes(t.transferred) }}<template v-if="t.total"> / {{ formatBytes(t.total) }}</template>
          </span>
          <span v-if="t.speed">{{ formatBytes(t.speed) }}/s</span>
          <span v-if="t.eta">{{ left(t.total, t.transferred, t.speed) }}</span>
          <span class="spacer"></span>
          <button v-if="t.status === 'active' || t.status === 'queued'" type="button" @click="cancel(t.id)">
            Cancel
          </button>
          <button v-if="t.status === 'failed' || t.status === 'canceled'" type="button" @click="retry(t.id)">
            Retry
          </button>
          <button
            v-if="t.kind === 'download' && t.status === 'done'"
            type="button"
            @click="reveal(t.localPath)"
          >
            Reveal
          </button>
          <button v-if="t.status !== 'active'" type="button" @click="remove(t.id)">Remove</button>
        </div>
        <p v-if="t.error" class="err">{{ t.error }}</p>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  max-height: 45vh;
  border-top: 1px solid #34373f;
  background: #1b1d21;
}
.thead {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-bottom: 1px solid #34373f;
  font-size: 13px;
}
.counts {
  flex: 1;
  color: #8a8f99;
  font-size: 12px;
}
.thead button,
.list button {
  padding: 3px 8px;
  border: 1px solid #3a3d45;
  border-radius: 4px;
  background: #23252b;
  color: #dfe1e6;
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}
.thead button.on {
  border-color: #4a6ea9;
  background: #2c3a55;
}
.none {
  margin: 10px;
  color: #8a8f99;
  font-size: 13px;
}
.list {
  margin: 0;
  padding: 0;
  overflow: auto;
  list-style: none;
}
.list li {
  padding: 8px 10px;
  border-bottom: 1px solid #23252b;
}
.line {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
}
.line.sub {
  margin-top: 4px;
  color: #8a8f99;
  font-size: 12px;
}
.spacer {
  flex: 1;
}
.arrow {
  color: #7aa2f7;
}
.name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.status {
  margin-left: auto;
  color: #8a8f99;
  font-size: 12px;
}
.bar {
  height: 4px;
  margin-top: 6px;
  border-radius: 2px;
  background: #2a2d33;
  overflow: hidden;
}
.fill {
  height: 100%;
  background: #4a6ea9;
  transition: width 0.2s linear;
}
.list li.done .fill {
  background: #4f9d69;
}
.list li.failed .fill,
.list li.canceled .fill {
  background: #a94a4a;
}
.err {
  margin: 4px 0 0;
  color: #ff8a8a;
  font-size: 12px;
}
</style>
