import {
    act,
    cleanup,
    fireEvent,
    render,
    screen,
} from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { afterEach, expect, test } from "vitest";
import { createAppI18n } from "../../i18n";
import { SettingsView } from "./SettingsView";

afterEach(cleanup);

test("switches between general and about settings tabs", async () => {
    const instance = await createAppI18n("en");
    render(
        <I18nextProvider i18n={instance}>
            <SettingsView
                languageSettings={<p role="status">General preferences stub</p>}
            />
        </I18nextProvider>,
    );
    expect(
        screen.getByRole("heading", { name: "Settings", level: 1 }),
    ).toBeDefined();
    expect(screen.getByRole("status").textContent).toBe(
        "General preferences stub",
    );
    fireEvent.click(screen.getByRole("tab", { name: "About" }));
    expect(screen.queryByRole("status")).toBeNull();
    expect(
        screen.getByRole("heading", { name: "Not implemented", level: 2 }),
    ).toBeDefined();
    fireEvent.click(screen.getByRole("tab", { name: "General" }));
    expect(screen.getByRole("status").textContent).toBe(
        "General preferences stub",
    );
});

test("translates settings tabs and the about placeholder", async () => {
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
        screen.getByRole("heading", { name: "功能尚未实现", level: 2 }),
    ).toBeDefined();
    await act(async () => {
        await instance.changeLanguage("en");
    });
    expect(screen.getByRole("tab", { name: "General" })).toBeDefined();
    expect(screen.getByRole("tab", { name: "About" })).toBeDefined();
});
