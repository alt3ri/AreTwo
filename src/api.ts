// Thin typed wrappers over the Tauri command surface. No business logic here.
import { invoke } from "@tauri-apps/api/core";
import type {
  FolderStats,
  ListPage,
  ObjectInfo,
  Profile,
  TextPreview,
} from "./types";

export const listProfiles = () => invoke<Profile[]>("list_profiles");

export const saveProfile = (profile: Profile, secret?: string) =>
  invoke<void>("save_profile", { profile, secret: secret ?? null });

export const deleteProfile = (id: string) => invoke<void>("delete_profile", { id });

export const listBuckets = (profileId: string) =>
  invoke<string[]>("list_buckets", { profileId });

export const listObjects = (profileId: string, bucket: string, prefix: string, token?: string) =>
  invoke<ListPage>("list_objects", { profileId, bucket, prefix, token: token ?? null });

export const headObject = (profileId: string, bucket: string, key: string) =>
  invoke<ObjectInfo>("head_object", { profileId, bucket, key });

export const readText = (profileId: string, bucket: string, key: string, maxBytes = 2_000_000) =>
  invoke<TextPreview>("read_text", { profileId, bucket, key, maxBytes });

export const readImage = (profileId: string, bucket: string, key: string, maxBytes = 25_000_000) =>
  invoke<string>("read_image", { profileId, bucket, key, maxBytes });

export const presignGet = (profileId: string, bucket: string, key: string, expiresSecs: number) =>
  invoke<string>("presign_get", { profileId, bucket, key, expiresSecs });

export const folderStats = (profileId: string, bucket: string, prefix: string) =>
  invoke<FolderStats>("folder_stats", { profileId, bucket, prefix });

export const testConnection = (profileId: string) =>
  invoke<string>("test_connection", { profileId });

export const revealInExplorer = (path: string) =>
  invoke<void>("reveal_in_explorer", { path });

export const uploadObject = (
  profileId: string,
  bucket: string,
  key: string,
  path: string,
  transferId: string,
  replace = false,
) => invoke<void>("upload_object", { profileId, bucket, key, path, transferId, replace });

export const downloadObject = (
  profileId: string,
  bucket: string,
  key: string,
  destPath: string,
  transferId: string,
) => invoke<void>("download_object", { profileId, bucket, key, destPath, transferId });

export const cancelTransfer = (transferId: string) =>
  invoke<void>("cancel_transfer", { transferId });

export const deleteObjects = (profileId: string, bucket: string, keys: string[]) =>
  invoke<void>("delete_objects", { profileId, bucket, keys });

export const deletePrefix = (profileId: string, bucket: string, prefix: string) =>
  invoke<number>("delete_prefix", { profileId, bucket, prefix });

export const copyObject = (
  profileId: string,
  bucket: string,
  fromKey: string,
  toKey: string,
  replace = false,
) => invoke<void>("copy_object", { profileId, bucket, fromKey, toKey, replace });

export const copyPrefix = (
  profileId: string,
  bucket: string,
  fromPrefix: string,
  toPrefix: string,
  replace = false,
) => invoke<number>("copy_prefix", { profileId, bucket, fromPrefix, toPrefix, replace });

export const createFolder = (profileId: string, bucket: string, prefix: string) =>
  invoke<void>("create_folder", { profileId, bucket, prefix });

export const createFile = (profileId: string, bucket: string, key: string) =>
  invoke<void>("create_file", { profileId, bucket, key });
