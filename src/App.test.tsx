import { StrictMode, type JSX, type ReactNode } from "react";
import {
    act,
    cleanup,
    fireEvent,
    render as renderView,
    screen,
    waitFor,
} from "@testing-library/react";
import type { i18n } from "i18next";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import DesktopApp from "./App";
import { createAppI18n } from "./i18n";
import { LocaleStartup } from "./features/settings/LocaleStartup";
import { AppearanceControl } from "./features/settings/AppearanceControl";
import { LanguageSelector } from "./features/settings/LanguageSelector";
import {
    getLocalePreference,
    saveLocalePreference,
    SettingsRequestError,
} from "./lib/desktop/settings";

import {
    getAppearancePreference,
    saveAppearancePreference,
} from "./lib/desktop/appearance";
import { getAppInfo, openProjectRepository } from "./lib/desktop";
import en from "./locales/en.json";

let translator: i18n;
function render(element: ReactNode) {
    // Keep StrictMode at the root, as in main.tsx. A nested-only StrictMode in
    // React 19 does not replay initial effects when its parent is not strict.
    return renderView(
        <StrictMode>
            <I18nextProvider i18n={translator}>{element}</I18nextProvider>
        </StrictMode>,
    );
}

// A concrete settings slot lets navigation tests observe preservation of a real
// input. Native language persistence is covered by the startup/selector tests.
function App(): JSX.Element {
    return (
        <DesktopApp
            languageSettings={
                <>
                    <LanguageSelector
                        initialPreference={{
                            kind: "desktop",
                            preference: "en",
                        }}
                        systemLanguage="en"
                        footer={<AppearanceControl />}
                    />
                    <input aria-label="Unsubmitted note" defaultValue="Draft" />
                </>
            }
        />
    );
}

// Replace only the desktop boundary; the component and React lifecycle remain real.
vi.mock(import("./lib/desktop/settings"), async (importOriginal) => ({
    ...(await importOriginal()),
    getLocalePreference: vi.fn<typeof getLocalePreference>(),
    saveLocalePreference: vi.fn<typeof saveLocalePreference>(),
}));

vi.mock(import("./lib/desktop/appearance"), async (importOriginal) => ({
    ...(await importOriginal()),
    getAppearancePreference: vi.fn<typeof getAppearancePreference>(),
    saveAppearancePreference: vi.fn<typeof saveAppearancePreference>(),
}));

vi.mock(import("./lib/desktop"), async (importOriginal) => ({
    ...(await importOriginal()),
    getAppInfo: vi.fn<typeof getAppInfo>(),
    openProjectRepository: vi.fn<typeof openProjectRepository>(),
}));

beforeEach(async () => {
    vi.mocked(getAppInfo)
        .mockReset()
        .mockResolvedValue({ name: "vibemate", version: "0.1.0" });
    vi.mocked(openProjectRepository).mockReset().mockResolvedValue(undefined);
    vi.mocked(getLocalePreference)
        .mockReset()
        .mockResolvedValue({ kind: "desktop", preference: "en" });
    vi.mocked(saveLocalePreference).mockReset();
    vi.mocked(getAppearancePreference)
        .mockReset()
        .mockResolvedValue({ kind: "preview" });
    vi.mocked(saveAppearancePreference).mockReset();
    translator = await createAppI18n("en");
});

afterEach(() => {
    // Vitest globals are disabled, so cleanup is registered explicitly.
    cleanup();
    // LocaleStartup writes the document language outside React's root; clear it
    // so one test's language cannot satisfy the next test's assertion.
    document.documentElement.removeAttribute("lang");
});

test("collapses navigation without losing accessible destinations or changing the current page", () => {
    render(<App />);
    const collapse = screen.getByRole("button", {
        name: "Collapse navigation",
    });
    collapse.focus();
    fireEvent.click(collapse);
    expect(screen.getByRole("button", { name: "Expand navigation" })).toBe(
        collapse,
    );
    expect(collapse.getAttribute("aria-expanded")).toBe("false");
    expect(document.activeElement).toBe(collapse);
    fireEvent.click(screen.getByRole("button", { name: "Providers" }));
    expect(
        screen
            .getByRole("button", { name: "Providers" })
            .getAttribute("aria-current"),
    ).toBe("page");
    expect(
        screen.getByRole("heading", { name: "Not implemented" }),
    ).toBeDefined();
    fireEvent.click(collapse);
    expect(collapse.getAttribute("aria-expanded")).toBe("true");
    expect(
        screen
            .getByRole("button", { name: "Providers" })
            .getAttribute("aria-current"),
    ).toBe("page");
});

