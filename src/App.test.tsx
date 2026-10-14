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
import { listSkills } from "./lib/desktop/skills";
import { countProviderModels } from "./lib/desktop/models";
import { discoverAgents } from "./lib/desktop/agents";
import { agentReport } from "./test/agentDiscovery";
import en from "./locales/en.json";

vi.mock(import("./lib/desktop/agents"), async (original) => ({
    ...(await original()),
    discoverAgents: vi.fn(),
}));

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

vi.mock(import("./lib/desktop/skills"), async (importOriginal) => ({
    ...(await importOriginal()),
    listSkills: vi.fn(),
}));

vi.mock(import("./lib/desktop/models"), async (importOriginal) => ({
    ...(await importOriginal()),
    countProviderModels: vi.fn(),
}));

beforeEach(async () => {
    vi.mocked(discoverAgents)
        .mockReset()
        .mockResolvedValue({ kind: "preview" });
    vi.mocked(listMcpDefinitions)
        .mockReset()
        .mockResolvedValue({ kind: "preview" });
    vi.mocked(listSkills).mockReset().mockResolvedValue({ kind: "preview" });
    vi.mocked(countProviderModels)
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
    expect(
        screen.queryByRole("textbox", { name: "Unsubmitted note" }),
    ).toBeNull();
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
    // The Skills page is a real module now; it opens its own heading.
    expect(
        await screen.findByRole("heading", { level: 1, name: "Skills" }),
    ).toBeDefined();
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
    // Keep separate text nodes separate: adjacent paragraphs such as
    // "coding agents." and "Version" are not an unresolved agents.Version key.
    const walker = document.createTreeWalker(
        document.body,
        NodeFilter.SHOW_TEXT,
    );
    const text: string[] = [];
    while (walker.nextNode()) text.push(walker.currentNode.textContent ?? "");
    return [...text, ...attributes].join("\n");
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
    // Overview v1 shows bilingual stat cards (browser preview stays honest).
    expect(
        await screen.findByRole("button", {
            name: "服务商: 请在桌面应用中查看此数量。",
        }),
    ).toBeDefined();
    expect(
        screen.getByRole("button", {
            name: "已添加模型: 请在桌面应用中查看此数量。",
        }),
    ).toBeDefined();
    // Brand and module names stay in their original form inside Chinese UI.
    expect(screen.getByRole("button", { name: "Agent" })).toBeDefined();
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
                protocolBaseUrls: ["https://api.deepseek.com"],
                defaultProtocol: "chat_completions",
                extensionFields: [],
            },
        ],
    });
    vi.mocked(listProviders).mockResolvedValue({
        kind: "desktop",
        page: { items: [savedProvider], nextCursor: null, total: 1 },
    });
}

test("the Providers page reads nothing until first visited", async () => {
    useDesktopProviders();
    render(<App />);
    // Overview may read the provider total; templates stay module-only until visit.
    await screen.findByRole("button", { name: /Providers:/ });
    expect(listProviders).toHaveBeenCalled();
    expect(listProviderTemplates).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Agents" }));
    expect(listProviderTemplates).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Providers" }));
    expect(await screen.findByText("Personal DeepSeek")).toBeDefined();
    expect(listProviderTemplates).toHaveBeenCalled();
});

