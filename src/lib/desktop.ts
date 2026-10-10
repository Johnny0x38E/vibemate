import { invoke, isTauri } from "@tauri-apps/api/core";

/** Rust serializes these fields with camelCase names at the IPC boundary. */
export interface AppInfo {
    name: string;
    version: string;
}

/**
 * Safe failure codes for application metadata and the repository action.
 * `open_failed` is the code Rust returns when the OS cannot open the browser;
 * the others describe failures detected at this frontend boundary.
 */
export type MetadataErrorCode =
    "open_failed" | "operation_failed" | "invalid_response";

/**
 * A sanitized failure from this module's two desktop requests: reading the
 * application info (`getAppInfo`) and opening the project repository link
 * (`openProjectRepository`). UI code may branch on `code` and shows only
 * bundled translations; the original runtime error is intentionally discarded.
 */
export class MetadataRequestError extends Error {
    /** Stable error code; never contains paths or platform diagnostics. */
    readonly code: MetadataErrorCode;

    /** Create a safe failure without retaining the original runtime error. */
    constructor(code: MetadataErrorCode) {
        super("Desktop metadata request failed.");
        this.name = "MetadataRequestError";
        this.code = code;
    }
}

/**
 * Read desktop metadata, or return null in a frontend-only browser preview.
 * Rejects with `MetadataRequestError`: `invalid_response` for malformed data,
 * `operation_failed` when IPC itself fails. TypeScript generics cannot
 * validate JSON received at runtime, so the fields are checked here.
 */
export async function getAppInfo(): Promise<AppInfo | null> {
    // Vite can preview the UI without a Rust runtime. Avoid a failed IPC call there.
    if (!isTauri()) return null;
    let data: unknown;
    try {
        data = await invoke<unknown>("get_app_info");
    } catch {
        // The Rust command cannot fail, so any rejection is a transport problem.
        throw new MetadataRequestError("operation_failed");
    }
    if (
        typeof data !== "object" ||
        data === null ||
        !("name" in data) ||
        typeof data.name !== "string" ||
        !("version" in data) ||
        typeof data.version !== "string"
    ) {
        throw new MetadataRequestError("invalid_response");
    }
    return { name: data.name, version: data.version };
}

/**
 * Open the fixed project repository. Rust owns desktop browser launching;
 * browser preview opens a new tab without exposing the opener window.
 * Rejects with `MetadataRequestError`: `open_failed` from Rust,
 * `invalid_response` for an unexpected acknowledgment, otherwise `operation_failed`.
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
    let response: unknown;
    try {
        response = await invoke<unknown>("open_project_repository");
    } catch (error: unknown) {
        // Only the documented Rust code is kept; anything else is a transport or
        // task failure whose details must not reach the interface.
        throw new MetadataRequestError(
            error === "open_failed" ? "open_failed" : "operation_failed",
        );
    }
    if (response !== null) throw new MetadataRequestError("invalid_response");
}
