import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
    getLocalePreference,
    saveLocalePreference,
    type LocalePreference,
} from "./settings";

// Auto-mocking retains invoke's generic signature without casting a mock that
// returns unknown into a function promising an arbitrary caller-selected type.
vi.mock(import("@tauri-apps/api/core"));

beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(isTauri).mockReturnValue(true);
});

describe("language preference desktop boundary", () => {
    it("identifies preview without invoking Rust or claiming a saved default", async () => {
        vi.mocked(isTauri).mockReturnValue(false);
        expect(await getLocalePreference()).toEqual({ kind: "preview" });
        await expect(saveLocalePreference("zh-CN")).rejects.toMatchObject({
            code: "desktop_required",
        });
        expect(invoke).not.toHaveBeenCalled();
    });

    it.each(["system", "zh-CN", "en"] as const)(
        "validates the saved choice %s",
        async (preference: LocalePreference) => {
            vi.mocked(invoke).mockResolvedValue(preference);
            expect(await getLocalePreference()).toEqual({
                kind: "desktop",
                preference,
            });
            expect(invoke).toHaveBeenCalledWith("get_locale_preference");
            expect(await saveLocalePreference(preference)).toBe(preference);
            expect(invoke).toHaveBeenLastCalledWith("save_locale_preference", {
                preference,
            });
        },
    );

    it.each([null, "fr", { preference: "en" }, 1])(
        "rejects malformed read data %j",
        async (response) => {
            vi.mocked(invoke).mockResolvedValue(response);
            await expect(getLocalePreference()).rejects.toMatchObject({
                code: "invalid_response",
            });
        },
    );

    it("does not report success for a mismatched save acknowledgment", async () => {
        vi.mocked(invoke).mockResolvedValue("en");
        await expect(saveLocalePreference("zh-CN")).rejects.toMatchObject({
            code: "invalid_response",
        });
    });

    it.each([
        "storage_unavailable",
        "read_failed",
        "write_failed",
        "invalid_preference",
        "operation_failed",
    ])("retains the known safe error code %s", async (code) => {
        vi.mocked(invoke).mockRejectedValue(code);
        await expect(getLocalePreference()).rejects.toMatchObject({ code });
        await expect(saveLocalePreference("en")).rejects.toMatchObject({
            code,
        });
    });

    it("drops unknown runtime diagnostics instead of forwarding them to the UI", async () => {
        vi.mocked(invoke).mockRejectedValue(
            new Error("Synthetic private path and SQL"),
        );
        try {
            await saveLocalePreference("en");
            throw new Error("Expected a failed save");
        } catch (error: unknown) {
            expect(error).toMatchObject({
                code: "operation_failed",
                message: "Language preference request failed.",
            });
            expect(String(error)).not.toContain("Synthetic private");
        }
    });
});
