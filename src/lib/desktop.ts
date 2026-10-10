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

/**
 * Open the fixed project repository. Rust owns desktop browser launching;
 * browser preview opens a new tab without exposing the opener window.
 */
export async function openProjectRepository(): Promise<void> {
    if (!isTauri()) {
        window.open(
            "https://github.com/Johnny0x38E/vibemate",
            "_blank",
            "noopener,noreferrer",
        );
        return;
    }
    try {
        const response = await invoke<unknown>("open_project_repository");
        if (response !== null) throw new Error("Invalid repository response.");
    } catch {
        throw new Error("Could not open the project repository.");
    }
}
