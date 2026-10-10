import {
    act,
    cleanup,
    fireEvent,
    render,
    screen,
} from "@testing-library/react";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import {
    createProvider,
    getProvider,
    ProviderRequestError,
    updateProvider,
    type ProviderRecord,
    type ProviderTemplate,
} from "../../lib/desktop/providers";
import { getProviderSecretStatus } from "../../lib/desktop/providerSecrets";
import {
    fieldSelectOptionLabels,
    fieldSelectValue,
    setFieldSelectValue,
} from "../../test/fieldSelect";
import { ProviderForm, type ProviderFormMode } from "./ProviderForm";

vi.mock(
    import("../../lib/desktop/providerSecrets"),
    async (importOriginal) => ({
        ...(await importOriginal()),
        getProviderSecretStatus: vi.fn(),
    }),
);

// Keep the real error class and types; only the IPC calls are replaced.
vi.mock(import("../../lib/desktop/providers"), async (importOriginal) => ({
    ...(await importOriginal()),
    createProvider: vi.fn(),
    getProvider: vi.fn(),
    updateProvider: vi.fn(),
}));
const create = vi.mocked(createProvider);
const update = vi.mocked(updateProvider);
const read = vi.mocked(getProvider);

afterEach(cleanup);
beforeEach(() => {
    vi.mocked(getProviderSecretStatus)
        .mockReset()
        .mockImplementation((providerId) =>
            Promise.resolve({
                kind: "desktop",
                status: { providerId, state: "set", updatedAtMs: 1000 },
            }),
        );
    create.mockReset();
    update.mockReset();
    read.mockReset();
});

const templates: ProviderTemplate[] = [
    {
        kind: "deepseek",
        brandName: "DeepSeek",
        defaultBaseUrl: "https://api.deepseek.com",
        protocols: ["chat_completions"],
        defaultProtocol: "chat_completions",
        extensionFields: [],
    },
    {
        kind: "openrouter",
        brandName: "OpenRouter",
        defaultBaseUrl: "https://openrouter.ai/api/v1",
        // A second protocol lets tests observe the protocol select.
        protocols: ["chat_completions", "responses"],
        defaultProtocol: "chat_completions",
        extensionFields: [],
    },
];

const ID = "0123456789abcdef0123456789abcdef";
// Synthetic; never a real key.
const SECRET = "sk-synthetic-form-2222";
const KEY_LABEL = /^(API key|API 密钥)$/;
const SAVE_BUTTON = /^(Save|保存|Save again|再次保存)$/;

function providerCombobox(label = "Provider"): HTMLElement {
    return screen.getByRole("combobox", { name: label });
}

function providerKindValue(label = "Provider"): string {
    return fieldSelectValue(providerCombobox(label));
}

function chooseProvider(kind: string, label = "Provider"): void {
    setFieldSelectValue(providerCombobox(label), kind);
}

const saved: ProviderRecord = {
    id: ID,
    kind: "deepseek",
    displayName: "Work",
    baseUrl: "https://api.deepseek.com",
    protocol: "chat_completions",
    extensions: {},
    revision: 3,
    createdAtMs: 1000,
    updatedAtMs: 2000,
};

function deferred<T>() {
    let resolve: (value: T) => void = () => {
        throw new Error("Not initialized");
    };
    let reject: (reason: unknown) => void = () => {
        throw new Error("Not initialized");
    };
    const promise = new Promise<T>((res, rej) => {
        resolve = res;
        reject = rej;
    });
    return { promise, resolve, reject };
}

