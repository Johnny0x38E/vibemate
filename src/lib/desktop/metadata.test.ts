import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getAppInfo, MetadataRequestError } from "../desktop";

// Auto-mocking keeps invoke's generic signature; only the desktop core is replaced.
vi.mock(import("@tauri-apps/api/core"));

beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(isTauri).mockReturnValue(true);
});

describe("application metadata desktop boundary", () => {
    it("identifies preview without invoking Rust or inventing a version", async () => {
        vi.mocked(isTauri).mockReturnValue(false);
        expect(await getAppInfo()).toBeNull();
        expect(invoke).not.toHaveBeenCalled();
    });

    it("returns only the validated name and version", async () => {
        vi.mocked(invoke).mockResolvedValue({
            name: "vibemate",
            version: "0.1.0",
            extra: "ignored",
        });
        expect(await getAppInfo()).toEqual({
            name: "vibemate",
            version: "0.1.0",
        });
        expect(invoke).toHaveBeenCalledExactlyOnceWith("get_app_info");
    });

    it.each([
        ["null", null],
        ["a string", "0.1.0"],
        ["a missing version", { name: "vibemate" }],
        ["a numeric version", { name: "vibemate", version: 1 }],
    ])("rejects %s as an invalid response", async (_label, response) => {
        vi.mocked(invoke).mockResolvedValue(response);
        const failure = getAppInfo();
        await expect(failure).rejects.toBeInstanceOf(MetadataRequestError);
        await expect(failure).rejects.toMatchObject({
            code: "invalid_response",
        });
    });

    it("replaces IPC rejections with a code and drops the private diagnostic", async () => {
        vi.mocked(invoke).mockRejectedValue(
            new Error("private /Users/someone/runtime detail"),
        );
        const error: unknown = await getAppInfo().catch(
            (reason: unknown) => reason,
        );
        expect(error).toBeInstanceOf(MetadataRequestError);
        expect(error).toMatchObject({ code: "operation_failed" });
        expect(String(error)).not.toContain("private");
        expect(error).not.toHaveProperty("cause");
    });
});
