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
    getProviderSecretStatus,
    type ProviderSecretStatusResult,
} from "../../lib/desktop/providerSecrets";
import {
    ProviderRequestError,
    updateProvider,
    type ProviderRecord,
    type ProviderTemplate,
} from "../../lib/desktop/providers";
import { ProviderForm } from "./ProviderForm";

vi.mock(import("../../lib/desktop/providers"), async (importOriginal) => ({
    ...(await importOriginal()),
    updateProvider: vi.fn(),
}));
vi.mock(
    import("../../lib/desktop/providerSecrets"),
    async (importOriginal) => ({
        ...(await importOriginal()),
        getProviderSecretStatus: vi.fn(),
    }),
);
const update = vi.mocked(updateProvider);
const readKey = vi.mocked(getProviderSecretStatus);
const SECRET = "synthetic-edit-replacement-not-a-real-key";
const record: ProviderRecord = {
    id: "0123456789abcdef0123456789abcdef",
    kind: "deepseek",
    displayName: "Work",
    baseUrl: "https://api.deepseek.com",
    protocol: "chat_completions",
    extensions: {},
    revision: 1,
    createdAtMs: 1000,
    updatedAtMs: 1000,
    selectedModelCount: 0,
};
const templates: ProviderTemplate[] = [
    {
        kind: "deepseek",
        brandName: "DeepSeek",
        defaultBaseUrl: record.baseUrl,
        protocols: ["chat_completions"],
        defaultProtocol: "chat_completions",
        extensionFields: [],
    },
];
const keyStatus: ProviderSecretStatusResult = {
    kind: "desktop",
    status: { providerId: record.id, state: "set", updatedAtMs: 1000 },
};

beforeEach(() => {
    update.mockReset().mockResolvedValue({ ...record, revision: 2 });
    readKey.mockReset().mockResolvedValue(keyStatus);
});
afterEach(cleanup);

function deferred<T>() {
    let resolve: (value: T) => void = () => {
        throw new Error("Not initialized");
    };
    let reject: (error: Error) => void = () => {
        throw new Error("Not initialized");
    };
    const promise = new Promise<T>((res, rej) => {
        resolve = res;
        reject = rej;
    });
    return { promise, resolve, reject };
}

async function mount(locale: "en" | "zh-CN" = "en") {
    const instance = await createAppI18n(locale);
    const callbacks = {
        onSaved: vi.fn(),
        onRecordLoaded: vi.fn(),
        onRefresh: vi.fn().mockResolvedValue(true),
        onBusyChange: vi.fn(),
    };
    const view = render(
        <StrictMode>
            <I18nextProvider i18n={instance}>
                <ProviderForm
                    templates={templates}
                    mode={{ kind: "edit", record }}
                    {...callbacks}
                />
            </I18nextProvider>
        </StrictMode>,
    );
    await act(async () => {
        await Promise.resolve();
    });
    return { ...view, ...callbacks };
}

function key(): HTMLInputElement {
    const element = screen.getByLabelText(/^(API key|API 密钥)$/);
    if (!(element instanceof HTMLInputElement))
        throw new Error("Expected a key input");
    return element;
}

async function save(name = "Save") {
    await act(async () => {
        fireEvent.click(screen.getByRole("button", { name }));
        await Promise.resolve();
    });
}

for (const locale of ["en", "zh-CN"] as const) {
    test(`editing in ${locale} replaces the key in the same request and clears it before acknowledgment`, async () => {
        const pending = deferred<ProviderRecord>();
        update.mockReturnValue(pending.promise);
        const { onSaved, onBusyChange } = await mount(locale);
        const field = key();
        expect(field.placeholder).toBe("••••••••");
        fireEvent.change(field, { target: { value: SECRET } });
        expect(field.placeholder).toBe("");
        expect(
            screen.queryByTitle(
                locale === "en" ? "API key configured" : "已配置 API 密钥",
            ),
        ).toBeNull();
        const button = screen.getByRole("button", {
            name: locale === "en" ? "Save" : "保存",
        });
        button.focus();
        await save(locale === "en" ? "Save" : "保存");
        expect(update).toHaveBeenCalledExactlyOnceWith({
            id: record.id,
            expectedRevision: 1,
            displayName: record.displayName,
            baseUrl: record.baseUrl,
            protocol: record.protocol,
            extensions: {},
            secret: SECRET,
        });
        expect(field.value).toBe("");
        expect(field.getAttribute("value")).toBe("");
        expect(field.readOnly).toBe(true);
        expect(document.activeElement).toBe(button);
        expect(onSaved).not.toHaveBeenCalled();
        await save(locale === "en" ? "Save" : "保存");
        expect(update).toHaveBeenCalledTimes(1);
        await act(async () => {
            pending.resolve({ ...record, revision: 2 });
            await pending.promise;
        });
        expect(onSaved).toHaveBeenCalledTimes(1);
        expect(onBusyChange).toHaveBeenLastCalledWith(false);
        expect(document.body.textContent).not.toContain(SECRET);
        expect(JSON.stringify(onSaved.mock.calls)).not.toContain(SECRET);
    });
}

