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
import { AppearanceControl } from "./AppearanceControl";

afterEach(cleanup);

test("selects appearance from the dropdown", async () => {
    const instance = await createAppI18n("en");
    render(
        <I18nextProvider i18n={instance}>
            <AppearanceControl />
        </I18nextProvider>,
    );
    const select = screen.getByRole("combobox", { name: "Appearance" });
    fireEvent.change(select, { target: { value: "light" } });
    expect(document.documentElement.dataset["appearance"]).toBe("light");
    expect(select).toHaveProperty("value", "light");
    fireEvent.change(select, { target: { value: "dark" } });
    expect(document.documentElement.dataset["appearance"]).toBe("dark");
    fireEvent.change(select, { target: { value: "system" } });
    expect(document.documentElement.dataset["appearance"]).toBe("system");
});

test("keeps the selected appearance when language changes", async () => {
    const instance = await createAppI18n("en");
    render(
        <I18nextProvider i18n={instance}>
            <AppearanceControl />
        </I18nextProvider>,
    );
    fireEvent.change(screen.getByRole("combobox", { name: "Appearance" }), {
        target: { value: "light" },
    });
    await act(async () => {
        await instance.changeLanguage("zh-CN");
    });
    expect(screen.getByRole("combobox", { name: "外观" })).toHaveProperty(
        "value",
        "light",
    );
    expect(document.documentElement.dataset["appearance"]).toBe("light");
});

test("restores the previous document attribute when its owner unmounts", async () => {
    document.documentElement.dataset["appearance"] = "dark";
    const instance = await createAppI18n("en");
    const view = render(
        <I18nextProvider i18n={instance}>
            <AppearanceControl />
        </I18nextProvider>,
    );
    fireEvent.change(screen.getByRole("combobox", { name: "Appearance" }), {
        target: { value: "light" },
    });
    view.unmount();
    expect(document.documentElement.dataset["appearance"]).toBe("dark");
    delete document.documentElement.dataset["appearance"];
});