test("returns to the relationship home from both Overview and the brand", () => {
    render(<App />);
    const overview = screen.getByRole("button", { name: "Overview" });
    expect(overview.getAttribute("aria-current")).toBe("page");
    fireEvent.click(screen.getByRole("button", { name: "Providers" }));
    expect(overview.getAttribute("aria-current")).toBeNull();
    fireEvent.click(overview);
    expect(overview.getAttribute("aria-current")).toBe("page");
    expect(
        screen.getByRole("main", { name: "Configuration overview" }),
    ).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Agents" }));
    fireEvent.click(
        screen.getByRole("button", {
            name: "vibemate · Configuration overview",
        }),
    );
    expect(overview.getAttribute("aria-current")).toBe("page");
});

test("keeps every button and field outside the window drag regions", () => {
    render(<App />);
    const dragRegions = Array.from(
        document.querySelectorAll("[data-tauri-drag-region]"),
    );
    const controls = Array.from(
        document.querySelectorAll("button, input, select, textarea, a[href]"),
    );
    // Guard against a vacuous pass: the check is meaningless without both sides.
    expect(dragRegions.length).toBeGreaterThan(0);
    expect(controls.length).toBeGreaterThan(0);
    // jsdom has no layout engine, so overlap is verified in the native window.
    // Containment is still the rule that stops clicks from being swallowed.
    const controlsInsideDragRegion = controls.filter((control) =>
        dragRegions.some((region) => region.contains(control)),
    );
    expect(controlsInsideDragRegion).toEqual([]);
});

test("keeps the same settings input and colors while navigating elsewhere", async () => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    await waitFor(() => {
        expect(
            screen.getByRole("combobox", { name: "Appearance" }),
        ).toHaveProperty("disabled", false);
    });
    const input = screen.getByRole("textbox", { name: "Unsubmitted note" });
    fireEvent.change(input, { target: { value: "Still editing" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Appearance" }), {
        target: { value: "light" },
    });
    fireEvent.click(screen.getByRole("radio", { name: "Iris" }));
    fireEvent.click(screen.getByRole("button", { name: "Agents" }));
    expect(screen.queryByRole("textbox")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("textbox")).toBe(input);
    expect(input).toHaveProperty("value", "Still editing");
    expect(screen.getByRole("combobox", { name: "Appearance" })).toHaveProperty(
        "value",
        "light",
    );
    expect(document.documentElement.dataset["appearance"]).toBe("light");
    expect(screen.getByRole("radio", { name: "Iris" })).toHaveProperty(
        "checked",
        true,
    );
});

test("translates current navigation without resetting the selected page or collapse", async () => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Skills" }));
    fireEvent.click(
        screen.getByRole("button", { name: "Collapse navigation" }),
    );
    await act(async () => {
        await translator.changeLanguage("zh-CN");
    });
    expect(
        screen
            .getByRole("button", { name: "技能" })
            .getAttribute("aria-current"),
    ).toBe("page");
    const expand = screen.getByRole("button", { name: "展开导航" });
    expect(expand.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(expand);
    expect(
        screen
            .getByRole("button", { name: "折叠导航" })
            .getAttribute("aria-expanded"),
    ).toBe("true");
});

// Compose startup exactly as main.tsx does. The system language defaults to a
// Chinese variant so a saved English choice must visibly take priority. An
// extra field lets tests observe that language changes keep unsaved input.
function renderDesktopStartup(systemLanguage = "zh-TW") {
    return render(
        <LocaleStartup systemLanguage={systemLanguage}>
            {(snapshot) => (
                <DesktopApp
                    languageSettings={
                        <>
                            <LanguageSelector
                                {...snapshot}
                                footer={<AppearanceControl />}
                            />
                            <input
                                aria-label="Unsubmitted note"
                                defaultValue="Draft"
                            />
                        </>
                    }
                />
            )}
        </LocaleStartup>,
    );
}

