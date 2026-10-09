import { StrictMode } from "react";
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
import {
    getLocalePreference,
    saveLocalePreference,
    SettingsRequestError,
    type LocalePreference,
    type LocalePreferenceResult,
} from "../../lib/desktop/settings";
import { LanguageSelector } from "./LanguageSelector";

// Keep React and translations real; replace only the Rust IPC boundary.
vi.mock(import("../../lib/desktop/settings"), async (importOriginal) => ({
    ...(await importOriginal()),
    getLocalePreference: vi.fn<typeof getLocalePreference>(),
    saveLocalePreference: vi.fn<typeof saveLocalePreference>(),
}));

beforeEach(() => {
    vi.mocked(getLocalePreference).mockReset();
    vi.mocked(saveLocalePreference).mockReset();
});
afterEach(cleanup);

function pendingPreference(): {
    promise: Promise<LocalePreference>;
    resolve: (preference: LocalePreference) => void;
} {
    let resolveResponse: (preference: LocalePreference) => void = () => {
        throw new Error("The pending response has not been initialized.");
    };
    const promise = new Promise<LocalePreference>((resolve) => {
        resolveResponse = resolve;
    });
    return { promise, resolve: resolveResponse };
}

async function renderSelector(
    initialPreference: LocalePreferenceResult = {
        kind: "desktop",
        preference: "en",
    },
    systemLanguage = "zh-TW",
) {
    const instance = await createAppI18n("en");
    const view = render(
        <I18nextProvider i18n={instance}>
            <LanguageSelector
                initialPreference={initialPreference}
                systemLanguage={systemLanguage}
            />
        </I18nextProvider>,
    );
    return { instance, ...view };
}

test("keeps the previous choice and offers another attempt after a definite write failure", async () => {
    vi.mocked(saveLocalePreference).mockRejectedValueOnce(
        new SettingsRequestError("write_failed"),
    );
    const { instance } = await renderSelector();
    fireEvent.change(screen.getByRole("combobox"), {
        target: { value: "zh-CN" },
    });
    await screen.findByText(
        "The language preference could not be saved. Your previous choice is unchanged; try again.",
    );
    expect(screen.getByRole("combobox")).toHaveProperty("value", "en");
    expect(instance.resolvedLanguage).toBe("en");
    vi.mocked(saveLocalePreference).mockResolvedValueOnce("zh-CN");
    fireEvent.change(screen.getByRole("combobox"), {
        target: { value: "zh-CN" },
    });
    await screen.findByRole("combobox", { name: "语言" });
    expect(screen.getByRole("combobox")).toHaveProperty("value", "zh-CN");
});

test("blocks further writes after an unknown outcome until the saved choice is reloaded", async () => {
    vi.mocked(saveLocalePreference).mockRejectedValue(
        new Error("Synthetic private diagnostic"),
    );
    const { instance } = await renderSelector();
    fireEvent.change(screen.getByRole("combobox"), {
        target: { value: "zh-CN" },
    });
    await screen.findByText(
        "The saved language preference could not be confirmed. Reload it before making another change.",
    );
    expect(screen.getByRole("combobox")).toHaveProperty("disabled", true);
    expect(instance.resolvedLanguage).toBe("en");
    expect(screen.queryByText("Synthetic private diagnostic")).toBeNull();
    vi.mocked(getLocalePreference).mockResolvedValue({
        kind: "desktop",
        preference: "zh-CN",
    });
    fireEvent.click(
        screen.getByRole("button", { name: "Reload saved preference" }),
    );
    await screen.findByRole("combobox", { name: "语言" });
    expect(screen.getByRole("combobox", { name: "语言" })).toHaveProperty(
        "value",
        "zh-CN",
    );
    expect(screen.getByRole("combobox")).toHaveProperty("disabled", false);
});

test("prevents browser preview writes and translates its explanation", async () => {
    const { instance } = await renderSelector({ kind: "preview" });
    expect(screen.getByRole("combobox")).toHaveProperty("disabled", true);
    expect(screen.getByRole("status").textContent).toBe(
        "Browser preview cannot save language preferences.",
    );
    fireEvent.change(screen.getByRole("combobox"), {
        target: { value: "zh-CN" },
    });
    expect(saveLocalePreference).not.toHaveBeenCalled();
    await act(async () => {
        await instance.changeLanguage("zh-CN");
    });
    expect(screen.getByRole("combobox", { name: "语言" })).toHaveProperty(
        "disabled",
        true,
    );
    expect(screen.getByRole("status").textContent).toBe(
        "浏览器预览无法保存语言偏好。",
    );
});

