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
import { fieldSelectValue, setFieldSelectValue } from "./test/fieldSelect";
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
import {
    createProvider,
    getProvider,
    listProviders,
    listProviderTemplates,
    updateProvider,
    type ProviderRecord,
} from "./lib/desktop/providers";
import {
    getProviderSecretStatus,
    replaceProviderSecret,
} from "./lib/desktop/providerSecrets";
import { listMcpDefinitions } from "./lib/desktop/mcp";
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

vi.mock(import("./lib/desktop/providers"), async (importOriginal) => ({
    ...(await importOriginal()),
    createProvider: vi.fn<typeof createProvider>(),
    getProvider: vi.fn<typeof getProvider>(),
    listProviders: vi.fn<typeof listProviders>(),
    listProviderTemplates: vi.fn<typeof listProviderTemplates>(),
    updateProvider: vi.fn<typeof updateProvider>(),
}));

vi.mock(import("./lib/desktop/providerSecrets"), async (importOriginal) => ({
    ...(await importOriginal()),
    getProviderSecretStatus: vi.fn<typeof getProviderSecretStatus>(),
    replaceProviderSecret: vi.fn<typeof replaceProviderSecret>(),
}));

vi.mock(import("./lib/desktop/mcp"), async (importOriginal) => ({
    ...(await importOriginal()),
    listMcpDefinitions: vi.fn(),
}));