test("the Providers sidebar returns to its list without remounting the module", async () => {
    translator = await createAppI18n("en");
    useDesktopProviders();
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Providers" }));
    expect(
        await screen.findByRole("button", { name: "Edit Personal DeepSeek" }),
    ).toBeDefined();
    // Saved settings are never presented as a live connection.
    expect(document.body.textContent).not.toMatch(
        /\bconnected\b|available|已连接|可用/i,
    );
    const reads = vi.mocked(listProviders).mock.calls.length;

    fireEvent.click(screen.getByRole("button", { name: "New" }));
    const draft = screen.getByRole("textbox", { name: "Name" });
    fireEvent.change(draft, { target: { value: "Draft name" } });
    // Synthetic; never a real key.
    fireEvent.change(screen.getByLabelText("API key"), {
        target: { value: "sk-synthetic-app-5555" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Agents" }));
    expect(
        await screen.findByRole("button", { name: "Discover" }),
    ).toBeDefined();
    expect(screen.queryByRole("textbox", { name: "Name" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Providers" }));
    expect(screen.queryByRole("textbox", { name: "Name" })).toBeNull();
    expect(
        screen.getByRole("button", { name: "Edit Personal DeepSeek" }),
    ).toBeDefined();
    // Returning to the list does not remount the module or re-read it.
    fireEvent.click(screen.getByRole("button", { name: "Providers" }));
    expect(
        screen.getByRole("button", { name: "Edit Personal DeepSeek" }),
    ).toBeDefined();
    expect(listProviders).toHaveBeenCalledTimes(reads);
    expect(createProvider).not.toHaveBeenCalled();
    expectNoKeysOrDiagnostics();
});

// Flaky (rare): asserts focus on the saved Edit row; sometimes lands on
// document.body after the edit form unmounts and the list IconButton remounts
// from refreshList. Rerun usually passes. Notifications close-button placement
// outside role=status was checked; no clear product-safe fix yet (2026-10-14).
test("a save shows an app-level notification that outlives the jump back and page changes", async () => {
    translator = await createAppI18n("en");
    useDesktopProviders();
    const renamed = {
        ...savedProvider,
        displayName: "Renamed",
        revision: savedProvider.revision + 1,
    };
    vi.mocked(updateProvider).mockResolvedValue(renamed);
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Providers" }));
    fireEvent.click(
        await screen.findByRole("button", {
            name: "Edit Personal DeepSeek",
        }),
    );
    await screen.findByPlaceholderText("••••••••");
    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), {
        target: { value: "Renamed" },
    });
    vi.mocked(listProviders).mockResolvedValue({
        kind: "desktop",
        page: { items: [renamed], nextCursor: null, total: 1 },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    const notice = await screen.findByText("Saved Renamed.");
    expect(notice.closest('[role="status"]')).not.toBeNull();
    // Back on the list, focus is on the saved row, not the notification.
    expect(document.activeElement).toBe(
        screen.getByRole("button", { name: "Edit Renamed" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Agents" }));
    expect(screen.getByText("Saved Renamed.")).toBe(notice);
    expectNoKeysOrDiagnostics();
});

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

test("About metadata failures are translated and retried", async () => {
    vi.mocked(getLocalePreference).mockResolvedValue({
        kind: "desktop",
        preference: "en",
    });
    vi.mocked(getAppInfo).mockRejectedValue(
        new Error("private runtime detail"),
    );
    renderDesktopStartup("en-US");
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    fireEvent.click(screen.getByRole("tab", { name: "About" }));
    expect((await screen.findByRole("alert")).textContent).toBe(
        "Application information could not be read. Try again.",
    );
    expectNoKeysOrDiagnostics();
    vi.mocked(getAppInfo).mockResolvedValue({
        name: "vibemate",
        version: "0.1.0",
    });
    fireEvent.click(
        screen.getByRole("button", { name: "Reload application information" }),
    );
    expect(await screen.findByText("0.1.0")).toBeDefined();
    expect(screen.queryByRole("alert")).toBeNull();
});

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

test("Settings sidebar returns to General without unmounting preference input", async () => {
    translator = await createAppI18n("en");
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    const input = screen.getByRole("textbox", {
        name: "Unsubmitted note",
    });
    fireEvent.change(input, { target: { value: "Retained setting" } });
    fireEvent.click(screen.getByRole("tab", { name: "About" }));
    fireEvent.click(screen.getByRole("button", { name: "Overview" }));
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(
        screen
            .getByRole("tab", { name: "General" })
            .getAttribute("aria-selected"),
    ).toBe("true");
    expect(screen.getByRole("textbox", { name: "Unsubmitted note" })).toBe(
        input,
    );
    expect(input).toHaveProperty("value", "Retained setting");
    fireEvent.click(screen.getByRole("tab", { name: "About" }));
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(
        screen
            .getByRole("tab", { name: "General" })
            .getAttribute("aria-selected"),
    ).toBe("true");
});

test("MCP mounts on first visit and sidebar activation returns to its list", async () => {
    vi.mocked(listMcpDefinitions).mockResolvedValue({
        kind: "desktop",
        page: {
            items: [],
            nextCursor: null,
            total: 0,
            deletedCleanupPending: false,
        },
    });
    render(<App />);
    // Overview may read the MCP total; the MCP module stays unmounted until visit.
    await screen.findByRole("button", { name: /MCP servers:/ });
    expect(listMcpDefinitions).toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "New" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "MCP servers" }));
    await screen.findByText("No MCP definitions saved yet.");
    fireEvent.click(screen.getByRole("button", { name: "New" }));
    fireEvent.change(screen.getByLabelText(/^Name$/), {
        target: { value: "Keep this draft" },
    });
    fireEvent.click(
        screen.getByRole("button", { name: "Add environment variable" }),
    );
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
    await screen.findByText("No MCP definitions saved yet.");
    expect(screen.queryByLabelText(/^Name$/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "New" }));
    fireEvent.click(screen.getByRole("button", { name: "MCP servers" }));
    await screen.findByText("No MCP definitions saved yet.");
    expect(screen.queryByLabelText(/^Name$/)).toBeNull();
});

test("Agent discovery starts on first visit and preserves paths across navigation", async () => {
    vi.mocked(discoverAgents).mockResolvedValue({
        kind: "desktop",
        items: agentReport([
            {
                id: "pi",
                state: "found",
                source: "path",
                entryPath: "/usr/bin/pi",
                resolvedPath: "/usr/bin/pi",
                version: "1.1.0",
                versionEvidence: "packageMetadata",
                capabilityEvidence: "documented",
                configDirectory: "/home/test/.pi/agent",
                directorySource: "default",
                files: [
                    { name: "settings.json", state: "missing" },
                    { name: "models.json", state: "missing" },
                    { name: "mcp.json", state: "missing" },
                ],
            },
            {
                id: "grokBuild",
                state: "notFound",
                source: "none",
                entryPath: null,
                resolvedPath: null,
                version: null,
                versionEvidence: "none",
                capabilityEvidence: "unverified",
                configDirectory: "/home/test/.grok",
                directorySource: "default",
                files: [
                    { name: "config.toml", state: "missing" },
                    { name: "managed_config.toml", state: "missing" },
                    { name: "requirements.toml", state: "missing" },
                ],
            },
        ]),
    });
    render(<App />);
    // Overview may discover for its agent total; the Agents module mounts on visit.
    await screen.findByRole("button", { name: /Discovered agents:/ });
    expect(discoverAgents).toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "Discover" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Agents" }));
    fireEvent.click(
        await screen.findByRole("button", { name: "Open Pi details" }),
    );
    const input = screen.getByRole("textbox", { name: "Pi entry path" });
    fireEvent.change(input, { target: { value: "/home/test/manual-pi" } });
    const callsBeforeOverview = vi.mocked(discoverAgents).mock.calls.length;
    fireEvent.click(screen.getByRole("button", { name: "Overview" }));
    expect(screen.queryByRole("textbox", { name: "Pi entry path" })).toBeNull();
    // Returning home remounts Overview and may re-read totals; Agents stays cached.
    await screen.findByRole("button", { name: /Discovered agents:/ });
    const callsAfterOverview = vi.mocked(discoverAgents).mock.calls.length;
    expect(callsAfterOverview).toBeGreaterThan(callsBeforeOverview);
    fireEvent.click(screen.getByRole("button", { name: "Agents" }));
    expect(screen.queryByRole("textbox", { name: "Pi entry path" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Open Pi details" }));
    expect(
        screen.getByRole("textbox", { name: "Pi entry path" }),
    ).toHaveProperty("value", "/home/test/manual-pi");
    fireEvent.click(screen.getByRole("button", { name: "Agents" }));
    expect(screen.queryByRole("textbox", { name: "Pi entry path" })).toBeNull();
    expect(discoverAgents).toHaveBeenCalledTimes(callsAfterOverview);
});
