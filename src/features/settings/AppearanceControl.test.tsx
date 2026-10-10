import {
    act,
    cleanup,
    fireEvent,
    render,
    screen,
    waitFor,
} from "@testing-library/react";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import { setFieldSelectValue } from "../../test/fieldSelect";
import {
    AppearanceRequestError,
    getAppearancePreference,
    saveAppearancePreference,
    type AppearancePreferenceResult,
    type AppearancePreference,
} from "../../lib/desktop/appearance";
import { AppearanceControl } from "./AppearanceControl";

vi.mock(import("../../lib/desktop/appearance"), async (importOriginal) => ({
    ...(await importOriginal()),
    getAppearancePreference: vi.fn(),
    saveAppearancePreference: vi.fn(),
}));
const read = vi.mocked(getAppearancePreference);
const save = vi.mocked(saveAppearancePreference);
beforeEach(() => {
    read.mockReset();
    save.mockReset();
    read.mockResolvedValue({ kind: "preview" });
});
afterEach(cleanup);

function deferred<T>() {
    let resolve: (value: T) => void = () => {
        throw new Error("Not initialized");
    };
    const promise = new Promise<T>((res) => {
        resolve = res;
    });
    return { promise, resolve };
}
async function mount(locale: "en" | "zh-CN" = "en") {
    const instance = await createAppI18n(locale);
    const view = render(
        <StrictMode>
            <I18nextProvider i18n={instance}>
                <AppearanceControl />
                <input aria-label="Draft" defaultValue="" />
            </I18nextProvider>
        </StrictMode>,
    );
    return { instance, ...view };
}
async function ready(label = "Iris") {
    const select = screen.getByRole("radio", { name: label });
    await waitFor(() => {
        expect(select).toHaveProperty("disabled", false);
    });
    return select;
}

test("preview tries built-in palettes and brightness without invoking save", async () => {
    await mount();
    const themes = await ready();
    fireEvent.click(themes);
    setFieldSelectValue(
        screen.getByRole("combobox", { name: "Appearance" }),
        "dark",
    );
    expect(document.documentElement.dataset["theme"]).toBe("iris");
    expect(document.documentElement.dataset["appearance"]).toBe("dark");
    expect(screen.getByRole("status").textContent).toContain("not saved");
    expect(save).not.toHaveBeenCalled();
});

test("language changes retain selected colors and edited sibling input", async () => {
    const { instance } = await mount();
    const select = await ready("Linen");
    fireEvent.click(select);
    fireEvent.change(screen.getByRole("textbox"), {
        target: { value: "editing" },
    });
    await act(async () => {
        await instance.changeLanguage("zh-CN");
    });
    expect(screen.getByRole("radio", { name: "亚麻" })).toBe(select);
    expect(select).toHaveProperty("checked", true);
    expect(screen.getByDisplayValue("editing")).toBeDefined();
});

test("restores document attributes on unmount", async () => {
    document.documentElement.dataset["appearance"] = "dark";
    document.documentElement.dataset["theme"] = "ocean";
    const view = await mount();
    await ready();
    view.unmount();
    expect(document.documentElement.dataset["appearance"]).toBe("dark");
    expect(document.documentElement.dataset["theme"]).toBe("ocean");
    delete document.documentElement.dataset["appearance"];
    delete document.documentElement.dataset["theme"];
});

test("loads the saved pair and applies changes only after confirmation", async () => {
    read.mockResolvedValue({
        kind: "desktop",
        preference: { appearance: "dark", theme: "ocean" },
    });
    const request = deferred<AppearancePreference>();
    save.mockReturnValue(request.promise);
    await mount();
    const select = await ready();
    expect(document.documentElement.dataset["theme"]).toBe("ocean");
    fireEvent.click(select);
    expect(select).toHaveProperty("disabled", true);
    expect(document.documentElement.dataset["theme"]).toBe("ocean");
    fireEvent.click(screen.getByRole("radio", { name: "Linen" }));
    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith({ appearance: "dark", theme: "iris" });
    expect(screen.queryByRole("status")).toBeNull();
    await act(async () => {
        request.resolve({ appearance: "dark", theme: "iris" });
        await request.promise;
    });
    expect(document.documentElement.dataset["theme"]).toBe("iris");
    expect(select).toHaveProperty("disabled", false);
});

test("known failed writes keep the previous pair and allow another attempt", async () => {
    read.mockResolvedValue({
        kind: "desktop",
        preference: { appearance: "light", theme: "forest" },
    });
    save.mockRejectedValue(new AppearanceRequestError("write_failed"));
    await mount("zh-CN");
    const select = await ready("亚麻");
    fireEvent.click(select);
    expect((await screen.findByRole("alert")).textContent).toContain(
        "原配色未改变",
    );
    expect(screen.getByRole("radio", { name: "森林" })).toHaveProperty(
        "checked",
        true,
    );
    expect(select).toHaveProperty("disabled", false);
});