test("an edited legacy instance requires a key but never submits a placeholder", async () => {
    readKey.mockResolvedValue({
        kind: "desktop",
        status: { providerId: record.id, state: "missing", updatedAtMs: null },
    });
    await mount();
    expect(key().placeholder).toBe("");
    await save();
    expect(update).not.toHaveBeenCalled();
    expect(screen.getByRole("alert").textContent).toBe("Enter the API key.");
    expect(document.activeElement).toBe(key());
    fireEvent.change(key(), { target: { value: SECRET } });
    await save();
    expect(update.mock.calls[0]?.[0].secret).toBe(SECRET);
});

test("failed key-status read blocks saving, exposes no diagnostics and retries with stable focus", async () => {
    readKey.mockRejectedValue(new Error("private key-status diagnostic"));
    await mount();
    await save();
    expect(update).not.toHaveBeenCalled();
    expect(screen.queryByTitle("API key configured")).toBeNull();
    expect(document.body.textContent).not.toContain("private key-status");
    readKey.mockResolvedValue(keyStatus);
    const retry = screen.getByRole("button", { name: "Retry key status" });
    retry.focus();
    await act(async () => {
        fireEvent.click(retry);
        await Promise.resolve();
    });
    expect(key().placeholder).toBe("••••••••");
    expect(document.activeElement).toBe(key());
    await save();
    expect(update.mock.calls[0]?.[0]).not.toHaveProperty("secret");
});

test("an uncertain replacement refreshes first and requires a retyped key for explicit retry", async () => {
    update.mockRejectedValueOnce(new ProviderRequestError("operation_failed"));
    const { onRefresh, onSaved } = await mount();
    fireEvent.change(key(), { target: { value: SECRET } });
    await save();
    expect(onRefresh).toHaveBeenCalledTimes(1);
    expect(onSaved).not.toHaveBeenCalled();
    expect(key().value).toBe("");
    expect(screen.getByText(/enter the replacement key again/)).toBeDefined();
    await save("Save again");
    expect(update).toHaveBeenCalledTimes(1);
    expect(screen.getByText("Enter the API key.")).toBeDefined();
    fireEvent.change(key(), { target: { value: SECRET } });
    await save("Save again");
    expect(update).toHaveBeenCalledTimes(2);
    expect(onSaved).toHaveBeenCalledTimes(1);
});

test("a failed combined save keeps the page and draft settings but clears the replacement key", async () => {
    update.mockRejectedValue(new ProviderRequestError("write_failed"));
    const { onSaved } = await mount();
    fireEvent.change(screen.getByLabelText("Name"), {
        target: { value: "Draft" },
    });
    fireEvent.change(key(), { target: { value: SECRET } });
    await save();
    expect(onSaved).not.toHaveBeenCalled();
    expect(screen.getByLabelText("Name")).toHaveProperty("value", "Draft");
    expect(key().value).toBe("");
    expect(screen.getByRole("alert").textContent).toContain(
        "previous settings are unchanged",
    );
});

for (const outcome of ["success", "failure"] as const) {
    test(`a stale StrictMode key-status ${outcome} cannot replace current configured state`, async () => {
        const obsolete = deferred<ProviderSecretStatusResult>();
        const current = deferred<ProviderSecretStatusResult>();
        readKey
            .mockReturnValueOnce(obsolete.promise)
            .mockReturnValueOnce(current.promise);
        await mount();
        expect(key().placeholder).toBe("");
        await save();
        expect(update).not.toHaveBeenCalled();
        await act(async () => {
            current.resolve(keyStatus);
            await current.promise;
        });
        await act(async () => {
            if (outcome === "success")
                obsolete.resolve({
                    kind: "desktop",
                    status: {
                        providerId: record.id,
                        state: "missing",
                        updatedAtMs: null,
                    },
                });
            else obsolete.reject(new Error("obsolete"));
            await obsolete.promise.catch(() => null);
        });
        expect(key().placeholder).toBe("••••••••");
        expect(screen.queryByRole("alert")).toBeNull();
    });
}

test("browser preview cannot claim a configured key or save an edit", async () => {
    readKey.mockResolvedValue({ kind: "preview" });
    await mount();
    expect(key().placeholder).toBe("");
    await save();
    expect(update).not.toHaveBeenCalled();
    expect(
        screen.getByText(
            "Settings cannot be saved in the browser preview. Use the vibemate desktop app.",
        ),
    ).toBeDefined();
});
