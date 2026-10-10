import { StrictMode, type JSX, type ReactNode } from "react";
import {
    act,
    cleanup,
    fireEvent,
    render,
    screen,
} from "@testing-library/react";
import * as i18next from "i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import {
    getLocalePreference,
    saveLocalePreference,
    type LocalePreferenceResult,
} from "../../lib/desktop/settings";
import { LocaleStartup as StartupBoundary } from "./LocaleStartup";
import { fieldSelectValue, setFieldSelectValue } from "../../test/fieldSelect";
import { LanguageSelector } from "./LanguageSelector";

// Compose a real consumer of the ready snapshot. Startup itself does not own
// the selector's position; these tests deliberately place it beside a real input.
function LocaleStartup({
    children,
    systemLanguage,
}: {
    children: ReactNode;
    systemLanguage: string;
}): JSX.Element {
    return (
        <StartupBoundary systemLanguage={systemLanguage}>
            {(snapshot) => (
                <>
                    <LanguageSelector {...snapshot} />
                    {children}
                </>
            )}
        </StartupBoundary>
    );
}

// Keep the startup gate, selector, React, and translator real; replace only IPC.
vi.mock(import("../../lib/desktop/settings"), async (importOriginal) => ({
    ...(await importOriginal()),
    getLocalePreference: vi.fn<typeof getLocalePreference>(),
    saveLocalePreference: vi.fn<typeof saveLocalePreference>(),
}));

// ESM exports cannot be spied on in place. Wrap the external factory while
// retaining its real implementation and all other i18next exports.
vi.mock(import("i18next"), async (importOriginal) => {
    const actual = await importOriginal();
    return { ...actual, createInstance: vi.fn(actual.createInstance) };
});

beforeEach(() => {
    vi.mocked(i18next.createInstance).mockClear();
    vi.mocked(getLocalePreference).mockReset();
    vi.mocked(saveLocalePreference).mockReset();
    document.documentElement.lang = "en";
});
afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
});

function pendingPreference() {
    let resolveResponse: (result: LocalePreferenceResult) => void = () => {
        throw new Error("The pending response has not been initialized.");
    };
    const promise = new Promise<LocalePreferenceResult>((resolve) => {
        resolveResponse = resolve;
    });
    return { promise, resolve: resolveResponse };
}

test("withholds application content until a saved English choice overrides a Chinese system", async () => {
    const response = pendingPreference();
    vi.mocked(getLocalePreference).mockReturnValue(response.promise);
    render(
        <LocaleStartup systemLanguage="zh-TW">
            <input aria-label="Unsubmitted note" defaultValue="Draft" />
        </LocaleStartup>,
    );
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(screen.getByRole("status").textContent).toBe("正在读取语言偏好…");
    await act(async () => {
        response.resolve({ kind: "desktop", preference: "en" });
        await response.promise;
    });
    expect(
        fieldSelectValue(
            await screen.findByRole("combobox", { name: "Language" }),
        ),
    ).toBe("en");
    expect(document.documentElement.lang).toBe("en");
    expect(screen.getByRole("textbox")).toHaveProperty("value", "Draft");
    expect(saveLocalePreference).not.toHaveBeenCalled();
});

test.each([
    ["en-US", "zh-CN", "语言", "zh-CN"],
    ["zh-Hant-HK", "system", "语言", "zh-CN"],
    ["fr-FR", "system", "Language", "en"],
] as const)(
    "resolves %s with saved %s before opening the app",
    async (systemLanguage, preference, label, language) => {
        vi.mocked(getLocalePreference).mockResolvedValue({
            kind: "desktop",
            preference,
        });
        render(
            <LocaleStartup systemLanguage={systemLanguage}>
                <input aria-label="Note" />
            </LocaleStartup>,
        );
        expect(
            fieldSelectValue(
                await screen.findByRole("combobox", { name: label }),
            ),
        ).toBe(preference);
        expect(document.documentElement.lang).toBe(language);
        expect(screen.getByRole("textbox")).toBeDefined();
        expect(saveLocalePreference).not.toHaveBeenCalled();
    },
);

test("opens preview in the system language without allowing persistence", async () => {
    vi.mocked(getLocalePreference).mockResolvedValue({ kind: "preview" });
    render(
        <LocaleStartup systemLanguage="zh-TW">
            <input aria-label="Note" />
        </LocaleStartup>,
    );
    expect(
        await screen.findByRole("combobox", { name: "语言" }),
    ).toHaveProperty("disabled", true);
    expect(document.documentElement.lang).toBe("zh-CN");
    expect(screen.getByText("浏览器预览无法保存语言偏好。")).toBeDefined();
    expect(saveLocalePreference).not.toHaveBeenCalled();
});

