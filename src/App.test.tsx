import { StrictMode, type JSX, type ReactNode } from "react";
import {
    act,
    cleanup,
    fireEvent,
    render as renderView,
    screen,
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

beforeEach(async () => {
    vi.mocked(getLocalePreference)
        .mockReset()
        .mockResolvedValue({ kind: "desktop", preference: "en" });
    vi.mocked(saveLocalePreference).mockReset();
    translator = await createAppI18n("en");
});

afterEach(() => {
    // Vitest globals are disabled, so cleanup is registered explicitly.
    cleanup();
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

test("keeps the same settings input and appearance while navigating elsewhere", () => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    const input = screen.getByRole("textbox", { name: "Unsubmitted note" });
    fireEvent.change(input, { target: { value: "Still editing" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Appearance" }), {
        target: { value: "light" },
    });
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

function renderDesktopStartup() {
    return render(
        <LocaleStartup systemLanguage="zh-TW">
            {(snapshot) => (
                <DesktopApp
                    languageSettings={
                        <LanguageSelector
                            {...snapshot}
                            footer={<AppearanceControl />}
                        />
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
