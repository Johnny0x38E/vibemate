import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { openProjectRepository } from "../desktop";

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
    await expect(openProjectRepository()).rejects.toThrow(
        "Could not open the project repository.",
    );
    invokeCommand.mockRejectedValueOnce(new Error("private OS error"));
    await expect(openProjectRepository()).rejects.toThrow(
        "Could not open the project repository.",
    );
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
