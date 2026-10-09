import { invoke, isTauri } from "@tauri-apps/api/core";
import type { AppLocale } from "../../i18n";

/** A persisted choice; "system" is resolved to a UI locale at startup. */
export type LocalePreference = "system" | AppLocale;

/** Preview has no persisted preference and must not claim desktop success. */
export type LocalePreferenceResult =
    { kind: "preview" } | { kind: "desktop"; preference: LocalePreference };

/** Safe Rust error codes plus failures specific to the frontend IPC boundary. */
export type SettingsErrorCode =
    | "storage_unavailable"
    | "read_failed"
    | "write_failed"
    | "invalid_preference"
    | "operation_failed"
    | "invalid_response"
    | "desktop_required";

/** A sanitized failure. UI code translates `code`, never raw invoke diagnostics. */
export class SettingsRequestError extends Error {
    /** Stable error code; unknown outcomes require reloading before claiming a save. */
    readonly code: SettingsErrorCode;

    /** Create a safe failure without retaining the original runtime error. */
    constructor(code: SettingsErrorCode) {
        super("Language preference request failed.");
        this.name = "SettingsRequestError";
        this.code = code;
    }
}

function isLocalePreference(value: unknown): value is LocalePreference {
    return value === "system" || value === "zh-CN" || value === "en";
}

function sanitizeError(error: unknown): SettingsRequestError {
    switch (error) {
        case "storage_unavailable":
        case "read_failed":
        case "write_failed":
        case "invalid_preference":
        case "operation_failed":
            return new SettingsRequestError(error);
        default:
            return new SettingsRequestError("operation_failed");
    }
}

/**
 * Read the choice through Rust, or identify browser preview without invoking IPC.
 * Reject unsupported/malformed responses and expose only safe error codes.
 */
export async function getLocalePreference(): Promise<LocalePreferenceResult> {
    if (!isTauri()) return { kind: "preview" };
    let data: unknown;
    try {
        data = await invoke<unknown>("get_locale_preference");
    } catch (error: unknown) {
        throw sanitizeError(error);
    }
    if (!isLocalePreference(data)) {
        throw new SettingsRequestError("invalid_response");
    }
    return { kind: "desktop", preference: data };
}

/**
 * Save through Rust, returning the confirmed choice only after acknowledgment.
 * Preview is rejected. Invalid responses or transport/task failures have an
 * unknown persisted outcome: callers must reload rather than claim rollback.
 */
export async function saveLocalePreference(
    preference: LocalePreference,
): Promise<LocalePreference> {
    if (!isTauri()) throw new SettingsRequestError("desktop_required");
    let data: unknown;
    try {
        data = await invoke<unknown>("save_locale_preference", { preference });
    } catch (error: unknown) {
        throw sanitizeError(error);
    }
    if (!isLocalePreference(data) || data !== preference) {
        throw new SettingsRequestError("invalid_response");
    }
    return data;
}
