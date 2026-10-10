import { invoke, isTauri } from "@tauri-apps/api/core";

/** Actual Rust-owned paths, with the file logger's startup outcome. */
export interface LogLocation {
    filePath: string;
    directoryPath: string;
    fileLoggingActive: boolean;
}

/** Safe domain failures and desktop-boundary validation/transport failures. */
export type LogErrorCode =
    | "path_unavailable"
    | "log_unavailable"
    | "open_failed"
    | "operation_failed"
    | "invalid_response"
    | "desktop_required";

/** A sanitized failure; never retains OS errors, paths or command diagnostics. */
export class LogRequestError extends Error {
    /** Stable code for localization, without the original platform error. */
    readonly code: LogErrorCode;

    /** Keep only a stable code for translated UI feedback. */
    constructor(code: LogErrorCode) {
        super("Desktop log request failed.");
        this.name = "LogRequestError";
        this.code = code;
    }
}

function sanitizeError(error: unknown): LogRequestError {
    switch (error) {
        case "path_unavailable":
        case "log_unavailable":
        case "open_failed":
        case "operation_failed":
            return new LogRequestError(error);
        default:
            return new LogRequestError("operation_failed");
    }
}

/** Read actual log paths, or return null in browser preview without IPC. */
export async function getLogLocation(): Promise<LogLocation | null> {
    if (!isTauri()) return null;
    let data: unknown;
    try {
        data = await invoke<unknown>("get_log_location");
    } catch (error: unknown) {
        throw sanitizeError(error);
    }
    if (
        typeof data !== "object" ||
        data === null ||
        Object.keys(data).length !== 3 ||
        !("filePath" in data) ||
        typeof data.filePath !== "string" ||
        data.filePath.trim() === "" ||
        !("directoryPath" in data) ||
        typeof data.directoryPath !== "string" ||
        data.directoryPath.trim() === "" ||
        !("fileLoggingActive" in data) ||
        typeof data.fileLoggingActive !== "boolean"
    ) {
        throw new LogRequestError("invalid_response");
    }
    return {
        filePath: data.filePath,
        directoryPath: data.directoryPath,
        fileLoggingActive: data.fileLoggingActive,
    };
}

async function requestOpen(
    command: "open_log_file" | "open_log_directory",
): Promise<void> {
    if (!isTauri()) throw new LogRequestError("desktop_required");
    let data: unknown;
    try {
        // No path or executable is sent: Rust determines both internally.
        data = await invoke<unknown>(command);
    } catch (error: unknown) {
        throw sanitizeError(error);
    }
    if (data !== null) throw new LogRequestError("invalid_response");
}

/** Request a text tool for the fixed log file; resolves on OS dispatch only. */
export async function openLogFile(): Promise<void> {
    await requestOpen("open_log_file");
}

/** Request the system file manager for the fixed app log directory. */
export async function openLogDirectory(): Promise<void> {
    await requestOpen("open_log_directory");
}
