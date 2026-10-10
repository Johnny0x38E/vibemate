import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";
import {
    getAppearancePreference,
    saveAppearancePreference,
} from "./appearance";

vi.mock(import("@tauri-apps/api/core"));
beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(isTauri).mockReturnValue(true);
});

test("preview never invents a saved choice or invokes a mutation", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(await getAppearancePreference()).toEqual({ kind: "preview" });
    await expect(
        saveAppearancePreference({ appearance: "dark", theme: "iris" }),
    ).rejects.toMatchObject({ code: "desktop_required" });
    expect(invoke).not.toHaveBeenCalled();
});

test("reads and confirms the exact desktop pair", async () => {
    const preference = { appearance: "dark", theme: "iris" } as const;
    vi.mocked(invoke).mockResolvedValue(preference);
    expect(await getAppearancePreference()).toEqual({
        kind: "desktop",
        preference,
    });
    expect(await saveAppearancePreference(preference)).toEqual(preference);
    expect(invoke).toHaveBeenLastCalledWith("save_appearance_preference", {
        preference,
    });
});

for (const response of [
    null,
    { appearance: "dark" },
    { appearance: "automatic", theme: "forest" },
    { appearance: "light", theme: "unknown" },
]) {
    test(`rejects malformed appearance response ${JSON.stringify(response)}`, async () => {
        vi.mocked(invoke).mockResolvedValue(response);
        await expect(getAppearancePreference()).rejects.toMatchObject({
            code: "invalid_response",
        });
    });
}

test("mismatched acknowledgments are unknown outcomes, not successful saves", async () => {
    vi.mocked(invoke).mockResolvedValue({
        appearance: "light",
        theme: "forest",
    });
    await expect(
        saveAppearancePreference({ appearance: "dark", theme: "iris" }),
    ).rejects.toMatchObject({ code: "invalid_response" });
});

test("retains safe failures but discards unknown private diagnostics", async () => {
    vi.mocked(invoke).mockRejectedValue("write_failed");
    await expect(
        saveAppearancePreference({ appearance: "dark", theme: "iris" }),
    ).rejects.toMatchObject({ code: "write_failed" });
    vi.mocked(invoke).mockRejectedValue(new Error("private SQL and path"));
    await expect(getAppearancePreference()).rejects.toMatchObject({
        code: "operation_failed",
        message: "Appearance preference request failed.",
    });
});
