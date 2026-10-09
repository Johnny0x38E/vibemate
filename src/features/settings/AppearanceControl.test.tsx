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

test("cycles this window's appearance and keeps the same focused button", async () => {
    const instance = await createAppI18n("en");
    render(
        <I18nextProvider i18n={instance}>
            <AppearanceControl />
        </I18nextProvider>,
    );
    const button = screen.getByRole("button", {
        name: "Appearance: Follow system; switch to Light",
    });
    button.focus();
    fireEvent.click(button);
    expect(document.documentElement.dataset["appearance"]).toBe("light");
    expect(
        screen.getByRole("button", {
            name: "Appearance: Light; switch to Dark",
        }),
    ).toBe(button);
    fireEvent.click(button);
    expect(document.documentElement.dataset["appearance"]).toBe("dark");
    fireEvent.click(button);
    expect(document.documentElement.dataset["appearance"]).toBe("system");
    expect(document.activeElement).toBe(button);
});

test("updates current and next mode names without resetting appearance when language changes", async () => {
    const instance = await createAppI18n("en");
    render(
        <I18nextProvider i18n={instance}>
            <AppearanceControl />
        </I18nextProvider>,
    );
    fireEvent.click(screen.getByRole("button"));
    await act(async () => {
        await instance.changeLanguage("zh-CN");
    });
    expect(
        screen.getByRole("button", { name: "外观：浅色；切换为深色" }),
    ).toHaveProperty("title", "外观：浅色；切换为深色");
    expect(screen.getByRole("status").textContent).toBe("当前外观：浅色。");
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
    fireEvent.click(screen.getByRole("button"));
    view.unmount();
    expect(document.documentElement.dataset["appearance"]).toBe("dark");
    delete document.documentElement.dataset["appearance"];
});
