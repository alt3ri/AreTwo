// ponytail: zero-dep self-check for the pure explorer helpers.
// Run: node --experimental-strip-types scripts/rows.check.ts
import assert from "node:assert/strict";
import {
  basename,
  bucketRows,
  extensionOf,
  filterRows,
  formatBytes,
  pageRows,
  parentPrefix,
  previewKind,
  sortRows,
  typeLabel,
  type Row,
} from "../src/rows.ts";

// --- prefix maths ---
assert.equal(parentPrefix("manga/one-piece/001/"), "manga/one-piece/");
assert.equal(parentPrefix("manga/"), "");
assert.equal(parentPrefix(""), "");

// --- basename handles both separators (keys and native Windows paths) ---
assert.equal(basename("manga/001/01.webp"), "01.webp");
assert.equal(basename("C:\\Users\\me\\cover.png"), "cover.png");
assert.equal(basename("top.webp"), "top.webp");

// --- sizes ---
assert.equal(formatBytes(0), "0 B");
assert.equal(formatBytes(512), "512 B");
assert.equal(formatBytes(1536), "1.5 KB");
assert.equal(formatBytes(20 * 1024 * 1024), "20 MB");
assert.equal(formatBytes(-2048), "-2.0 KB");

// --- preview routing ---
assert.equal(previewKind("notes.txt"), "text");
assert.equal(previewKind("cover.WEBP"), "image");
assert.equal(previewKind("archive.zip"), "none");
assert.equal(previewKind("README"), "none");

// --- row building: folders first, then files ---
const page = pageRows({
  folders: [{ prefix: "a/", name: "a" }],
  files: [
    { key: "z.txt", name: "z.txt", size: 10, lastModified: "", etag: "e", storageClass: "STANDARD" },
  ],
  nextToken: null,
  truncated: false,
  folderCount: 1,
  fileCount: 1,
  totalSize: 10,
});
assert.deepEqual(
  page.map((r) => `${r.kind}:${r.name}`),
  ["folder:a", "file:z.txt"],
);
assert.equal(page[1].size, 10);
assert.deepEqual(bucketRows(["zeta", "alpha"]).map((r) => r.bucketName), ["zeta", "alpha"]);

// --- sorting: folders pinned on top, natural numeric order within a group ---
const rows: Row[] = [
  { kind: "file", name: "9.webp", key: "9.webp", prefix: "", size: 90, lastModified: "", storageClass: "" },
  { kind: "file", name: "10.webp", key: "10.webp", prefix: "", size: 100, lastModified: "", storageClass: "" },
  { kind: "folder", name: "zz", key: "zz/", prefix: "zz/", size: 0, lastModified: "", storageClass: "" },
  { kind: "folder", name: "aa", key: "aa/", prefix: "aa/", size: 0, lastModified: "", storageClass: "" },
];

assert.deepEqual(
  sortRows(rows, { key: "name", dir: 1 }).map((r) => r.name),
  ["aa", "zz", "9.webp", "10.webp"],
);
assert.deepEqual(
  sortRows(rows, { key: "name", dir: -1 }).map((r) => r.name),
  ["zz", "aa", "10.webp", "9.webp"],
);
// Folders all report size 0, so only the file group has a defined size order.
const bySize = sortRows(rows, { key: "size", dir: 1 });
assert.deepEqual(bySize.slice(0, 2).map((r) => r.kind), ["folder", "folder"]);
assert.deepEqual(bySize.slice(2).map((r) => r.name), ["9.webp", "10.webp"]);
assert.deepEqual(
  sortRows(rows, { key: "size", dir: -1 }).slice(2).map((r) => r.name),
  ["10.webp", "9.webp"],
);

// --- filtering keeps folders and files, ignores casing and padding ---
assert.deepEqual(
  filterRows(rows, "  WEBP ").map((r) => r.name),
  ["9.webp", "10.webp"],
);
assert.equal(filterRows(rows, "").length, 4);

// --- extension + type labels (the Type column) ---
assert.equal(extensionOf("cover.webp"), "webp");
assert.equal(extensionOf("data.XLSX"), "xlsx");
assert.equal(extensionOf("README"), "");
assert.equal(extensionOf(".env"), "", "a dotfile is not an extension");

assert.equal(typeLabel("cover.webp"), "WebP image");
assert.equal(typeLabel("index.ts"), "TypeScript file");
assert.equal(typeLabel("archive.zip"), "Zip archive");
assert.equal(typeLabel("data.XLSX"), "Excel workbook");
assert.equal(typeLabel("README"), "File");
assert.equal(typeLabel("odd.qqq"), "QQQ file");
assert.equal(typeLabel("manga", "folder"), "Folder");

// --- sorting by the Type column keeps folders pinned and groups labels ---
assert.deepEqual(
  sortRows(rows, { key: "type", dir: 1 }).map((r) => typeLabel(r.name, r.kind)),
  ["Folder", "Folder", "WebP image", "WebP image"],
);

console.log("rows.check: all assertions passed");
