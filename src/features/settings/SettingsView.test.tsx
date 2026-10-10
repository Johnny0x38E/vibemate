import {
    act,
    cleanup,
    fireEvent,
    render,
    screen,
} from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import { getAppInfo } from "../../lib/desktop";
import { getLogLocation, openLogFile } from "../../lib/desktop/logs";
import { SettingsView } from "./SettingsView";

vi.mock("../../lib/desktop", () => ({ getAppInfo: vi.fn() }));
vi.mock(import("../../lib/desktop/logs"), async (importOriginal) => ({
    ...(await importOriginal()),
    getLogLocation: vi.fn(),
    openLogFile: vi.fn(),
    openLogDirectory: vi.fn(),
}));
const readInfo = vi.mocked(getAppInfo);
beforeEach(() => {
    vi.mocked(getLogLocation).mockReset().mockResolvedValue(null);
    vi.mocked(openLogFile).mockReset().mockResolvedValue(undefined);
    readInfo.mockReset();
    readInfo.mockResolvedValue(null);
});
afterEach(cleanup);

test("log paths and pending opens survive tab and sidebar navigation", async () => {
    vi.mocked(getLogLocation).mockResolvedValue({
        filePath: "/logs/vibemate.log",
        directoryPath: "/logs",
        fileLoggingActive: true,
    });
    let resolveOpen: () => void = () => {
        throw new Error("Not initialized");
    };
    const pending = new Promise<void>((resolve) => {
        resolveOpen = resolve;
    });
    vi.mocked(openLogFile).mockReturnValue(pending);
    const instance = await createAppI18n("en");
    const content = (hidden: boolean) => (
        <I18nextProvider i18n={instance}>
            <SettingsView
                hidden={hidden}
                languageSettings={<span>Preferences</span>}
            />
        </I18nextProvider>
    );
    const { rerender } = render(content(false));
    await screen.findByText("/logs/vibemate.log");
    fireEvent.click(screen.getByRole("button", { name: "View logs" }));
    fireEvent.click(screen.getByRole("tab", { name: "About" }));
    rerender(content(true));
    await act(async () => {
        resolveOpen();
        await pending;
    });
    rerender(content(false));
    fireEvent.click(screen.getByRole("tab", { name: "General" }));
    expect(screen.getByText("/logs/vibemate.log")).toBeDefined();
    expect(
        screen
            .getByRole("button", { name: "View logs" })
            .getAttribute("aria-disabled"),
    ).toBe("false");
    expect(getLogLocation).toHaveBeenCalledTimes(1);
    expect(openLogFile).toHaveBeenCalledTimes(1);
});

test("switches between general and about settings tabs", async () => {
    const instance = await createAppI18n("en");
    render(
        <I18nextProvider i18n={instance}>
            <SettingsView
                languageSettings={
                    <p role="status" aria-label="Preference feedback">
                        General preferences stub
                    </p>
                }
            />
        </I18nextProvider>,
    );
    expect(
        screen.getByRole("heading", { name: "Settings", level: 1 }),
    ).toBeDefined();
    expect(
        screen.getByRole("status", { name: "Preference feedback" }).textContent,
    ).toBe("General preferences stub");
    fireEvent.click(screen.getByRole("tab", { name: "About" }));
    await screen.findByText(
        "Browser preview cannot read the desktop application version.",
    );
    expect(
        screen.getByRole("heading", { name: "vibemate", level: 2 }),
    ).toBeDefined();
    fireEvent.click(screen.getByRole("tab", { name: "General" }));
    expect(
        screen.getByRole("status", { name: "Preference feedback" }).textContent,
    ).toBe("General preferences stub");
});

test("translates settings tabs and about feedback", async () => {
    const instance = await createAppI18n("zh-CN");
    render(
        <I18nextProvider i18n={instance}>
            <SettingsView languageSettings={<span>语言区</span>} />
        </I18nextProvider>,
    );
    expect(
        screen.getByRole("heading", { name: "设置", level: 1 }),
    ).toBeDefined();
    expect(screen.getByRole("tab", { name: "常规" })).toBeDefined();
    expect(screen.getByRole("tab", { name: "关于" })).toBeDefined();
    fireEvent.click(screen.getByRole("tab", { name: "关于" }));
    expect(
        screen.getByRole("heading", { name: "vibemate", level: 2 }),
    ).toBeDefined();
    await act(async () => {
        await instance.changeLanguage("en");
    });
    expect(screen.getByRole("tab", { name: "General" })).toBeDefined();
    expect(screen.getByRole("tab", { name: "About" })).toBeDefined();
});

test("keyboard tabs lazily load metadata and preserve general inputs and loaded results", async () => {
    readInfo.mockResolvedValue({ name: "vibemate", version: "0.1.0" });
    const instance = await createAppI18n("en");
    render(
        <I18nextProvider i18n={instance}>
            <SettingsView
                languageSettings={
                    <input aria-label="Draft" defaultValue="unsaved" />
                }
            />
        </I18nextProvider>,
    );
    fireEvent.change(screen.getByRole("textbox", { name: "Draft" }), {
        target: { value: "edited draft" },
    });
    expect(readInfo).not.toHaveBeenCalled();
    const general = screen.getByRole("tab", { name: "General" });
    general.focus();
    fireEvent.keyDown(general, { key: "ArrowRight" });
    const about = screen.getByRole("tab", { name: "About" });
    expect(document.activeElement).toBe(about);
    expect(about.getAttribute("aria-selected")).toBe("true");
    await screen.findByText("0.1.0");
    fireEvent.keyDown(about, { key: "Home" });
    expect(document.activeElement).toBe(general);
    expect(screen.getByDisplayValue("edited draft")).toBeDefined();
    fireEvent.keyDown(general, { key: "End" });
    expect(screen.getByText("0.1.0")).toBeDefined();
    expect(readInfo).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(about, { key: "ArrowLeft" });
    expect(document.activeElement).toBe(general);
});
