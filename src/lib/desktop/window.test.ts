import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
    closeWindow,
    isWindowMaximized,
    minimizeWindow,
    toggleMaximizeWindow,
    usesCustomWindowChrome,
} from "./window";

vi.mock(import("@tauri-apps/api/core"));
vi.mock(import("@tauri-apps/api/window"));

const windowMock = {
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
    isMaximized: vi.fn(),
    onResized: vi.fn(),
};

beforeEach(() => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(getCurrentWindow).mockReturnValue(
        windowMock as unknown as ReturnType<typeof getCurrentWindow>,
    );
    windowMock.minimize.mockReset();
    windowMock.toggleMaximize.mockReset();
    windowMock.close.mockReset();
    windowMock.isMaximized.mockReset().mockResolvedValue(false);
    windowMock.onResized.mockReset().mockResolvedValue(() => undefined);
    vi.stubEnv("TAURI_ENV_PLATFORM", "darwin");
});

describe("usesCustomWindowChrome", () => {
    it("is false in browser preview", () => {
        vi.mocked(isTauri).mockReturnValue(false);
        vi.stubEnv("TAURI_ENV_PLATFORM", "windows");
        expect(usesCustomWindowChrome()).toBe(false);
    });

    it.each(["windows", "linux"] as const)(
        "is true on %s desktop builds",
        (platform) => {
            vi.stubEnv("TAURI_ENV_PLATFORM", platform);
            expect(usesCustomWindowChrome()).toBe(true);
        },
    );

    it("is false on macOS desktop builds", () => {
        vi.stubEnv("TAURI_ENV_PLATFORM", "darwin");
        expect(usesCustomWindowChrome()).toBe(false);
    });
});

describe("window commands", () => {
    it("no-op outside custom chrome platforms", async () => {
        vi.stubEnv("TAURI_ENV_PLATFORM", "darwin");
        await minimizeWindow();
        await toggleMaximizeWindow();
        await closeWindow();
        expect(windowMock.minimize).not.toHaveBeenCalled();
    });

    it("forwards minimize, toggle, and close on Windows builds", async () => {
        vi.stubEnv("TAURI_ENV_PLATFORM", "windows");
        await minimizeWindow();
        await toggleMaximizeWindow();
        await closeWindow();
        expect(windowMock.minimize).toHaveBeenCalled();
        expect(windowMock.toggleMaximize).toHaveBeenCalled();
        expect(windowMock.close).toHaveBeenCalled();
    });

    it("reads maximized state only on custom chrome builds", async () => {
        vi.stubEnv("TAURI_ENV_PLATFORM", "linux");
        windowMock.isMaximized.mockResolvedValue(true);
        await expect(isWindowMaximized()).resolves.toBe(true);
        vi.stubEnv("TAURI_ENV_PLATFORM", "darwin");
        await expect(isWindowMaximized()).resolves.toBe(false);
    });
});