async function mount(
    mode: ProviderFormMode = { kind: "create" },
    locale: "en" | "zh-CN" = "en",
    refresh: () => Promise<boolean> = () => Promise.resolve(true),
    hidden = false,
) {
    const instance = await createAppI18n(locale);
    const onSaved = vi.fn<(record: ProviderRecord) => void>();
    const onRecordLoaded = vi.fn<(record: ProviderRecord) => void>();
    const onRefresh = vi.fn(refresh);
    const onCancel = vi.fn<() => void>();
    const onBusyChange = vi.fn<(busy: boolean) => void>();
    const tree = (isHidden: boolean) => (
        <StrictMode>
            <I18nextProvider i18n={instance}>
                <ProviderForm
                    templates={templates}
                    mode={mode}
                    onSaved={onSaved}
                    onRecordLoaded={onRecordLoaded}
                    onRefresh={onRefresh}
                    onCancel={onCancel}
                    onBusyChange={onBusyChange}
                    hidden={isHidden}
                />
            </I18nextProvider>
        </StrictMode>
    );
    const { rerender, unmount } = render(tree(hidden));
    await act(async () => {
        await Promise.resolve();
    });
    return {
        unmount,
        setHidden: (isHidden: boolean) => {
            rerender(tree(isHidden));
        },
        instance,
        onSaved,
        onRecordLoaded,
        onRefresh,
        onCancel,
        onBusyChange,
    };
}

function input(label: string): HTMLElement {
    return screen.getByLabelText(label);
}

function fieldValue(label: string): string {
    const element = input(label);
    if (element instanceof HTMLInputElement) return element.value;
    if (element instanceof HTMLSelectElement) return element.value;
    if (element.getAttribute("role") === "combobox")
        return fieldSelectValue(element);
    throw new Error(`${label} is not a supported field`);
}

function setField(label: string, value: string): void {
    const element = input(label);
    if (
        element instanceof HTMLInputElement ||
        element instanceof HTMLSelectElement
    ) {
        fireEvent.change(element, { target: { value } });
        return;
    }
    if (element.getAttribute("role") === "combobox") {
        setFieldSelectValue(element, value);
        return;
    }
    throw new Error(`${label} is not a supported field`);
}

function type(label: string, value: string): void {
    setField(label, value);
}

/**
 * The key field is empty both as a property and as the `value` attribute that
 * React mirrors into the DOM, so no copy of the key is left in the markup.
 */
function expectKeyCleared(): void {
    expect(keyField().value).toBe("");
    expect(keyField().getAttribute("value") ?? "").toBe("");
}

/** The create form's key field. */
function keyField(): HTMLInputElement {
    const element = screen.getByLabelText(KEY_LABEL);
    if (!(element instanceof HTMLInputElement))
        throw new Error("The API key field is not an input");
    return element;
}

/**
 * Click a button. A save on the create form first types the synthetic key,
 * because every submit clears it; pass `key: null` to submit without one.
 */
async function submit(
    name = "Save",
    key: string | null = SECRET,
): Promise<void> {
    if (
        key !== null &&
        SAVE_BUTTON.test(name) &&
        screen.queryByLabelText(KEY_LABEL) !== null &&
        keyField().required
    ) {
        fireEvent.change(keyField(), { target: { value: key } });
    }
    await act(async () => {
        fireEvent.click(screen.getByRole("button", { name }));
        await Promise.resolve();
    });
}