test("unknown saves block further writes until the persisted pair is reloaded", async () => {
    read.mockResolvedValue({
        kind: "desktop",
        preference: { appearance: "light", theme: "forest" },
    });
    save.mockRejectedValue(new AppearanceRequestError("invalid_response"));
    await mount();
    const select = await ready();
    fireEvent.click(select);
    await screen.findByRole("alert");
    expect(select).toHaveProperty("disabled", true);
    fireEvent.click(screen.getByRole("radio", { name: "Ocean" }));
    expect(save).toHaveBeenCalledTimes(1);
    read.mockResolvedValue({
        kind: "desktop",
        preference: { appearance: "light", theme: "iris" },
    });
    fireEvent.click(
        screen.getByRole("button", { name: "Reload saved appearance" }),
    );
    await ready();
    expect(select).toHaveProperty("checked", true);
});

test("read failures are safe and disable changes until retry succeeds", async () => {
    read.mockRejectedValue(new Error("private diagnostic"));
    await mount();
    await screen.findByRole("alert");
    expect(screen.queryByText(/private diagnostic/)).toBeNull();
    expect(screen.getByRole("radio", { name: "Iris" })).toHaveProperty(
        "disabled",
        true,
    );
    read.mockResolvedValue({ kind: "preview" });
    fireEvent.click(
        screen.getByRole("button", { name: "Reload saved appearance" }),
    );
    await ready();
});

test("obsolete StrictMode reads cannot replace the current saved theme", async () => {
    const old = deferred<AppearancePreferenceResult>();
    const current = deferred<AppearancePreferenceResult>();
    read.mockReturnValueOnce(old.promise).mockReturnValueOnce(current.promise);
    await mount();
    await act(async () => {
        current.resolve({
            kind: "desktop",
            preference: { appearance: "dark", theme: "iris" },
        });
        await current.promise;
    });
    await act(async () => {
        old.resolve({
            kind: "desktop",
            preference: { appearance: "light", theme: "forest" },
        });
        await old.promise;
    });
    expect(document.documentElement.dataset["theme"]).toBe("iris");
    expect(document.documentElement.dataset["appearance"]).toBe("dark");
});

test("late saves after unmount cannot change the document colors", async () => {
    read.mockResolvedValue({
        kind: "desktop",
        preference: { appearance: "system", theme: "forest" },
    });
    const request = deferred<AppearancePreference>();
    save.mockReturnValue(request.promise);
    const view = await mount();
    const select = await ready();
    fireEvent.click(select);
    view.unmount();
    await act(async () => {
        request.resolve({ appearance: "system", theme: "iris" });
        await request.promise;
    });
    expect(document.documentElement.dataset["theme"]).toBeUndefined();
});

test("shows all built-in colors together and moves the single selection on click", async () => {
    await mount();
    await ready();
    const group = screen.getByRole("radiogroup", { name: "Color theme" });
    expect(group.querySelectorAll('input[type="radio"]')).toHaveLength(6);
    expect(screen.getByRole("radio", { name: "Forest" })).toHaveProperty(
        "checked",
        true,
    );
    for (const name of ["Graphite", "Linen", "Iris", "Ocean", "Ink"]) {
        fireEvent.click(screen.getByRole("radio", { name }));
        expect(screen.getByRole("radio", { name })).toHaveProperty(
            "checked",
            true,
        );
        expect(group.querySelectorAll("input:checked")).toHaveLength(1);
    }
    expect(save).not.toHaveBeenCalled();
});

test("saves Ink with the current brightness and restores it after remount", async () => {
    read.mockResolvedValue({
        kind: "desktop",
        preference: { appearance: "dark", theme: "forest" },
    });
    const choice = { appearance: "dark", theme: "notion" } as const;
    save.mockResolvedValue(choice);
    const view = await mount();
    const ink = await ready("Ink");
    expect(screen.getAllByRole("radio")[1]).toBe(ink);
    fireEvent.click(ink);
    await waitFor(() => {
        expect(document.documentElement.dataset["theme"]).toBe("notion");
    });
    expect(save).toHaveBeenCalledExactlyOnceWith(choice);
    view.unmount();
    read.mockResolvedValue({ kind: "desktop", preference: choice });
    await mount("zh-CN");
    const selected = await ready("纸墨");
    expect(selected).toHaveProperty("checked", true);
    expect(document.documentElement.dataset["appearance"]).toBe("dark");
    expect(document.documentElement.dataset["theme"]).toBe("notion");
    expect(save).toHaveBeenCalledTimes(1);
});