beforeEach(async () => {
    vi.mocked(listMcpDefinitions)
        .mockReset()
        .mockResolvedValue({ kind: "preview" });
    // jsdom is a browser, so the Providers page defaults to its preview state.
    vi.mocked(listProviderTemplates)
        .mockReset()
        .mockResolvedValue({ kind: "preview" });
    vi.mocked(listProviders).mockReset().mockResolvedValue({ kind: "preview" });
    vi.mocked(createProvider).mockReset();
    vi.mocked(getProvider).mockReset();
    vi.mocked(updateProvider).mockReset();
    vi.mocked(getProviderSecretStatus)
        .mockReset()
        .mockImplementation((providerId) =>
            Promise.resolve({
                kind: "desktop",
                status: { providerId, state: "set", updatedAtMs: 1000 },
            }),
        );
    vi.mocked(replaceProviderSecret).mockReset();
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

test("collapses navigation without losing accessible destinations or changing the current page", async () => {
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
        await screen.findByRole("heading", { level: 1, name: "Providers" }),
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
    await setFieldSelectValue(
        screen.getByRole("combobox", { name: "Appearance" }),
        "light",
    );
    fireEvent.click(screen.getByRole("radio", { name: "Iris" }));
    fireEvent.click(screen.getByRole("button", { name: "Agents" }));
    expect(screen.queryByRole("textbox")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("textbox")).toBe(input);
    expect(input).toHaveProperty("value", "Still editing");
    expect(
        fieldSelectValue(screen.getByRole("combobox", { name: "Appearance" })),
    ).toBe("light");
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
    await setFieldSelectValue(selector, "zh-CN");
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
    expect(
        fieldSelectValue(screen.getByRole("combobox", { name: "语言" })),
    ).toBe("zh-CN");
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
    await setFieldSelectValue(selector, "zh-CN");
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
    expect(fieldSelectValue(selector)).toBe("zh-CN");
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
    expect(
        fieldSelectValue(screen.getByRole("combobox", { name: "语言" })),
    ).toBe("zh-CN");
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
    expect(
        fieldSelectValue(screen.getByRole("combobox", { name: "语言" })),
    ).toBe("system");
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

    await setFieldSelectValue(selector, "zh-CN");
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
    await setFieldSelectValue(selector, "en");
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
    expect(
        screen.getByRole("heading", { level: 1, name: "Providers" }),
    ).toBeDefined();
    expect(
        await screen.findByText(
            "浏览器预览无法读取或保存服务商配置。请在 vibemate 桌面应用中打开此页面。",
        ),
    ).toBeDefined();
    expectNoKeysOrDiagnostics();
});

const savedProvider: ProviderRecord = {
    id: "0123456789abcdef0123456789abcdef",
    kind: "deepseek",
    displayName: "Personal DeepSeek",
    baseUrl: "https://api.deepseek.com",
    protocol: "chat_completions",
    extensions: {},
    revision: 1,
    createdAtMs: 1000,
    updatedAtMs: 1000,
    selectedModelCount: 0,
};

function useDesktopProviders(): void {
    vi.mocked(getProviderSecretStatus).mockImplementation((providerId) =>
        Promise.resolve({
            kind: "desktop",
            status: { providerId, state: "set", updatedAtMs: 1000 },
        }),
    );
    vi.mocked(listProviderTemplates).mockResolvedValue({
        kind: "desktop",
        templates: [
            {
                kind: "deepseek",
                brandName: "DeepSeek",
                defaultBaseUrl: "https://api.deepseek.com",
                protocols: ["chat_completions"],
                defaultProtocol: "chat_completions",
                extensionFields: [],
            },
        ],
    });
    vi.mocked(listProviders).mockResolvedValue({
        kind: "desktop",
        page: { items: [savedProvider], nextCursor: null },
    });
}

test("the Providers page reads nothing until first visited", async () => {
    useDesktopProviders();
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Agents" }));
    expect(listProviders).not.toHaveBeenCalled();
    expect(listProviderTemplates).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Providers" }));
    expect(await screen.findByText("Personal DeepSeek")).toBeDefined();
    expect(screen.queryByText("Not implemented")).toBeNull();
});

test.each([
    {
        locale: "en",
        providers: "Providers",
        agents: "Agents",
        add: "New configuration",
        name: "Name",
        key: "API key",
        edit: "Edit Personal DeepSeek",
        planned: "Not implemented",
    },
    {
        locale: "zh-CN",
        providers: "服务商",
        agents: "Agent",
        add: "新建配置",
        name: "名称",
        key: "API 密钥",
        edit: "编辑 Personal DeepSeek",
        planned: "功能尚未实现",
    },
] as const)(
    "in $locale the Providers page shows saved configurations and keeps a draft across navigation",
    async ({ locale, providers, agents, add, name, key, edit, planned }) => {
        translator = await createAppI18n(locale);
        useDesktopProviders();
        render(<App />);
        fireEvent.click(screen.getByRole("button", { name: providers }));
        expect(await screen.findByRole("button", { name: edit })).toBeDefined();
        expect(screen.queryByText(planned)).toBeNull();
        // Saved settings are never presented as a live connection.
        expect(document.body.textContent).not.toMatch(
            /\bconnected\b|available|已连接|可用/i,
        );
        const reads = vi.mocked(listProviders).mock.calls.length;

        fireEvent.click(screen.getByRole("button", { name: add }));
        const draft = screen.getByRole("textbox", { name });
        fireEvent.change(draft, { target: { value: "Draft name" } });
        // Synthetic; never a real key.
        fireEvent.change(screen.getByLabelText(key), {
            target: { value: "sk-synthetic-app-5555" },
        });
        fireEvent.click(screen.getByRole("button", { name: agents }));
        expect(screen.getByText(planned)).toBeDefined();
        expect(screen.queryByRole("textbox", { name })).toBeNull();

        fireEvent.click(screen.getByRole("button", { name: providers }));
        expect(screen.getByRole("textbox", { name })).toBe(draft);
        expect(draft).toHaveProperty("value", "Draft name");
        // The draft stays, but a typed key is cleared while the page is hidden.
        expect(screen.getByLabelText(key)).toHaveProperty("value", "");
        // Returning does not remount the page or read the list again.
        expect(listProviders).toHaveBeenCalledTimes(reads);
        expect(createProvider).not.toHaveBeenCalled();
        expectNoKeysOrDiagnostics();
    },
);

test.each([
    {
        locale: "en",
        providers: "Providers",
        agents: "Agents",
        edit: "Edit Personal DeepSeek",
        renamedEdit: "Edit Renamed",
        name: "Name",
        save: "Save",
        saved: "Saved Renamed.",
    },
    {
        locale: "zh-CN",
        providers: "服务商",
        agents: "Agent",
        edit: "编辑 Personal DeepSeek",
        renamedEdit: "编辑 Renamed",
        name: "名称",
        save: "保存",
        saved: "已保存 Renamed。",
    },
] as const)(
    "in $locale a save shows an app-level notification that outlives the jump back and page changes",
    async ({
        locale,
        providers,
        agents,
        edit,
        renamedEdit,
        name,
        save,
        saved,
    }) => {
        translator = await createAppI18n(locale);
        useDesktopProviders();
        const renamed = {
            ...savedProvider,
            displayName: "Renamed",
            revision: savedProvider.revision + 1,
        };
        vi.mocked(updateProvider).mockResolvedValue(renamed);
        render(<App />);
        fireEvent.click(screen.getByRole("button", { name: providers }));
        fireEvent.click(await screen.findByRole("button", { name: edit }));
        await screen.findByTitle(
            locale === "en" ? "API key configured" : "已配置 API 密钥",
        );
        fireEvent.change(screen.getByRole("textbox", { name }), {
            target: { value: "Renamed" },
        });
        vi.mocked(listProviders).mockResolvedValue({
            kind: "desktop",
            page: { items: [renamed], nextCursor: null },
        });
        fireEvent.click(screen.getByRole("button", { name: save }));

        const notice = await screen.findByText(saved);
        expect(notice.closest('[role="status"]')).not.toBeNull();
        expect(notice.closest("main")).toBeNull();
        // Back on the list, focus is on the saved row, not the notification.
        expect(document.activeElement).toBe(
            screen.getByRole("button", { name: renamedEdit }),
        );
        fireEvent.click(screen.getByRole("button", { name: agents }));
        expect(screen.getByText(saved)).toBe(notice);
        expectNoKeysOrDiagnostics();
    },
);

test("switching language keeps the Providers list and an open form in place", async () => {
    useDesktopProviders();
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Providers" }));
    fireEvent.click(
        await screen.findByRole("button", {
            name: "Edit Personal DeepSeek",
        }),
    );
    const field = screen.getByRole("textbox", { name: "Name" });
    fireEvent.change(field, { target: { value: "Renamed" } });
    const reads = vi.mocked(listProviders).mock.calls.length;
    await act(async () => {
        await translator.changeLanguage("zh-CN");
    });
    expect(screen.getByRole("textbox", { name: "名称" })).toBe(field);
    expect(field).toHaveProperty("value", "Renamed");
    expect(
        screen.getByRole("heading", {
            level: 1,
            name: "DeepSeek",
        }),
    ).toBeDefined();
    expect(
        screen
            .getByRole("button", { name: "服务商" })
            .getAttribute("aria-current"),
    ).toBe("page");
    expect(listProviders).toHaveBeenCalledTimes(reads);
    expect(updateProvider).not.toHaveBeenCalled();
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
        await setFieldSelectValue(selector, next);
        // Appearance preview has its own status line, so find this one by text.
        const feedback = await screen.findByText(message);
        expect(feedback.getAttribute("role")).toBe("status");
        expect(fieldSelectValue(selector)).toBe(preference);
        expect(selector).toHaveProperty("disabled", false);
        expect(document.documentElement.lang).toBe(preference);
        expectNoKeysOrDiagnostics();
    },
);

test("MCP mounts only on first visit and hides credential input without losing non-secret drafts", async () => {
    vi.mocked(listMcpDefinitions).mockResolvedValue({
        kind: "desktop",
        page: { items: [], nextCursor: null },
    });
    render(<App />);
    expect(listMcpDefinitions).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "MCP servers" }));
    await screen.findByText("No MCP definitions saved yet.");
    fireEvent.click(screen.getByRole("button", { name: "New definition" }));
    fireEvent.change(screen.getByLabelText("Name"), {
        target: { value: "Keep this draft" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add field" }));
    fireEvent.change(screen.getByLabelText("Field name 1"), {
        target: { value: "TOKEN" },
    });
    fireEvent.change(screen.getByLabelText("Value 1"), {
        target: { value: "synthetic-app-mcp-secret" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Agents" }));
    await waitFor(() => {
        expect(screen.getByLabelText<HTMLInputElement>("Value 1").value).toBe(
            "",
        );
    });
    fireEvent.click(screen.getByRole("button", { name: "MCP servers" }));
    expect(screen.getByLabelText<HTMLInputElement>("Name").value).toBe(
        "Keep this draft",
    );
});
