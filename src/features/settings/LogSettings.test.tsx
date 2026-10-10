import {
    act,
    cleanup,
    fireEvent,
    render,
    screen,
    waitFor,
} from "@testing-library/react";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import {
    getLogLocation,
    LogRequestError,
    openLogDirectory,
    openLogFile,
    type LogLocation,
} from "../../lib/desktop/logs";
import { LogSettings } from "./LogSettings";

vi.mock(import("../../lib/desktop/logs"), async (importOriginal) => ({
    ...(await importOriginal()),
    getLogLocation: vi.fn(),
    openLogFile: vi.fn(),
    openLogDirectory: vi.fn(),
}));
const read = vi.mocked(getLogLocation);
const viewFile = vi.mocked(openLogFile);
const viewDirectory = vi.mocked(openLogDirectory);
const location: LogLocation = {
    filePath: "/home/test user/日志/vibemate.log",
    directoryPath: "/home/test user/日志",
    fileLoggingActive: true,
};

beforeEach(() => {
    read.mockReset().mockResolvedValue(location);
    viewFile.mockReset().mockResolvedValue(undefined);
    viewDirectory.mockReset().mockResolvedValue(undefined);
});
afterEach(cleanup);

function deferred<T>() {
    let resolve: (value: T) => void = () => {
        throw new Error("Not initialized");
    };
    let reject: (reason: Error) => void = () => {
        throw new Error("Not initialized");
    };
    const promise = new Promise<T>((res, rej) => {
        resolve = res;
        reject = rej;
    });
    return { promise, resolve, reject };
}

async function mount(locale: "en" | "zh-CN" = "en") {
    const instance = await createAppI18n(locale);
    const view = render(
        <StrictMode>
            <I18nextProvider i18n={instance}>
                <LogSettings />
            </I18nextProvider>
        </StrictMode>,
    );
    return { instance, ...view };
}

for (const locale of ["en", "zh-CN"] as const) {
    test(`shows actual selectable paths and opens only on explicit activation in ${locale}`, async () => {
        const { instance } = await mount(locale);
        await screen.findByText(location.filePath);
        expect(screen.queryByText(location.directoryPath)).toBeNull();
        expect(viewFile).not.toHaveBeenCalled();
        expect(viewDirectory).not.toHaveBeenCalled();
        fireEvent.click(
            screen.getByRole("button", {
                name: locale === "en" ? "View logs" : "查看日志",
            }),
        );
        await waitFor(() => {
            expect(viewFile).toHaveBeenCalledExactlyOnceWith();
        });
        await waitFor(() => {
            expect(
                screen
                    .getByRole("button", {
                        name: locale === "en" ? "View logs" : "查看日志",
                    })
                    .getAttribute("aria-disabled"),
            ).toBe("false");
        });
        fireEvent.click(
            screen.getByRole("button", {
                name: locale === "en" ? "Open log folder" : "打开日志文件夹",
            }),
        );
        await waitFor(() => {
            expect(viewDirectory).toHaveBeenCalledExactlyOnceWith();
        });
        const calls = read.mock.calls.length;
        await act(async () => {
            await instance.changeLanguage(locale === "en" ? "zh-CN" : "en");
        });
        expect(screen.getByText(location.filePath)).toBeDefined();
        expect(read).toHaveBeenCalledTimes(calls);
    });
}

test("preview invents no path and offers no OS actions", async () => {
    read.mockResolvedValue(null);
    await mount("zh-CN");
    await screen.findByText("浏览器预览无法读取或打开桌面日志。");
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.queryByText(location.filePath)).toBeNull();
    expect(viewFile).not.toHaveBeenCalled();
    expect(viewDirectory).not.toHaveBeenCalled();
});

test("loading blocks actions and sanitizes read failures", async () => {
    const request = deferred<LogLocation | null>();
    read.mockReturnValue(request.promise);
    await mount("zh-CN");
    expect(screen.getByRole("status").textContent).toBe("正在读取日志路径…");
    expect(
        screen.queryByRole("button", { name: "查看日志" }),
    ).toBeNull();
    expect(viewFile).not.toHaveBeenCalled();
    await act(async () => {
        request.reject(new Error("private filesystem details"));
        await request.promise.catch(() => null);
    });
    expect(screen.getByRole("alert").textContent).toBe(
        "日志请求未能完成，请重试。",
    );
    expect(screen.queryByText(/private filesystem/)).toBeNull();
    expect(screen.queryByText(location.filePath)).toBeNull();
});