test("keeps an uncertain language save blocked across navigation until a real selector reload reconciles it", async () => {
    vi.mocked(saveLocalePreference).mockRejectedValue(
        new SettingsRequestError("operation_failed"),
    );
    renderDesktopStartup();
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    const selector = screen.getByRole("combobox", {
        name: "Language",
    });
    fireEvent.change(selector, { target: { value: "zh-CN" } });
    const reload = await screen.findByRole("button", {
        name: "Reload saved preference",
    });
    fireEvent.click(screen.getByRole("button", { name: "Providers" }));
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("combobox", { name: "Language" })).toBe(selector);
    expect(selector).toHaveProperty("disabled", true);
    expect(
        screen.getByRole("button", { name: "Reload saved preference" }),
    ).toBe(reload);
    vi.mocked(getLocalePreference).mockResolvedValue({
        kind: "desktop",
        preference: "zh-CN",
    });
    fireEvent.click(reload);
    await screen.findByRole("combobox", { name: "语言" });
    expect(screen.getByRole("combobox", { name: "语言" })).toHaveProperty(
        "value",
        "zh-CN",
    );
    expect(document.documentElement.lang).toBe("zh-CN");
    expect(
        screen
            .getByRole("button", { name: "设置" })
            .getAttribute("aria-current"),
    ).toBe("page");
});

test("finishes a pending save while Settings is hidden and retains its confirmed choice on return", async () => {
    let finishSave: (choice: "zh-CN") => void = () => {
        throw new Error("Not initialized");
    };
    const pendingSave = new Promise<"zh-CN">((resolve) => {
        finishSave = resolve;
    });
    vi.mocked(saveLocalePreference).mockReturnValue(pendingSave);
    renderDesktopStartup();
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    const selector = screen.getByRole("combobox", {
        name: "Language",
    });
    fireEvent.change(selector, { target: { value: "zh-CN" } });
    fireEvent.click(screen.getByRole("button", { name: "Providers" }));
    await act(async () => {
        finishSave("zh-CN");
        await pendingSave;
    });
    expect(document.documentElement.lang).toBe("zh-CN");
    expect(
        screen
            .getByRole("button", { name: "服务商" })
            .getAttribute("aria-current"),
    ).toBe("page");
    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    expect(screen.getByRole("combobox", { name: "语言" })).toBe(selector);
    expect(selector).toHaveProperty("value", "zh-CN");
    expect(selector).toHaveProperty("disabled", false);
});