test("explicit choices override the system and follow system resolves Chinese variants", async () => {
    await renderSelector({ kind: "desktop", preference: "en" }, "zh-Hant-HK");
    vi.mocked(saveLocalePreference).mockResolvedValueOnce("zh-CN");
    fireEvent.change(screen.getByRole("combobox"), {
        target: { value: "zh-CN" },
    });
    await screen.findByRole("combobox", { name: "语言" });
    vi.mocked(saveLocalePreference).mockResolvedValueOnce("en");
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "en" } });
    await screen.findByRole("combobox", { name: "Language" });
    expect(screen.getByRole("combobox", { name: "Language" })).toHaveProperty(
        "value",
        "en",
    );
    vi.mocked(saveLocalePreference).mockResolvedValueOnce("system");
    fireEvent.change(screen.getByRole("combobox"), {
        target: { value: "system" },
    });
    await screen.findByRole("combobox", { name: "语言" });
    expect(screen.getByRole("combobox", { name: "语言" })).toHaveProperty(
        "value",
        "system",
    );
});

test("does not save when the current choice is selected again", async () => {
    await renderSelector();
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "en" } });
    expect(saveLocalePreference).not.toHaveBeenCalled();
});

test("keeps writes blocked after a reload failure and permits a later reload", async () => {
    vi.mocked(saveLocalePreference).mockRejectedValue(
        new SettingsRequestError("invalid_response"),
    );
    const { instance } = await renderSelector();
    fireEvent.change(screen.getByRole("combobox"), {
        target: { value: "zh-CN" },
    });
    const retry = await screen.findByRole("button", {
        name: "Reload saved preference",
    });
    vi.mocked(getLocalePreference).mockRejectedValueOnce(
        new SettingsRequestError("read_failed"),
    );
    fireEvent.click(retry);
    await screen.findByText(
        "The saved language preference could not be read. Try reloading it again.",
    );
    expect(screen.getByRole("combobox")).toHaveProperty("disabled", true);
    expect(instance.resolvedLanguage).toBe("en");
    vi.mocked(getLocalePreference).mockResolvedValueOnce({
        kind: "desktop",
        preference: "system",
    });
    fireEvent.click(
        screen.getByRole("button", { name: "Reload saved preference" }),
    );
    await screen.findByRole("combobox", { name: "Language" });
    expect(screen.getByRole("combobox")).toHaveProperty("value", "system");
});

test("distinguishes a committed save from a translator failure", async () => {
    const instance = await createAppI18n("en");
    // This is a failure of the external i18next service, not a fake translator:
    // only its next language change rejects; the real provider keeps rendering.
    const change = vi
        .spyOn(instance, "changeLanguage")
        .mockRejectedValueOnce(new Error("Synthetic translation failure"));
    // react-i18next 17 wraps instance methods when the hook mounts, so configure
    // the external failure before rendering rather than patching a captured method.
    render(
        <I18nextProvider i18n={instance}>
            <LanguageSelector
                initialPreference={{ kind: "desktop", preference: "en" }}
                systemLanguage="en"
            />
        </I18nextProvider>,
    );
    vi.mocked(saveLocalePreference).mockResolvedValue("zh-CN");
    fireEvent.change(screen.getByRole("combobox"), {
        target: { value: "zh-CN" },
    });
    await screen.findByText(
        "The preference is saved, but the interface language could not be changed. Reload the saved preference to retry.",
    );
    expect(screen.getByRole("combobox")).toHaveProperty("value", "zh-CN");
    expect(screen.getByRole("combobox")).toHaveProperty("disabled", true);
    vi.mocked(getLocalePreference).mockResolvedValue({
        kind: "desktop",
        preference: "zh-CN",
    });
    fireEvent.click(
        screen.getByRole("button", { name: "Reload saved preference" }),
    );
    await screen.findByRole("combobox", { name: "语言" });
    change.mockRestore();
});

