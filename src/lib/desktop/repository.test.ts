import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { MetadataRequestError, openProjectRepository } from "../desktop";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
const invokeCommand = vi.mocked(invoke);
const desktopAvailable = vi.mocked(isTauri);
beforeEach(() => {
    vi.resetAllMocks();
});
afterEach(() => {
    vi.restoreAllMocks();
});

test("desktop opens only the fixed repository command and validates its acknowledgment", async () => {
    desktopAvailable.mockReturnValue(true);
    invokeCommand.mockResolvedValueOnce(null);
    await openProjectRepository();
    expect(invokeCommand).toHaveBeenCalledExactlyOnceWith(
        "open_project_repository",
    );
    invokeCommand.mockResolvedValueOnce("invalid");
    await expect(openProjectRepository()).rejects.toMatchObject({
        code: "invalid_response",
    });
});

test("desktop failures expose only stable codes", async () => {
    desktopAvailable.mockReturnValue(true);
    // Rust's documented code is preserved so the UI can rely on it.
    invokeCommand.mockRejectedValueOnce("open_failed");
    await expect(openProjectRepository()).rejects.toMatchObject({
        code: "open_failed",
    });
    // Any other rejection is a transport failure; its details are discarded.
    invokeCommand.mockRejectedValueOnce(new Error("private OS error"));
    const error: unknown = await openProjectRepository().catch(
        (reason: unknown) => reason,
    );
    expect(error).toBeInstanceOf(MetadataRequestError);
    expect(error).toMatchObject({ code: "operation_failed" });
    expect(String(error)).not.toContain("private");
});

test("preview opens the fixed HTTPS repository with no opener access or desktop IPC", async () => {
    desktopAvailable.mockReturnValue(false);
    const open = vi.spyOn(window, "open").mockReturnValue(null);
    await openProjectRepository();
    expect(open).toHaveBeenCalledExactlyOnceWith(
        "https://github.com/Johnny0x38E/vibemate",
        "_blank",
        "noopener,noreferrer",
    );
    expect(invokeCommand).not.toHaveBeenCalled();
});
