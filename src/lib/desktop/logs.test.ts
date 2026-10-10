import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";
import { getLogLocation, openLogDirectory, openLogFile } from "./logs";

vi.mock(import("@tauri-apps/api/core"));
const location = {
    filePath: "/home/test user/日志/vibemate.log",
    directoryPath: "/home/test user/日志",
    fileLoggingActive: true,
};

beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(isTauri).mockReturnValue(true);
});

test("preview reads no paths and launches nothing", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(await getLogLocation()).toBeNull();
    await expect(openLogFile()).rejects.toMatchObject({
        code: "desktop_required",
    });
    await expect(openLogDirectory()).rejects.toMatchObject({
        code: "desktop_required",
    });
    expect(invoke).not.toHaveBeenCalled();
});

for (const active of [true, false]) {
    test(`reads actual paths with file logging active=${String(active)}`, async () => {
        vi.mocked(invoke).mockResolvedValue({
            ...location,
            fileLoggingActive: active,
        });
        expect(await getLogLocation()).toEqual({
            ...location,
            fileLoggingActive: active,
        });
        expect(invoke).toHaveBeenCalledExactlyOnceWith("get_log_location");
    });
}

for (const response of [
    null,
    [],
    "path",
    {},
    { ...location, filePath: "" },
    { ...location, directoryPath: " " },
    { ...location, filePath: 3 },
    { ...location, fileLoggingActive: "true" },
    { ...location, extra: true },
]) {
    test(`rejects malformed path metadata ${JSON.stringify(response)}`, async () => {
        vi.mocked(invoke).mockResolvedValue(response);
        await expect(getLogLocation()).rejects.toMatchObject({
            code: "invalid_response",
        });
    });
}

for (const [command, action] of [
    ["open_log_file", openLogFile],
    ["open_log_directory", openLogDirectory],
] as const) {
    test(`${command} passes no path or executable and accepts only null`, async () => {
        vi.mocked(invoke).mockResolvedValue(null);
        await expect(action()).resolves.toBeUndefined();
        expect(invoke).toHaveBeenCalledExactlyOnceWith(command);
        vi.mocked(invoke).mockResolvedValue(true);
        await expect(action()).rejects.toMatchObject({
            code: "invalid_response",
        });
    });
}

for (const code of [
    "path_unavailable",
    "log_unavailable",
    "open_failed",
    "operation_failed",
] as const) {
    test(`retains only safe domain code ${code}`, async () => {
        vi.mocked(invoke).mockRejectedValue(code);
        await expect(getLogLocation()).rejects.toMatchObject({ code });
        await expect(openLogFile()).rejects.toMatchObject({ code });
        await expect(openLogDirectory()).rejects.toMatchObject({ code });
    });
}

test("drops raw OS diagnostics from read and open errors", async () => {
    vi.mocked(invoke).mockRejectedValue(
        new Error("private path and executable details"),
    );
    for (const action of [getLogLocation, openLogFile, openLogDirectory]) {
        await expect(action()).rejects.toMatchObject({
            code: "operation_failed",
            message: "Desktop log request failed.",
        });
    }
});
