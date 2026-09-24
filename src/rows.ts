// Pure row / formatting helpers for the explorer. No Vue, no Tauri, no DOM.
// Kept dependency-free so `scripts/rows.check.ts` can exercise it directly.
import type { ListPage } from "./types";

export type SortKey = "name" | "type" | "size" | "lastModified" | "storageClass";

export interface SortState {
  key: SortKey;
  dir: 1 | -1;
}

/** One table row. Flat on purpose: templates read `row.size` without narrowing a union. */
export interface Row {
  kind: "folder" | "file";
  name: string;
  /** Object key, or folder prefix. Doubles as the row identity. */
  key: string;
  /** Parent prefix the row lives under. */
  prefix: string;
  size: number;
  lastModified: string;
  storageClass: string;
  /** Object ETag, files only (folder rows have none). */
  etag?: string;
  /** Set only while a tab shows the bucket list instead of a bucket's contents. */
  bucketName?: string;
}

export interface MenuItem {
  id: string;
  label: string;
  disabled?: boolean;
  danger?: boolean;
}

/** Fixed row height drives both the CSS and the virtual-window math. */
export const ROW_HEIGHT = 26;

export function bucketRows(buckets: string[]): Row[] {
  return buckets.map((name) => ({
    kind: "folder" as const,
    name,
    key: name,
    prefix: "",
    size: 0,
    lastModified: "",
    storageClass: "",
    bucketName: name,
  }));
}

export function pageRows(page: ListPage): Row[] {
  const folders: Row[] = page.folders.map((f) => ({
    kind: "folder",
    name: f.name,
    key: f.prefix,
    prefix: f.prefix,
    size: 0,
    lastModified: "",
    storageClass: "",
  }));
  const files: Row[] = page.files.map((f) => ({
    kind: "file",
    name: f.name,
    key: f.key,
    prefix: f.key,
    size: f.size,
    lastModified: f.lastModified,
    storageClass: f.storageClass,
    etag: f.etag,
  }));
  return [...folders, ...files];
}

/** Folders stay on top; the sort only orders within each group. */
function sortValue(row: Row, key: SortKey): string | number {
  if (key === "size") return row.size;
  if (key === "type") return typeLabel(row.name, row.kind);
  return row[key];
}

/** Folders stay on top; the sort only orders within each group. */
export function sortRows(rows: Row[], { key, dir }: SortState): Row[] {
  return [...rows].sort((a, b) => {
    if (a.kind !== b.kind) return a.kind === "folder" ? -1 : 1;
    const av = sortValue(a, key);
    const bv = sortValue(b, key);
    const cmp =
      typeof av === "number" && typeof bv === "number"
        ? av - bv
        : String(av).localeCompare(String(bv), undefined, { numeric: true, sensitivity: "base" });
    return cmp * dir;
  });
}

export function filterRows(rows: Row[], needle: string): Row[] {
  const q = needle.trim().toLowerCase();
  return q ? rows.filter((r) => r.name.toLowerCase().includes(q)) : rows;
}

/** "a/b/" -> "a/", "a/" -> "", "" -> "" */
export function parentPrefix(prefix: string): string {
  const trimmed = prefix.replace(/\/+$/, "");
  const i = trimmed.lastIndexOf("/");
  return i < 0 ? "" : trimmed.slice(0, i + 1);
}

export function joinPrefix(prefix: string, name: string): string {
  return prefix + name;
}

/** Handles both key separators and Windows paths from the native file picker. */
export function basename(path: string): string {
  const i = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return i < 0 ? path : path.slice(i + 1);
}

const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"];

export function formatBytes(bytes: number): string {
  if (!bytes) return "0 B";
  let value = Math.abs(bytes);
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit++;
  }
  const shown = unit === 0 ? String(value) : value.toFixed(value < 10 ? 1 : 0);
  return `${bytes < 0 ? "-" : ""}${shown} ${UNITS[unit]}`;
}

export function formatDate(iso: string): string {
  if (!iso) return "";
  const date = new Date(iso);
  return Number.isNaN(date.getTime())
    ? iso
    : date.toLocaleString(undefined, {
        year: "numeric",
        month: "2-digit",
        day: "2-digit",
        hour: "2-digit",
        minute: "2-digit",
      });
}

export type PreviewKind = "text" | "image" | "none";

const IMAGE_EXT = new Set(["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "avif", "svg"]);
const TEXT_EXT = new Set([
  "txt", "md", "markdown", "json", "jsonc", "xml", "yml", "yaml", "toml", "ini", "cfg", "conf",
  "csv", "tsv", "log", "html", "htm", "css", "scss", "js", "mjs", "cjs", "ts", "tsx", "jsx",
  "vue", "rs", "py", "go", "rb", "php", "java", "kt", "c", "h", "cpp", "hpp", "cs", "sh", "ps1",
  "bat", "cmd", "sql", "env", "lock", "gitignore", "dockerfile", "editorconfig",
]);

export function previewKind(name: string): PreviewKind {
  const dot = name.lastIndexOf(".");
  const ext = (dot < 0 ? name : name.slice(dot + 1)).toLowerCase();
  if (IMAGE_EXT.has(ext)) return "image";
  return TEXT_EXT.has(ext) ? "text" : "none";
}

/** Extension without the dot, lowercased. "" when the name has none. */
export function extensionOf(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot <= 0 ? "" : name.slice(dot + 1).toLowerCase();
}

const TYPE_NAMES: Record<string, string> = {
  png: "PNG image",
  jpg: "JPEG image",
  jpeg: "JPEG image",
  gif: "GIF image",
  webp: "WebP image",
  avif: "AVIF image",
  bmp: "Bitmap image",
  svg: "SVG image",
  ico: "Icon",
  pdf: "PDF document",
  csv: "CSV file",
  tsv: "TSV file",
  json: "JSON file",
  xml: "XML file",
  yml: "YAML file",
  yaml: "YAML file",
  toml: "TOML file",
  md: "Markdown",
  txt: "Text file",
  log: "Log file",
  html: "HTML file",
  css: "CSS file",
  js: "JavaScript file",
  mjs: "JavaScript file",
  cjs: "JavaScript file",
  ts: "TypeScript file",
  tsx: "TSX file",
  jsx: "JSX file",
  vue: "Vue component",
  rs: "Rust source",
  py: "Python source",
  go: "Go source",
  php: "PHP source",
  rb: "Ruby source",
  java: "Java source",
  kt: "Kotlin source",
  c: "C source",
  h: "C header",
  cpp: "C++ source",
  cs: "C# source",
  sh: "Shell script",
  ps1: "PowerShell script",
  bat: "Batch file",
  sql: "SQL file",
  zip: "Zip archive",
  rar: "RAR archive",
  "7z": "7z archive",
  gz: "Gzip archive",
  tar: "Tar archive",
  mp3: "MP3 audio",
  wav: "WAV audio",
  flac: "FLAC audio",
  mp4: "MP4 video",
  webm: "WebM video",
  mov: "QuickTime video",
  mkv: "Matroska video",
  docx: "Word document",
  xlsx: "Excel workbook",
  pptx: "PowerPoint deck",
  epub: "EPUB book",
};

/** Explorer-style type column: "WebP image", "TypeScript file", "File", "Folder". */
export function typeLabel(name: string, kind: Row["kind"] = "file"): string {
  if (kind === "folder") return "Folder";
  return TYPE_NAMES[extensionOf(name)] ?? (extensionOf(name) ? `${extensionOf(name).toUpperCase()} file` : "File");
}
