import { basename, extensionOf } from "./rows.ts";

// Vendored Material Icon Theme SVGs; see public/icons/material/README.md.
const groups: Record<string, string[]> = {
  image: ["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "avif", "tif", "tiff", "heic"],
  svg: ["svg"],
  pdf: ["pdf"],
  zip: ["zip", "rar", "7z", "gz", "gzip", "tar", "bz2", "xz", "tgz"],
  audio: ["mp3", "wav", "flac", "aac", "ogg", "m4a", "opus"],
  video: ["mp4", "webm", "mov", "mkv", "avi", "m4v"],
  word: ["doc", "docx", "odt", "rtf"],
  table: ["csv", "tsv", "xls", "xlsx", "ods"],
  powerpoint: ["ppt", "pptx", "odp"],
  epub: ["epub"],
  font: ["woff", "woff2", "ttf", "otf"],
  document: ["txt", "text"],
  markdown: ["md", "markdown", "mdx"],
  json: ["json", "jsonc", "json5", "webmanifest", "ipynb"],
  yaml: ["yaml", "yml"],
  toml: ["toml"],
  xml: ["xml", "xsl", "xsd"],
  html: ["html", "htm", "xhtml"],
  css: ["css"],
  sass: ["sass", "scss"],
  less: ["less"],
  javascript: ["js", "mjs", "cjs"],
  typescript: ["ts", "mts", "cts"],
  react: ["jsx"],
  react_ts: ["tsx"],
  vue: ["vue"],
  rust: ["rs"],
  python: ["py", "pyw", "pyi"],
  go: ["go"],
  ruby: ["rb"],
  php: ["php"],
  java: ["java", "class", "jar"],
  kotlin: ["kt", "kts"],
  c: ["c", "h"],
  cpp: ["cpp", "cc", "cxx", "hpp", "hxx"],
  csharp: ["cs"],
  console: ["sh", "bash", "zsh", "fish", "bat", "cmd"],
  powershell: ["ps1", "psm1", "psd1"],
  database: ["sql", "db", "sqlite", "sqlite3"],
  settings: ["ini", "cfg", "conf", "config", "env"],
  log: ["log"],
  lock: ["lock"],
  exe: ["exe", "msi", "dll"],
};

const extensions = new Map(Object.entries(groups).flatMap(([icon, values]) =>
  values.map((ext) => [ext, icon] as const),
));
const filenames = new Map([
  ["readme", "readme"], ["readme.md", "readme"], ["readme.txt", "readme"],
  ["license", "license"], ["license.md", "license"], ["license.txt", "license"],
  ["copying", "license"], ["dockerfile", "docker"], ["containerfile", "docker"],
  [".dockerignore", "docker"], ["compose.yaml", "docker"], ["compose.yml", "docker"],
  ["docker-compose.yml", "docker"], ["docker-compose.yaml", "docker"],
  [".gitignore", "git"], [".gitattributes", "git"], [".gitmodules", "git"],
  [".editorconfig", "settings"], [".env", "settings"],
  ["package.json", "nodejs"], ["package-lock.json", "npm"], [".npmrc", "npm"],
  ["pnpm-lock.yaml", "pnpm"], ["pnpm-workspace.yaml", "pnpm"],
]);

export const fileIconNames = [...new Set([...extensions.values(), ...filenames.values(), "typescript-def"])];

export function fileIconName(path: string): string {
  const name = basename(path).toLowerCase();
  if (name.endsWith(".d.ts") || name.endsWith(".d.mts") || name.endsWith(".d.cts")) return "typescript-def";
  if (name.startsWith(".env.")) return "settings";
  if (name.startsWith("dockerfile.")) return "docker";
  return filenames.get(name) ?? extensions.get(extensionOf(name)) ?? "document";
}
