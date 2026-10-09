import { invoke, isTauri } from "@tauri-apps/api/core";

/** Rust serializes these fields with camelCase names at the IPC boundary. */
export interface AppInfo {
  name: string;
  version: string;
}

/**
 * Read desktop metadata, or return null in a frontend-only browser preview.
 * Reject malformed runtime responses; TypeScript generics cannot validate JSON.
 */
export async function getAppInfo(): Promise<AppInfo | null> {
  // Vite can preview the UI without a Rust runtime. Avoid a failed IPC call there.
  if (!isTauri()) return null;
  const data = await invoke<unknown>("get_app_info");
  if (
    typeof data !== "object" ||
    data === null ||
    !("name" in data) ||
    typeof data.name !== "string" ||
    !("version" in data) ||
    typeof data.version !== "string"
  ) {
    throw new Error(
      "The desktop runtime returned invalid application metadata.",
    );
  }
  return { name: data.name, version: data.version };
}
