// Wire contract with the Rust backend. Field names match #[serde(rename_all = "camelCase")].

export interface Profile {
  id: string;
  name: string;
  accountId: string;
  accessKeyId: string;
  endpoint?: string | null;
  defaultBucket?: string | null;
  readOnly: boolean;
  createdAt: string;
  lastUsedAt?: string | null;
}

export interface FolderEntry {
  prefix: string;
  name: string;
}

export interface ObjectEntry {
  key: string;
  name: string;
  size: number;
  lastModified: string;
  etag: string;
  storageClass: string;
}

export interface ListPage {
  folders: FolderEntry[];
  files: ObjectEntry[];
  nextToken?: string | null;
  truncated: boolean;
  folderCount: number;
  fileCount: number;
  totalSize: number;
}

export interface ObjectInfo {
  size: number;
  contentType?: string | null;
  etag?: string | null;
  lastModified?: string | null;
  storageClass?: string | null;
  cacheControl?: string | null;
  metadata: Record<string, string>;
}

export interface TextPreview {
  text: string;
  size: number;
  truncated: boolean;
}

export interface FolderStats {
  files: number;
  folders: number;
  size: number;
  scannedAt: string;
}

export interface TransferProgress {
  id: string;
  transferred: number;
  total: number;
}

/** One row of the explorer table: a folder (prefix) or an object (key). */
export type Entry =
  | { kind: "folder"; name: string; prefix: string; key: string; size: 0 }
  | { kind: "file"; name: string; key: string; prefix: string; size: number };