test("starts with the first template selected and its defaults filled", async () => {
    await mount();
    const kind = providerCombobox();
    // The provider is the first field, with no empty placeholder option.
    expect(screen.getAllByRole("combobox")[0]).toBe(kind);
    expect(fieldSelectValue(kind)).toBe("deepseek");
    fireEvent.click(kind);
    expect(
        screen.getAllByRole("option").map((option) => option.textContent),
    ).toContain("OpenRouter");
    fireEvent.click(kind);
    expect(fieldValue("Name")).toBe("DeepSeek");
    expect(fieldValue("Base URL")).toBe("https://api.deepseek.com");
    expect(fieldValue("Protocol")).toBe("chat_completions");
    expect(
        screen.getByRole("form", { name: "Basic information" }),
    ).toBeDefined();
    // The required key is part of the create form: a masked, empty field
    // that browsers are asked not to fill with a saved password.
    const key = keyField();
    expect(key.type).toBe("password");
    expect(key.value).toBe("");
    expect(key.getAttribute("autocomplete")).toBe("new-password");
    expect(key.getAttribute("spellcheck")).toBe("false");
    expect(key.required).toBe(true);
    // It belongs to the same form, without a persistent storage/clearing hint.
    expect(
        input("Protocol").compareDocumentPosition(key) &
            Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(key.getAttribute("aria-describedby")).toBeNull();
});

test("switching provider replaces untouched defaults and only allowed protocols are listed", async () => {
    await mount();
    chooseProvider("openrouter");
    expect(fieldValue("Name")).toBe("OpenRouter");
    expect(fieldValue("Base URL")).toBe("https://openrouter.ai/api/v1");
    expect(fieldValue("Protocol")).toBe("chat_completions");
    expect(fieldSelectOptionLabels(input("Protocol"))).toEqual([
        "Chat Completions",
        "Responses",
    ]);

    chooseProvider("deepseek");
    expect(fieldValue("Name")).toBe("DeepSeek");
    expect(fieldValue("Base URL")).toBe("https://api.deepseek.com");
    expect(fieldSelectOptionLabels(input("Protocol"))).toEqual([
        "Chat Completions",
    ]);
});

test("switching provider keeps the fields the user edited", async () => {
    await mount();
    type("Name", "My router");
    chooseProvider("openrouter");
    // The edited name stays; the untouched URL follows the new provider.
    expect(fieldValue("Name")).toBe("My router");
    expect(fieldValue("Base URL")).toBe("https://openrouter.ai/api/v1");
    type("Base URL", "https://proxy.example.com/v1");
    setField("Protocol", "responses");

    chooseProvider("deepseek");
    expect(fieldValue("Name")).toBe("My router");
    expect(fieldValue("Base URL")).toBe("https://proxy.example.com/v1");
    // A chosen protocol the new provider does not allow falls back to its default.
    expect(fieldValue("Protocol")).toBe("chat_completions");
});

test("creates for the selected provider and reports the normalized record Rust returns", async () => {
    const normalized: ProviderRecord = {
        ...saved,
        kind: "openrouter",
        displayName: "Team router",
        baseUrl: "https://openrouter.ai/api/v1",
        protocol: "responses",
        revision: 1,
    };
    create.mockResolvedValue(normalized);
    const { onSaved } = await mount();
    chooseProvider("openrouter");
    type("Name", "  Team router  ");
    type("Base URL", "HTTPS://OpenRouter.ai/api/v1/");
    setField("Protocol", "responses");
    await submit();

    expect(create).toHaveBeenCalledTimes(1);
    // The key is sent as typed, with the other fields, in the same request.
    expect(create).toHaveBeenCalledWith({
        kind: "openrouter",
        displayName: "  Team router  ",
        baseUrl: "HTTPS://OpenRouter.ai/api/v1/",
        protocol: "responses",
        extensions: {},
        secret: SECRET,
    });
    // The parent leaves this page and shows these normalized values.
    expect(onSaved).toHaveBeenCalledExactlyOnceWith(normalized);
    expectKeyCleared();
});

test.each([
    ["base_url_not_https", "The base URL must start with https://."],
    [
        "base_url_invalid",
        "Enter a valid base URL without spaces, a query (?) or fragment (#), up to 2048 characters.",
    ],
    [
        "base_url_has_credentials",
        "The base URL must not contain a user name or password.",
    ],
] as const)(
    "shows %s next to the base URL and clears it on edit",
    async (code, message) => {
        create.mockRejectedValue(new ProviderRequestError(code));
        const { onSaved } = await mount();
        type("Base URL", "http://user:pass@example.com?q=1");
        await submit();

        const alert = screen.getByRole("alert");
        expect(alert.textContent).toBe(message);
        const field = input("Base URL");
        expect(field.getAttribute("aria-invalid")).toBe("true");
        expect(field.getAttribute("aria-describedby")).toContain(alert.id);
        expect(onSaved).not.toHaveBeenCalled();
        // The key never reaches the page text, even next to an error.
        expect(document.body.textContent).not.toContain(SECRET);
        expect(keyField().value).toBe("");

        type("Base URL", "https://example.com");
        expect(screen.queryByRole("alert")).toBeNull();
        expect(input("Base URL").getAttribute("aria-invalid")).toBe("false");
    },
);

test("shows field errors in Chinese for the name", async () => {
    create.mockRejectedValue(new ProviderRequestError("display_name_invalid"));
    await mount({ kind: "create" }, "zh-CN");
    type("名称", " ");
    await submit("保存");
    expect(screen.getByRole("alert").textContent).toBe(
        "名称须为 1–64 个可见字符，不能包含换行、控制字符或不可见的格式字符。",
    );
    expect(input("名称").getAttribute("aria-invalid")).toBe("true");
    expect(screen.getByRole("heading", { name: "基本信息" })).toBeDefined();
});

test("a definite write failure is shown at form level and can be retried", async () => {
    create
        .mockRejectedValueOnce(new ProviderRequestError("write_failed"))
        .mockResolvedValueOnce({ ...saved, revision: 1 });
    const { onSaved, onRefresh } = await mount();
    await submit();
    expect(screen.getByRole("alert").textContent).toBe(
        "The provider settings were not saved. The previous settings are unchanged. Try again.",
    );
    expect(onRefresh).not.toHaveBeenCalled();
    await submit();
    expect(create).toHaveBeenCalledTimes(2);
    expect(onSaved).toHaveBeenCalledTimes(1);
});

test("keeps focus on Save, announces saving and ignores a second submit while pending", async () => {
    const request = deferred<ProviderRecord>();
    create.mockReturnValue(request.promise);
    const { onBusyChange } = await mount();
    chooseProvider("openrouter");
    const save = screen.getByRole("button", { name: "Save" });
    save.focus();
    await submit();
    // aria-disabled, not disabled: a disabled button would drop focus to body.
    expect(save.getAttribute("aria-disabled")).toBe("true");
    expect(save.hasAttribute("disabled")).toBe(false);
    expect(document.activeElement).toBe(save);
    expect(screen.getByRole("status").textContent).toBe("Saving…");
    // Blocked fields stay focusable but cannot change.
    expect(input("Name")).toHaveProperty("readOnly", true);
    expect(input("Protocol").getAttribute("aria-disabled")).toBe("true");
    setField("Protocol", "responses");
    expect(fieldValue("Protocol")).toBe("chat_completions");
    expect(providerCombobox().getAttribute("aria-disabled")).toBe("true");
    chooseProvider("deepseek");
    expect(providerKindValue()).toBe("openrouter");
    expect(fieldValue("Name")).toBe("OpenRouter");
    // The page blocks its back control while this is reported.
    expect(onBusyChange).toHaveBeenLastCalledWith(true);
    fireEvent.click(save);
    fireEvent.submit(screen.getByRole("form"));
    expect(create).toHaveBeenCalledTimes(1);
    await act(async () => {
        request.resolve({ ...saved, revision: 1 });
        await request.promise;
    });
});

test("edits by id with the expected revision and never sends or changes the type", async () => {
    const next: ProviderRecord = {
        ...saved,
        displayName: "Work 2",
        revision: 4,
    };
    update.mockResolvedValueOnce(next);
    const { onSaved } = await mount({ kind: "edit", record: saved });

    expect(screen.queryByRole("combobox", { name: "Provider" })).toBeNull();
    expect(screen.getByText("Provider").nextElementSibling?.textContent).toBe(
        "DeepSeek",
    );
    expect(screen.queryByText(ID)).toBeNull();
    expect(fieldValue("Name")).toBe("Work");

    type("Name", "Work 2 ");
    await submit();
    expect(update).toHaveBeenLastCalledWith({
        id: ID,
        expectedRevision: 3,
        displayName: "Work 2 ",
        baseUrl: "https://api.deepseek.com",
        protocol: "chat_completions",
        extensions: {},
    });
    expect(update.mock.calls[0]?.[0]).not.toHaveProperty("kind");
    expect(onSaved).toHaveBeenCalledExactlyOnceWith(next);
});

test("a revision conflict blocks saving until the latest record is reloaded", async () => {
    update.mockRejectedValueOnce(new ProviderRequestError("revision_conflict"));
    const latest: ProviderRecord = {
        ...saved,
        displayName: "Changed elsewhere",
        revision: 7,
    };
    read.mockResolvedValue(latest);
    const { onRecordLoaded, onSaved } = await mount({
        kind: "edit",
        record: saved,
    });
    type("Name", "Mine");
    await submit();

    expect(screen.getByRole("alert").textContent).toBe(
        "This configuration was changed elsewhere. Reload it before editing.",
    );
    expect(
        screen
            .getByRole("button", { name: "Save" })
            .getAttribute("aria-disabled"),
    ).toBe("true");
    expect(onSaved).not.toHaveBeenCalled();

    await submit("Reload latest settings");
    expect(read).toHaveBeenCalledWith(ID);
    // The reload button is gone; focus stays inside the form on Save.
    expect(document.activeElement).toBe(
        screen.getByRole("button", { name: "Save" }),
    );
    expect(onRecordLoaded).toHaveBeenCalledWith(latest);
    expect(fieldValue("Name")).toBe("Changed elsewhere");
    expect(screen.getByRole("status").textContent).toBe(
        "The latest saved settings were loaded. Review them before saving.",
    );

    update.mockResolvedValueOnce({ ...latest, revision: 8 });
    await submit();
    expect(update.mock.calls[1]?.[0]).toMatchObject({ expectedRevision: 7 });
});

test("a failed reload keeps saving blocked and offers another reload", async () => {
    update.mockRejectedValueOnce(new ProviderRequestError("revision_conflict"));
    read.mockRejectedValueOnce(new ProviderRequestError("not_found"));
    await mount({ kind: "edit", record: saved });
    await submit();
    await submit("Reload latest settings");
    expect(screen.getByRole("alert").textContent).toBe(
        "This provider configuration was not found. Refresh the list.",
    );
    expect(
        screen
            .getByRole("button", { name: "Save" })
            .getAttribute("aria-disabled"),
    ).toBe("true");
    expect(
        screen.getByRole("button", { name: "Reload latest settings" }),
    ).toBeDefined();
});

test.each(["operation_failed", "invalid_response"] as const)(
    "after %s the list is refreshed before a retry can be confirmed",
    async (code) => {
        const refresh = deferred<boolean>();
        create
            .mockRejectedValueOnce(new ProviderRequestError(code))
            .mockResolvedValueOnce({ ...saved, revision: 1 });
        const { onRefresh, onSaved } = await mount(
            { kind: "create" },
            "en",
            () => refresh.promise,
        );
        await submit();

        expect(onRefresh).toHaveBeenCalledTimes(1);
        expect(screen.getByRole("status").textContent).toBe(
            "Refreshing the list to check the result of this save…",
        );
        expect(screen.queryByRole("button", { name: "Save again" })).toBeNull();
        expect(
            screen
                .getByRole("button", { name: "Save" })
                .getAttribute("aria-disabled"),
        ).toBe("true");

        await act(async () => {
            refresh.resolve(true);
            await refresh.promise;
        });
        expect(screen.getByRole("alert").textContent).toMatch(
            /result is unknown/,
        );
        expect(
            screen.getByText(/The list was refreshed\. Check whether/),
        ).toBeDefined();
        // A plain Save stays blocked; only the explicit retry saves again.
        fireEvent.submit(screen.getByRole("form"));
        expect(create).toHaveBeenCalledTimes(1);

        await submit("Save again");
        expect(document.activeElement).toBe(
            screen.getByRole("button", { name: "Save" }),
        );
        expect(create).toHaveBeenCalledTimes(2);
        expect(onSaved).toHaveBeenCalledTimes(1);
    },
);

test("when the refresh after an unknown outcome fails, retry stays unavailable", async () => {
    create.mockRejectedValue(new ProviderRequestError("operation_failed"));
    const results = [false, true];
    const { onRefresh } = await mount({ kind: "create" }, "zh-CN", () =>
        Promise.resolve(results.shift() ?? true),
    );
    await submit("保存");
    expect(
        screen.getByText("列表未能刷新，暂时不能再次保存。请先刷新列表确认。"),
    ).toBeDefined();
    expect(screen.queryByRole("button", { name: "再次保存" })).toBeNull();

    await submit("刷新列表");
    expect(onRefresh).toHaveBeenCalledTimes(2);
    expect(screen.getByRole("button", { name: "再次保存" })).toBeDefined();
    expect(create).toHaveBeenCalledTimes(1);
});

test("browser preview cannot save and never reports success", async () => {
    create.mockRejectedValue(new ProviderRequestError("desktop_required"));
    const { onSaved, onRefresh } = await mount();
    await submit();
    expect(keyField().value).toBe("");
    expect(screen.getByRole("alert").textContent).toBe(
        "Settings cannot be saved in the browser preview. Use the vibemate desktop app.",
    );
    expect(onSaved).not.toHaveBeenCalled();
    expect(onRefresh).not.toHaveBeenCalled();
});

test("a result that arrives after unmount is ignored", async () => {
    const request = deferred<ProviderRecord>();
    create.mockReturnValue(request.promise);
    const { onSaved } = await mount();
    await submit();
    cleanup();
    await act(async () => {
        request.resolve({ ...saved, revision: 1 });
        await request.promise;
    });
    expect(onSaved).not.toHaveBeenCalled();
});

test("cancel calls back and switching language keeps typed values", async () => {
    const { instance, onCancel } = await mount();
    type("Name", "Personal");
    await act(async () => {
        await instance.changeLanguage("zh-CN");
    });
    expect(fieldValue("名称")).toBe("Personal");
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    expect(onCancel).toHaveBeenCalledTimes(1);
});

test("cancel is ignored while a save is pending", async () => {
    const request = deferred<ProviderRecord>();
    create.mockReturnValue(request.promise);
    const { onCancel } = await mount();
    await submit();
    const cancel = screen.getByRole("button", { name: "Cancel" });
    expect(cancel.getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(cancel);
    expect(onCancel).not.toHaveBeenCalled();
    await act(async () => {
        request.resolve({ ...saved, revision: 1 });
        await request.promise;
    });
});

test("reports pending work so the page can block going back", async () => {
    const request = deferred<ProviderRecord>();
    create.mockReturnValue(request.promise);
    const { onBusyChange } = await mount();
    expect(onBusyChange).toHaveBeenLastCalledWith(false);
    await submit();
    expect(onBusyChange).toHaveBeenLastCalledWith(true);
    await act(async () => {
        request.reject(new ProviderRequestError("write_failed"));
        await request.promise.catch(() => undefined);
    });
    expect(onBusyChange).toHaveBeenLastCalledWith(false);
});

test("an edit has a fixed configured indicator, never a fetched or submitted key", async () => {
    update.mockResolvedValue({ ...saved, revision: saved.revision + 1 });
    const { onSaved } = await mount({ kind: "edit", record: saved });
    const key = keyField();
    expect(key.type).toBe("password");
    expect(key.value).toBe("");
    expect(key.placeholder).toBe("••••••••");
    expect(key.required).toBe(false);
    const indicator = screen.getByTitle("API key configured");
    expect(indicator.querySelector("svg")).not.toBeNull();
    expect(
        document.getElementById(key.getAttribute("aria-describedby") ?? "")
            ?.textContent,
    ).toBe("API key configured");
    await submit("Save", null);
    expect(update).toHaveBeenCalledTimes(1);
    expect(update.mock.calls[0]?.[0]).not.toHaveProperty("secret");
    expect(JSON.stringify(update.mock.calls)).not.toContain("••••••••");
    expect(onSaved).toHaveBeenCalledTimes(1);
});

test.each([
    ["en", "", "Enter the API key.", "Save"],
    ["zh-CN", "   ", "请输入 API 密钥。", "保存"],
] as const)(
    "in %s an empty key is caught before any request and shown next to the field",
    async (locale, key, message, save) => {
        await mount({ kind: "create" }, locale);
        await submit(save, key);
        expect(create).not.toHaveBeenCalled();
        const alert = screen.getByRole("alert");
        expect(alert.textContent).toBe(message);
        const field = keyField();
        expect(field.getAttribute("aria-invalid")).toBe("true");
        expect(field.getAttribute("aria-describedby")).toContain(alert.id);
        // Focus moves to the field to fix, and whitespace is cleared too.
        expect(document.activeElement).toBe(field);
        expect(field.value).toBe("");
        fireEvent.change(field, { target: { value: "s" } });
        expect(screen.queryByRole("alert")).toBeNull();
        expect(field.getAttribute("aria-invalid")).toBe("false");
    },
);

test("secret_invalid from Rust is shown next to the key field, which is cleared", async () => {
    create.mockRejectedValue(new ProviderRequestError("secret_invalid"));
    await mount();
    await submit("Save", "  sk\u0007bell  ");
    // The key goes to Rust untrimmed and unchecked beyond being non-empty.
    expect(create.mock.calls[0]?.[0].secret).toBe("  sk\u0007bell  ");
    const alert = screen.getByRole("alert");
    expect(alert.textContent).toBe(
        "Enter the API key. It must not contain line breaks or other control characters, and must fit the system credential store's size limit.",
    );
    const field = keyField();
    expect(field.getAttribute("aria-invalid")).toBe("true");
    expect(field.getAttribute("aria-describedby")).toContain(alert.id);
    expect(field.value).toBe("");
    expect(input("Base URL").getAttribute("aria-invalid")).toBe("false");
});

test.each([
    [
        "credential_store_unavailable",
        "No system credential store is available on this device (for example, no Secret Service on Linux). Nothing was saved, and vibemate never stores keys as plain text.",
    ],
    [
        "credential_store_access_denied",
        "vibemate could not access the system credential store. It may be locked or access was refused, and nothing was changed. Unlock it or allow access, then try again.",
    ],
    [
        "credential_store_failed",
        "The system credential store reported an error and nothing was changed. If you denied access in a system prompt, try again and allow it.",
    ],
] as const)(
    "%s is shown at the top of the form and the key is cleared",
    async (code, message) => {
        create.mockRejectedValue(new ProviderRequestError(code));
        const { onRefresh, onSaved } = await mount();
        await submit();
        const alert = screen.getByRole("alert");
        expect(alert.textContent).toBe(message);
        // Before the first field, not by the key field or the buttons.
        expect(
            alert.compareDocumentPosition(providerCombobox()) &
                Node.DOCUMENT_POSITION_FOLLOWING,
        ).toBeTruthy();
        expect(keyField().getAttribute("aria-invalid")).toBe("false");
        expectKeyCleared();
        expect(onRefresh).not.toHaveBeenCalled();
        expect(onSaved).not.toHaveBeenCalled();
        // A definite failure: Save is available again with a newly typed key.
        create.mockResolvedValueOnce({ ...saved, revision: 1 });
        await submit("Save", "second-synthetic");
        expect(create).toHaveBeenLastCalledWith(
            expect.objectContaining({ secret: "second-synthetic" }),
        );
        expect(onSaved).toHaveBeenCalledTimes(1);
    },
);

test("create_outcome_unknown re-reads the list and a retry needs the key typed again", async () => {
    create
        .mockRejectedValueOnce(
            new ProviderRequestError("create_outcome_unknown"),
        )
        .mockResolvedValueOnce({ ...saved, revision: 1 });
    const { onRefresh, onSaved } = await mount({ kind: "create" }, "zh-CN");
    await submit("保存");
    expect(onRefresh).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("alert").textContent).toBe(
        "无法确认这项配置是否已保存，系统凭据库中可能留有一个未被使用的密钥条目（vibemate 不会读取它）。请先刷新列表确认，再决定是否重新添加。",
    );
    expect(
        screen.getByText(
            "列表已刷新。请先确认这项配置是否已出现在列表中；只有在没有出现时才再次保存。API 密钥栏已被清空，再次保存前请重新输入密钥。",
        ),
    ).toBeDefined();
    const field = keyField();
    expect(field.value).toBe("");
    // The key field stays editable so it can be typed again; others do not.
    expect(field.readOnly).toBe(false);
    expect(input("名称")).toHaveProperty("readOnly", true);

    // "Save again" without a key asks for it and keeps the unknown state.
    await submit("再次保存", null);
    expect(create).toHaveBeenCalledTimes(1);
    expect(document.activeElement).toBe(field);
    expect(screen.getByText("请输入 API 密钥。")).toBeDefined();
    expect(screen.getByRole("button", { name: "再次保存" })).toBeDefined();
    fireEvent.submit(screen.getByRole("form"));
    expect(create).toHaveBeenCalledTimes(1);

    await submit("再次保存", "retyped-synthetic");
    expect(create).toHaveBeenCalledTimes(2);
    expect(create).toHaveBeenLastCalledWith(
        expect.objectContaining({ secret: "retyped-synthetic" }),
    );
    expect(onSaved).toHaveBeenCalledTimes(1);
});

test("the key is cleared after every submit, on cancel and while hidden; other fields stay", async () => {
    const request = deferred<ProviderRecord>();
    create.mockReturnValueOnce(request.promise);
    const { onCancel, setHidden } = await mount();
    await submit();
    // Cleared as soon as it is sent, while the request is still pending.
    expectKeyCleared();
    expect(keyField().readOnly).toBe(true);
    await act(async () => {
        request.reject(new ProviderRequestError("write_failed"));
        await request.promise.catch(() => undefined);
    });
    expect(keyField().value).toBe("");

    type("Name", "Draft");
    fireEvent.change(keyField(), { target: { value: SECRET } });
    // React mirrors a typed value into the attribute, so clearing must reach it.
    expect(keyField().getAttribute("value")).toBe(SECRET);
    setHidden(true);
    expectKeyCleared();
    setHidden(false);
    expect(keyField().value).toBe("");
    expect(fieldValue("Name")).toBe("Draft");

    fireEvent.change(keyField(), { target: { value: SECRET } });
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(keyField().value).toBe("");
});

test("the key never appears in page text, callbacks or the console", async () => {
    const spies = (["log", "info", "warn", "error", "debug"] as const).map(
        (method) => vi.spyOn(console, method),
    );
    create
        .mockRejectedValueOnce(new ProviderRequestError("secret_invalid"))
        .mockRejectedValueOnce(new ProviderRequestError("operation_failed"))
        .mockResolvedValueOnce({ ...saved, revision: 1 });
    const { onSaved, onBusyChange, onRefresh } = await mount();
    await submit();
    await submit();
    await submit("Save again");
    expect(onSaved).toHaveBeenCalledTimes(1);
    expect(document.body.textContent).not.toContain(SECRET);
    for (const mock of [onSaved, onBusyChange, onRefresh])
        expect(JSON.stringify(mock.mock.calls)).not.toContain(SECRET);
    for (const spy of spies) {
        expect(JSON.stringify(spy.mock.calls)).not.toContain(SECRET);
        spy.mockRestore();
    }
});

test("a plain Enter after an unknown outcome is ignored but still clears the key", async () => {
    create.mockRejectedValueOnce(new ProviderRequestError("operation_failed"));
    await mount();
    await submit();
    expect(screen.getByRole("button", { name: "Save again" })).toBeDefined();
    // The key field stays editable here so the key can be typed again.
    fireEvent.change(keyField(), { target: { value: SECRET } });
    expect(keyField().value).toBe(SECRET);
    // Enter in a field submits the form; saving stays blocked until "Save
    // again", yet the typed key does not survive the attempt.
    await act(async () => {
        fireEvent.submit(screen.getByRole("form"));
        await Promise.resolve();
    });
    expect(create).toHaveBeenCalledTimes(1);
    expectKeyCleared();
    expect(screen.getByRole("button", { name: "Save again" })).toBeDefined();
    expect(screen.queryByText("Enter the API key.")).toBeNull();
});

test("a second submit while saving is ignored and leaves no key behind", async () => {
    const request = deferred<ProviderRecord>();
    create.mockReturnValueOnce(request.promise);
    await mount();
    await submit();
    // The field is read-only while saving; even a forced value is dropped.
    fireEvent.change(keyField(), { target: { value: SECRET } });
    fireEvent.submit(screen.getByRole("form"));
    expect(create).toHaveBeenCalledTimes(1);
    expectKeyCleared();
    await act(async () => {
        request.resolve({ ...saved, revision: 1 });
        await request.promise;
    });
});