test("the brand returns home in both logo states and preserves collapse", () => {
    render(<App />);
    const brand = screen.getByRole("button", {
        name: "vibemate · Configuration overview",
    });
    fireEvent.click(screen.getByRole("button", { name: "Agents" }));
    fireEvent.click(brand);
    expect(screen.getByRole("main").getAttribute("aria-label")).toBe(
        "Configuration overview",
    );
    fireEvent.click(
        screen.getByRole("button", { name: "Collapse navigation" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Providers" }));
    expect(
        screen.getByRole("button", {
            name: "vibemate · Configuration overview",
        }),
    ).toBe(brand);
    fireEvent.click(brand);
    expect(screen.getByRole("main").getAttribute("aria-label")).toBe(
        "Configuration overview",
    );
    expect(
        screen.getByRole("button", { name: "Expand navigation" }),
    ).toBeDefined();
});

/**
 * Collect rendered text plus accessible names and tooltips, which can expose a
 * raw key or diagnostic even when it is not visible text.
 */
function renderedTextAndNames(): string {
    const attributes = Array.from(
        document.body.querySelectorAll(
            "[aria-label], [title], [placeholder], [alt]",
        ),
    ).flatMap((element) =>
        ["aria-label", "title", "placeholder", "alt"].map(
            (name) => element.getAttribute(name) ?? "",
        ),
    );
    return [document.body.textContent, ...attributes].join("\n");
}

// Unresolved keys start with a top-level resource group, e.g. `settings.about.failed`.
// Derive the groups from the English resource so a new group is covered too.
// Group names are plain camelCase identifiers, so they need no regex escaping.
const UNRESOLVED_KEY = new RegExp(
    `\\b(?:${Object.keys(en).join("|")})\\.[A-Za-z]`,
);

/** Fail if any rendered text or accessible attribute shows a key or diagnostic. */
function expectNoKeysOrDiagnostics(): void {
    const text = renderedTextAndNames();
    expect(text).not.toMatch(UNRESOLVED_KEY);
    expect(text).not.toMatch(/private/i);
}

test("a saved Chinese choice overrides an English system at startup", async () => {
    vi.mocked(getLocalePreference).mockResolvedValue({
        kind: "desktop",
        preference: "zh-CN",
    });
    renderDesktopStartup("en-US");
    expect(await screen.findByRole("main", { name: "配置概览" })).toBeDefined();
    expect(document.documentElement.lang).toBe("zh-CN");
    expect(screen.getByRole("list", { name: "计划中的服务商" })).toBeDefined();
    // Brand and provider names stay in their original form inside Chinese UI.
    expect(screen.getByText("OpenRouter")).toBeDefined();
    expect(
        screen.getByRole("button", { name: "vibemate · 配置概览" }),
    ).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    expect(screen.getByRole("combobox", { name: "语言" })).toHaveProperty(
        "value",
        "zh-CN",
    );
    expectNoKeysOrDiagnostics();
});

test("following the system opens in Chinese for a Chinese system language", async () => {
    vi.mocked(getLocalePreference).mockResolvedValue({
        kind: "desktop",
        preference: "system",
    });
    renderDesktopStartup("zh-Hans-CN");
    fireEvent.click(await screen.findByRole("button", { name: "设置" }));
    expect(document.documentElement.lang).toBe("zh-CN");
    expect(screen.getByRole("combobox", { name: "语言" })).toHaveProperty(
        "value",
        "system",
    );
    expect(screen.getByRole("tab", { name: "常规" })).toBeDefined();
    expectNoKeysOrDiagnostics();
});

test("switching language through the real selector keeps page, tab, collapse, input and loaded metadata", async () => {
    vi.mocked(saveLocalePreference).mockImplementation((choice) =>
        Promise.resolve(choice),
    );
    renderDesktopStartup("en-US");
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    // Load About once, then return to General where the selector lives.
    fireEvent.click(screen.getByRole("tab", { name: "About" }));
    expect(await screen.findByText("0.1.0")).toBeDefined();
    // StrictMode replays the first effect, so compare against this count.
    const metadataReads = vi.mocked(getAppInfo).mock.calls.length;
    fireEvent.click(screen.getByRole("tab", { name: "General" }));
    fireEvent.click(
        screen.getByRole("button", { name: "Collapse navigation" }),
    );
    const input = screen.getByRole("textbox", { name: "Unsubmitted note" });
    fireEvent.change(input, { target: { value: "Still editing" } });
    const selector = screen.getByRole("combobox", { name: "Language" });

    fireEvent.change(selector, { target: { value: "zh-CN" } });
    await screen.findByRole("combobox", { name: "语言" });
    expect(saveLocalePreference).toHaveBeenCalledExactlyOnceWith("zh-CN");
    expect(document.documentElement.lang).toBe("zh-CN");
    expect(screen.getByRole("combobox", { name: "语言" })).toBe(selector);
    expect(screen.getByRole("textbox")).toBe(input);
    expect(input).toHaveProperty("value", "Still editing");
    expect(
        screen
            .getByRole("button", { name: "设置" })
            .getAttribute("aria-current"),
    ).toBe("page");
    expect(
        screen.getByRole("tab", { name: "常规" }).getAttribute("aria-selected"),
    ).toBe("true");
    expect(
        screen
            .getByRole("button", { name: "展开导航" })
            .getAttribute("aria-expanded"),
    ).toBe("false");
    // The loaded metadata is translated in place rather than read again.
    fireEvent.click(screen.getByRole("tab", { name: "关于" }));
    expect(screen.getByText("版本")).toBeDefined();
    expect(screen.getByText("0.1.0")).toBeDefined();
    expect(getAppInfo).toHaveBeenCalledTimes(metadataReads);

    fireEvent.click(screen.getByRole("tab", { name: "常规" }));
    fireEvent.change(selector, { target: { value: "en" } });
    await screen.findByRole("combobox", { name: "Language" });
    expect(document.documentElement.lang).toBe("en");
    expect(input).toHaveProperty("value", "Still editing");
    expect(
        screen
            .getByRole("button", { name: "Expand navigation" })
            .getAttribute("aria-expanded"),
    ).toBe("false");
    expectNoKeysOrDiagnostics();
});

test("an untranslated Chinese entry falls back to English instead of its key", async () => {
    translator = await createAppI18n("zh-CN");
    // An empty value counts as missing because the translator disables empty
    // results; this is the case the resource check is designed to prevent.
    translator.addResource("zh-CN", "translation", "desktop.nav.providers", "");
    render(<App />);
    const providers = screen.getByRole("button", { name: "Providers" });
    expect(screen.getByRole("button", { name: "技能" })).toBeDefined();
    fireEvent.click(providers);
    expect(screen.getByRole("main", { name: "Providers" })).toBeDefined();
    expect(screen.getByRole("heading", { name: "功能尚未实现" })).toBeDefined();
    expectNoKeysOrDiagnostics();
});

test.each([
    {
        system: "en-US",
        message:
            "The language preference could not be read. Retry before opening the app.",
        retry: "Retry startup",
        settings: "Settings",
    },
    {
        system: "zh-CN",
        message: "无法读取语言偏好，请重试后再进入应用。",
        retry: "重试启动",
        settings: "设置",
    },
])(
    "a failed startup read blocks the app and retries in $system",
    async ({ system, message, retry, settings }) => {
        vi.mocked(getLocalePreference).mockRejectedValue(
            new Error("private SQLite error at /Users/someone/app.db"),
        );
        renderDesktopStartup(system);
        expect((await screen.findByRole("alert")).textContent).toBe(message);
        expect(screen.queryByRole("button", { name: settings })).toBeNull();
        expectNoKeysOrDiagnostics();
        vi.mocked(getLocalePreference).mockResolvedValue({
            kind: "desktop",
            preference: "system",
        });
        fireEvent.click(screen.getByRole("button", { name: retry }));
        expect(
            await screen.findByRole("button", { name: settings }),
        ).toBeDefined();
        expect(saveLocalePreference).not.toHaveBeenCalled();
    },
);

test.each([
    {
        preference: "en",
        settings: "Settings",
        about: "About",
        message: "Application information could not be read. Try again.",
        retry: "Reload application information",
    },
    {
        preference: "zh-CN",
        settings: "设置",
        about: "关于",
        message: "无法读取应用信息，请重试。",
        retry: "重新读取应用信息",
    },
] as const)(
    "About metadata failures are translated and retried in $preference",
    async ({ preference, settings, about, message, retry }) => {
        vi.mocked(getLocalePreference).mockResolvedValue({
            kind: "desktop",
            preference,
        });
        vi.mocked(getAppInfo).mockRejectedValue(
            new Error("private runtime detail"),
        );
        renderDesktopStartup("en-US");
        fireEvent.click(await screen.findByRole("button", { name: settings }));
        fireEvent.click(screen.getByRole("tab", { name: about }));
        expect((await screen.findByRole("alert")).textContent).toBe(message);
        expectNoKeysOrDiagnostics();
        vi.mocked(getAppInfo).mockResolvedValue({
            name: "vibemate",
            version: "0.1.0",
        });
        fireEvent.click(screen.getByRole("button", { name: retry }));
        expect(await screen.findByText("0.1.0")).toBeDefined();
        expect(screen.queryByRole("alert")).toBeNull();
    },
);

test.each([
    {
        preference: "en",
        next: "zh-CN",
        settings: "Settings",
        label: "Language",
        message:
            "The language preference could not be saved. Your previous choice is unchanged; try again.",
    },
    {
        preference: "zh-CN",
        next: "en",
        settings: "设置",
        label: "语言",
        message: "无法保存语言偏好，原选择未改变，请重试。",
    },
] as const)(
    "a failed language save keeps $preference and explains it in that language",
    async ({ preference, next, settings, label, message }) => {
        vi.mocked(getLocalePreference).mockResolvedValue({
            kind: "desktop",
            preference,
        });
        vi.mocked(saveLocalePreference).mockRejectedValue(
            new SettingsRequestError("write_failed"),
        );
        renderDesktopStartup("en-US");
        fireEvent.click(await screen.findByRole("button", { name: settings }));
        const selector = screen.getByRole("combobox", { name: label });
        fireEvent.change(selector, { target: { value: next } });
        // Appearance preview has its own status line, so find this one by text.
        const feedback = await screen.findByText(message);
        expect(feedback.getAttribute("role")).toBe("status");
        expect(selector).toHaveProperty("value", preference);
        expect(selector).toHaveProperty("disabled", false);
        expect(document.documentElement.lang).toBe(preference);
        expectNoKeysOrDiagnostics();
    },
);
