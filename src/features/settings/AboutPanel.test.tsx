import {
    act,
    cleanup,
    fireEvent,
    render,
    screen,
} from "@testing-library/react";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import { getAppInfo, type AppInfo } from "../../lib/desktop";
import { AboutPanel } from "./AboutPanel";

vi.mock("../../lib/desktop", () => ({ getAppInfo: vi.fn() }));
const readInfo = vi.mocked(getAppInfo);
afterEach(cleanup);
beforeEach(() => {
    readInfo.mockReset();
});

function deferred() {
    let resolve: (value: AppInfo | null) => void = () => {
        throw new Error("Not initialized");
    };
    let reject: (reason: Error) => void = () => {
        throw new Error("Not initialized");
    };
    const promise = new Promise<AppInfo | null>((res, rej) => {
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
                <AboutPanel />
            </I18nextProvider>
        </StrictMode>,
    );
    return { instance, ...view };
}

test("loads runtime metadata and translates without rereading or changing identifiers", async () => {
    const request = deferred();
    readInfo.mockReturnValue(request.promise);
    const { instance } = await mount();
    expect(screen.getByRole("status").textContent).toBe(
        "Reading application information…",
    );
    await act(async () => {
        request.resolve({ name: "vibemate", version: "0.1.0" });
        await request.promise;
    });
    expect(screen.getByText("0.1.0")).toBeDefined();
    expect(
        screen.getByText("https://github.com/Johnny0x38E/vibemate"),
    ).toBeDefined();
    const calls = readInfo.mock.calls.length;
    await act(async () => {
        await instance.changeLanguage("zh-CN");
    });
    expect(screen.getByText("版本")).toBeDefined();
    expect(screen.getByText("0.1.0")).toBeDefined();
    expect(readInfo).toHaveBeenCalledTimes(calls);
});

test("identifies browser preview without inventing a desktop version", async () => {
    readInfo.mockResolvedValue(null);
    await mount("zh-CN");
    expect(
        await screen.findByText("浏览器预览无法读取桌面应用版本。"),
    ).toBeDefined();
    expect(screen.queryByText("0.1.0")).toBeNull();
});

test("sanitizes failures and retries in Chinese", async () => {
    readInfo.mockRejectedValue(new Error("private runtime details"));
    await mount("zh-CN");
    expect((await screen.findByRole("alert")).textContent).toBe(
        "无法读取应用信息，请重试。",
    );
    expect(screen.queryByText(/private runtime/)).toBeNull();
    const request = deferred();
    readInfo.mockReturnValue(request.promise);
    fireEvent.click(screen.getByRole("button", { name: "重新读取应用信息" }));
    expect(screen.queryByRole("button")).toBeNull();
    await act(async () => {
        request.resolve({ name: "vibemate", version: "0.1.0" });
        await request.promise;
    });
    expect(screen.getByText("0.1.0")).toBeDefined();
});

for (const outcome of ["success", "failure"] as const) {
    test(`ignores obsolete StrictMode ${outcome}`, async () => {
        const obsolete = deferred();
        const current = deferred();
        readInfo
            .mockReturnValueOnce(obsolete.promise)
            .mockReturnValueOnce(current.promise);
        await mount();
        await act(async () => {
            current.resolve({ name: "vibemate", version: "0.1.0" });
            await current.promise;
        });
        await act(async () => {
            if (outcome === "success")
                obsolete.resolve({ name: "obsolete", version: "9.9.9" });
            else obsolete.reject(new Error("obsolete failure"));
            await obsolete.promise.catch(() => null);
        });
        expect(screen.getByText("0.1.0")).toBeDefined();
        expect(screen.queryByRole("alert")).toBeNull();
        expect(screen.queryByText("9.9.9")).toBeNull();
    });
}
