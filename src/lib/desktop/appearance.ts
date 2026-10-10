import { invoke, isTauri } from "@tauri-apps/api/core";
import type { SettingsErrorCode } from "./settings";

/** Brightness is independent of the selected light/dark palette pair. */
export type Appearance = "system" | "light" | "dark";
/** Stable palette IDs also used by bundled CSS and translation keys. */
export const THEME_IDS = [
    "forest",
    "notion",
    "graphite",
    "linen",
    "iris",
    "ocean",
] as const;
/** An installed built-in palette, never an arbitrary CSS value. */
export type ThemeId = (typeof THEME_IDS)[number];
/** Rust commits brightness and palette together. */
export interface AppearancePreference {
    appearance: Appearance;
    theme: ThemeId;
}
/** Preview can try a palette locally but has no saved desktop preference. */
export type AppearancePreferenceResult =
    { kind: "preview" } | { kind: "desktop"; preference: AppearancePreference };

/** Sanitized request failure; unknown save outcomes require a reload. */
export class AppearanceRequestError extends Error {
    readonly code: SettingsErrorCode;
    constructor(code: SettingsErrorCode) {
        super("Appearance preference request failed.");
        this.name = "AppearanceRequestError";
        this.code = code;
    }
}

/** Narrow a UI or IPC value to a supported brightness policy. */
export function isAppearance(value: unknown): value is Appearance {
    return value === "system" || value === "light" || value === "dark";
}
/** Narrow a UI or IPC value to a bundled palette ID. */
export function isThemeId(value: unknown): value is ThemeId {
    return THEME_IDS.some((theme) => theme === value);
}
function validatePreference(value: unknown): AppearancePreference {
    if (
        typeof value !== "object" ||
        value === null ||
        !("appearance" in value) ||
        !("theme" in value) ||
        !isAppearance(value.appearance) ||
        !isThemeId(value.theme)
    ) {
        throw new AppearanceRequestError("invalid_response");
    }
    return { appearance: value.appearance, theme: value.theme };
}
function sanitize(error: unknown): AppearanceRequestError {
    switch (error) {
        case "storage_unavailable":
        case "read_failed":
        case "write_failed":
        case "invalid_preference":
        case "operation_failed":
            return new AppearanceRequestError(error);
        default:
            return new AppearanceRequestError("operation_failed");
    }
}

/** Read validated desktop preferences, identifying preview without invoking IPC. */
export async function getAppearancePreference(): Promise<AppearancePreferenceResult> {
    if (!isTauri()) return { kind: "preview" };
    let data: unknown;
    try {
        data = await invoke<unknown>("get_appearance_preference");
    } catch (error: unknown) {
        throw sanitize(error);
    }
    return { kind: "desktop", preference: validatePreference(data) };
}

/** Save through Rust; only an exact validated acknowledgment confirms the pair. */
export async function saveAppearancePreference(
    preference: AppearancePreference,
): Promise<AppearancePreference> {
    if (!isTauri()) throw new AppearanceRequestError("desktop_required");
    let data: unknown;
    try {
        data = await invoke<unknown>("save_appearance_preference", {
            preference,
        });
    } catch (error: unknown) {
        throw sanitize(error);
    }
    const saved = validatePreference(data);
    if (
        saved.appearance !== preference.appearance ||
        saved.theme !== preference.theme
    )
        throw new AppearanceRequestError("invalid_response");
    return saved;
}