test("blocks the app after a read failure and retries without saving a default", async () => {
    vi.mocked(getLocalePreference).mockRejectedValueOnce(
        new Error("Synthetic private diagnostic"),
    );
    const response = pendingPreference();
    vi.mocked(getLocalePreference).mockReturnValueOnce(response.promise);
    render(
        <LocaleStartup systemLanguage="en">
            <input aria-label="Note" />
        </LocaleStartup>,
    );
    expect((await screen.findByRole("alert")).textContent).toBe(
        "The language preference could not be read. Retry before opening the app.",
    );
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(screen.queryByText("Synthetic private diagnostic")).toBeNull();
    const retry = screen.getByRole("button", { name: "Retry startup" });
    fireEvent.click(retry);
    fireEvent.click(retry);
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(screen.getByRole("status").textContent).toBe(
        "Reading language preference…",
    );
    await act(async () => {
        response.resolve({ kind: "desktop", preference: "zh-CN" });
        await response.promise;
    });
    expect(
        fieldSelectValue(await screen.findByRole("combobox", { name: "语言" })),
    ).toBe("zh-CN");
    expect(getLocalePreference).toHaveBeenCalledTimes(2);
    expect(saveLocalePreference).not.toHaveBeenCalled();
});

test("distinguishes translator initialization failure from a failed preference read", async () => {
    vi.mocked(getLocalePreference).mockResolvedValue({
        kind: "desktop",
        preference: "en",
    });
    // Fail only the external i18next service's first initialization; retry is real.
    vi.mocked(i18next.createInstance).mockImplementationOnce(() => {
        throw new Error("Synthetic translator diagnostic");
    });
    render(
        <LocaleStartup systemLanguage="zh">
            <input aria-label="Note" />
        </LocaleStartup>,
    );
    expect((await screen.findByRole("alert")).textContent).toBe(
        "无法准备界面语言，请重试后再进入应用。",
    );
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(screen.queryByText("Synthetic translator diagnostic")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "重试启动" }));
    expect(
        fieldSelectValue(
            await screen.findByRole("combobox", { name: "Language" }),
        ),
    ).toBe("en");
    expect(saveLocalePreference).not.toHaveBeenCalled();
});

test.each(["resolve", "reject"] as const)(
    "ignores an obsolete StrictMode read that later %ss",
    async (outcome) => {
        let rejectOld: (error: Error) => void = () => {
            throw new Error("Not initialized");
        };
        let resolveOld: (result: LocalePreferenceResult) => void = () => {
            throw new Error("Not initialized");
        };
        const oldResponse = new Promise<LocalePreferenceResult>(
            (resolve, reject) => {
                resolveOld = resolve;
                rejectOld = reject;
            },
        );
        vi.mocked(getLocalePreference)
            .mockReturnValueOnce(oldResponse)
            .mockResolvedValueOnce({ kind: "desktop", preference: "en" });
        render(
            <StrictMode>
                <LocaleStartup systemLanguage="zh">
                    <input aria-label="Note" />
                </LocaleStartup>
            </StrictMode>,
        );
        await screen.findByRole("combobox", { name: "Language" });
        const input = screen.getByRole("textbox");
        fireEvent.change(input, { target: { value: "Still editing" } });
        await act(async () => {
            if (outcome === "resolve")
                resolveOld({ kind: "desktop", preference: "zh-CN" });
            else rejectOld(new Error("Synthetic stale failure"));
            await oldResponse.catch(() => undefined);
        });
        expect(
            fieldSelectValue(
                screen.getByRole("combobox", { name: "Language" }),
            ),
        ).toBe("en");
        expect(document.documentElement.lang).toBe("en");
        expect(screen.getByRole("textbox")).toBe(input);
        expect(input).toHaveProperty("value", "Still editing");
        expect(screen.queryByRole("alert")).toBeNull();
    },
);

test("updates HTML language only after a confirmed save and preserves the same input", async () => {
    vi.mocked(getLocalePreference).mockResolvedValue({
        kind: "desktop",
        preference: "en",
    });
    let finishSave: (choice: "zh-CN") => void = () => {
        throw new Error("Not initialized");
    };
    const save = new Promise<"zh-CN">((resolve) => {
        finishSave = resolve;
    });
    vi.mocked(saveLocalePreference).mockReturnValue(save);
    render(
        <LocaleStartup systemLanguage="en">
            <input aria-label="Note" />
        </LocaleStartup>,
    );
    await screen.findByRole("combobox", { name: "Language" });
    const input = screen.getByRole("textbox");
    fireEvent.change(input, { target: { value: "Unsubmitted draft" } });
    setFieldSelectValue(screen.getByRole("combobox"), "zh-CN");
    expect(document.documentElement.lang).toBe("en");
    await act(async () => {
        finishSave("zh-CN");
        await save;
    });
    await screen.findByRole("combobox", { name: "语言" });
    expect(document.documentElement.lang).toBe("zh-CN");
    expect(screen.getByRole("textbox")).toBe(input);
    expect(input).toHaveProperty("value", "Unsubmitted draft");
});

test("does not change HTML language when an unmounted startup read completes", async () => {
    const response = pendingPreference();
    vi.mocked(getLocalePreference).mockReturnValue(response.promise);
    const view = render(
        <LocaleStartup systemLanguage="en">
            <input aria-label="Note" />
        </LocaleStartup>,
    );
    view.unmount();
    document.documentElement.lang = "fr";
    await act(async () => {
        response.resolve({ kind: "desktop", preference: "zh-CN" });
        await response.promise;
    });
    expect(document.documentElement.lang).toBe("fr");
    expect(saveLocalePreference).not.toHaveBeenCalled();
});