test("keeps the Chinese choice and gives a Chinese explanation after a failed save", async () => {
    const instance = await createAppI18n("zh-CN");
    vi.mocked(saveLocalePreference).mockRejectedValue(
        new SettingsRequestError("storage_unavailable"),
    );
    render(
        <I18nextProvider i18n={instance}>
            <LanguageSelector
                initialPreference={{ kind: "desktop", preference: "zh-CN" }}
                systemLanguage="en-US"
            />
        </I18nextProvider>,
    );
    fireEvent.change(screen.getByRole("combobox", { name: "语言" }), {
        target: { value: "en" },
    });
    await screen.findByText("无法保存语言偏好，原选择未改变，请重试。");
    expect(screen.getByRole("combobox")).toHaveProperty("value", "zh-CN");
    expect(instance.resolvedLanguage).toBe("zh-CN");
});

test("stops saving if the desktop boundary becomes unavailable", async () => {
    vi.mocked(saveLocalePreference).mockRejectedValue(
        new SettingsRequestError("desktop_required"),
    );
    await renderSelector();
    fireEvent.change(screen.getByRole("combobox"), {
        target: { value: "zh-CN" },
    });
    await screen.findByText(
        "Browser preview cannot save language preferences.",
    );
    expect(screen.getByRole("combobox")).toHaveProperty("disabled", true);
    expect(screen.getByRole("combobox")).toHaveProperty("value", "en");
});

test("prevents duplicate reloads and ignores a reload completed after unmount", async () => {
    vi.mocked(saveLocalePreference).mockRejectedValue(
        new SettingsRequestError("operation_failed"),
    );
    const { instance, unmount } = await renderSelector();
    fireEvent.change(screen.getByRole("combobox"), {
        target: { value: "zh-CN" },
    });
    const retry = await screen.findByRole("button", {
        name: "Reload saved preference",
    });
    const response = pendingPreference();
    vi.mocked(getLocalePreference).mockReturnValue(
        response.promise.then((preference) => ({
            kind: "desktop",
            preference,
        })),
    );
    fireEvent.click(retry);
    expect(retry).toHaveProperty("disabled", true);
    fireEvent.click(retry);
    expect(getLocalePreference).toHaveBeenCalledTimes(1);
    unmount();
    await act(async () => {
        response.resolve("zh-CN");
        await response.promise;
    });
    expect(instance.resolvedLanguage).toBe("en");
});

test("does not change the shared translator after unmounting during a save", async () => {
    const response = pendingPreference();
    vi.mocked(saveLocalePreference).mockReturnValue(response.promise);
    const { instance, unmount } = await renderSelector();
    fireEvent.change(screen.getByRole("combobox"), {
        target: { value: "zh-CN" },
    });
    unmount();
    await act(async () => {
        response.resolve("zh-CN");
        await response.promise;
    });
    expect(instance.resolvedLanguage).toBe("en");
});

test("waits for a saved choice before switching language and keeps other input", async () => {
    const instance = await createAppI18n("en");
    const response = pendingPreference();
    vi.mocked(saveLocalePreference).mockReturnValue(response.promise);
    render(
        <StrictMode>
            <I18nextProvider i18n={instance}>
                <input
                    aria-label="Unsubmitted model ID"
                    defaultValue="deepseek-chat"
                />
                <LanguageSelector
                    initialPreference={{
                        kind: "desktop",
                        preference: "system",
                    }}
                    systemLanguage="en-US"
                />
            </I18nextProvider>
        </StrictMode>,
    );
    const model = screen.getByRole("textbox", { name: "Unsubmitted model ID" });
    fireEvent.change(model, { target: { value: "custom-model-id" } });
    const select = screen.getByRole("combobox", { name: "Language" });
    fireEvent.change(select, { target: { value: "zh-CN" } });
    expect(select).toHaveProperty("disabled", true);
    expect(select).toHaveProperty("value", "system");
    expect(instance.resolvedLanguage).toBe("en");
    // Even a second dispatched change must not produce a concurrent write.
    fireEvent.change(select, { target: { value: "en" } });
    expect(saveLocalePreference).toHaveBeenCalledTimes(1);
    await act(async () => {
        response.resolve("zh-CN");
        await response.promise;
    });
    expect(screen.getByRole("combobox", { name: "语言" })).toHaveProperty(
        "value",
        "zh-CN",
    );
    expect(model).toHaveProperty("value", "custom-model-id");
    expect(screen.getByRole("textbox", { name: "Unsubmitted model ID" })).toBe(
        model,
    );
});
