import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { fileIconName, fileIconNames } from "../src/fileIcons.ts";

for (const [name, expected] of [
  ["README.MD", "readme"], ["LICENSE", "license"], ["docs/guide.md", "markdown"],
  ["photo.JPEG", "image"], ["logo.svg", "svg"], ["archive.tar.gz", "zip"],
  ["report.pdf", "pdf"], ["sheet.xlsx", "table"], ["slides.pptx", "powerpoint"],
  ["song.mp3", "audio"], ["movie.mp4", "video"], ["main.rs", "rust"],
  ["main.py", "python"], ["component.vue", "vue"], ["view.tsx", "react_ts"],
  ["types.d.ts", "typescript-def"], [".env.production", "settings"],
  ["Dockerfile.dev", "docker"], ["package.json", "nodejs"],
  ["pnpm-lock.yaml", "pnpm"], [".gitignore", "git"], ["C:\\docs\\README.md", "readme"],
  ["no-extension", "document"], ["unknown.xyz", "document"],
  ["__proto__", "document"], ["constructor", "document"], ["image.constructor", "document"],
]) {
  assert.equal(fileIconName(name), expected, name);
  assert(existsSync(new URL(`../public/icons/material/${expected}.svg`, import.meta.url)));
}

// Every configured icon must be shipped, including less common extensions.
for (const icon of fileIconNames) {
  const svg = readFileSync(new URL(`../public/icons/material/${icon}.svg`, import.meta.url), "utf8");
  assert(svg.includes("<svg"), `${icon}: invalid SVG`);
  assert(!/<script\b|\bon\w+\s*=|(?:href|src)\s*=/i.test(svg), `${icon}: must be a self-contained image`);
}
console.log(`icons.check: mapping, fallback and ${fileIconNames.length} local SVG assets passed`);
