import { invoke, isTauri } from "@tauri-apps/api/core";

/** Rust serializes these fields with camelCase names at the IPC boundary. */
export interface AppInfo {
  name: string;
  version: string;
}

/** Read desktop metadata, or return null in a frontend-only browser preview. */
export async function getAppInfo(): Promise<AppInfo | null> {
  // Vite can preview the UI without a Rust runtime. Avoid a failed IPC call there.
  if (!isTauri()) return null;
  return invoke<AppInfo>("get_app_info");
}