test("an OS request blocks repeated and competing actions without dropping focus", async () => {
    const request = deferred<undefined>();
    viewFile.mockReturnValue(request.promise);
    await mount();
    await screen.findByText(location.filePath);
    const view = screen.getByRole("button", { name: "View logs" });
    view.focus();
    fireEvent.click(view);
    fireEvent.click(view);
    fireEvent.click(screen.getByRole("button", { name: "Open log folder" }));
    expect(viewFile).toHaveBeenCalledTimes(1);
    expect(viewDirectory).not.toHaveBeenCalled();
    expect(screen.getByRole("status").textContent).toBe(
        "Requesting the text tool…",
    );
    expect(document.activeElement).toBe(view);
    await act(async () => {
        request.resolve(undefined);
        await request.promise;
    });
    expect(view.getAttribute("aria-disabled")).toBe("false");
    expect(screen.queryByRole("status")).toBeNull();
    expect(document.activeElement).toBe(view);
});

test("missing file gives safe feedback while the directory action can still be retried", async () => {
    viewFile.mockRejectedValue(new LogRequestError("log_unavailable"));
    await mount();
    await screen.findByText(location.filePath);
    fireEvent.click(screen.getByRole("button", { name: "View logs" }));
    expect((await screen.findByRole("alert")).textContent).toContain(
        "does not exist or cannot be accessed",
    );
    fireEvent.click(screen.getByRole("button", { name: "Open log folder" }));
    await waitFor(() => {
        expect(screen.queryByRole("alert")).toBeNull();
    });
    expect(viewDirectory).toHaveBeenCalledTimes(1);
});

test("open failure stays translated and retries without exposing OS diagnostics", async () => {
    viewDirectory.mockRejectedValue(new Error("private OS error"));
    await mount("zh-CN");
    await screen.findByText(location.filePath);
    const button = screen.getByRole("button", { name: "打开日志文件夹" });
    fireEvent.click(button);
    expect((await screen.findByRole("alert")).textContent).toBe(
        "日志请求未能完成，请重试。",
    );
    expect(screen.queryByText(/private OS/)).toBeNull();
    viewDirectory.mockResolvedValue(undefined);
    fireEvent.click(button);
    await waitFor(() => {
        expect(screen.queryByRole("alert")).toBeNull();
    });
    expect(viewDirectory).toHaveBeenCalledTimes(2);
});

test("stderr fallback is visible but still allows opening an older file", async () => {
    read.mockResolvedValue({ ...location, fileLoggingActive: false });
    await mount();
    await screen.findByText(location.filePath);
    expect(screen.getByRole("status").textContent).toContain(
        "File logging was not enabled at startup",
    );
    fireEvent.click(screen.getByRole("button", { name: "View logs" }));
    await waitFor(() => {
        expect(viewFile).toHaveBeenCalledTimes(1);
    });
});

for (const outcome of ["success", "failure"] as const) {
    test(`ignores obsolete StrictMode read ${outcome}`, async () => {
        const obsolete = deferred<LogLocation | null>();
        const current = deferred<LogLocation | null>();
        read.mockReturnValueOnce(obsolete.promise).mockReturnValueOnce(
            current.promise,
        );
        await mount();
        await act(async () => {
            current.resolve(location);
            await current.promise;
        });
        await act(async () => {
            if (outcome === "success")
                obsolete.resolve({ ...location, filePath: "/obsolete.log" });
            else obsolete.reject(new Error("obsolete"));
            await obsolete.promise.catch(() => null);
        });
        expect(screen.getByText(location.filePath)).toBeDefined();
        expect(screen.queryByText("/obsolete.log")).toBeNull();
        expect(screen.queryByRole("alert")).toBeNull();
    });
}

test("ignores a late OS acknowledgment after unmount", async () => {
    const request = deferred<undefined>();
    viewFile.mockReturnValue(request.promise);
    const { unmount } = await mount();
    await screen.findByText(location.filePath);
    fireEvent.click(screen.getByRole("button", { name: "View logs" }));
    unmount();
    await act(async () => {
        request.reject(new Error("late failure"));
        await request.promise.catch(() => null);
    });
    expect(screen.queryByRole("alert")).toBeNull();
});
