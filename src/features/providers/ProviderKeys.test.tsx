import {
    act,
    cleanup,
    fireEvent,
    render,
    screen,
    within,
} from "@testing-library/react";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { NotificationProvider } from "../../components/Notifications";
import { createAppI18n } from "../../i18n";
import { ProviderRequestError } from "../../lib/desktop/providers";
import {
    getProviderSecretStatus,
    replaceProviderSecret,
    type ProviderSecretStatus,
    type ProviderSecretStatusResult,
} from "../../lib/desktop/providerSecrets";
import { ProviderKeys } from "./ProviderKeys";

// Only the IPC calls are replaced; types come from the real module.
vi.mock(
    import("../../lib/desktop/providerSecrets"),
    async (importOriginal) => ({
        ...(await importOriginal()),
        getProviderSecretStatus: vi.fn(),
        replaceProviderSecret: vi.fn(),
    }),
);
const readStatus = vi.mocked(getProviderSecretStatus);
const replace = vi.mocked(replaceProviderSecret);

const ID = "0123456789abcdef0123456789abcdef";
// Synthetic; never a real key.
const SECRET = "sk-synthetic-keys-3333";
const UPDATED = Date.UTC(2026, 9, 10, 11, 40);
const NEWER = Date.UTC(2026, 9, 11, 8, 5);

const set: ProviderSecretStatus = {
    providerId: ID,
    state: "set",
    updatedAtMs: UPDATED,
};
const missing: ProviderSecretStatus = {
    providerId: ID,
    state: "missing",
    updatedAtMs: null,
};

function desktop(status: ProviderSecretStatus): ProviderSecretStatusResult {
    return { kind: "desktop", status };
}

function formatted(locale: string, ms: number): string {
    return new Intl.DateTimeFormat(locale, {
        dateStyle: "medium",
        timeStyle: "short",
    }).format(ms);
}

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

afterEach(cleanup);
beforeEach(() => {
    readStatus.mockReset().mockResolvedValue(desktop(set));
    replace.mockReset();
});

async function mount(locale: "en" | "zh-CN" = "en") {
    const instance = await createAppI18n(locale);
    const onBusyChange = vi.fn<(busy: boolean) => void>();
    const tree = (hidden: boolean) => (
        <StrictMode>
            <I18nextProvider i18n={instance}>
                <NotificationProvider>
                    <ProviderKeys
                        providerId={ID}
                        onBusyChange={onBusyChange}
                        hidden={hidden}
                    />
                </NotificationProvider>
            </I18nextProvider>
        </StrictMode>
    );
    let rerender: (ui: React.ReactNode) => void = () => undefined;
    await act(async () => {
        ({ rerender } = render(tree(false)));
        await Promise.resolve();
    });
    return {
        instance,
        onBusyChange,
        setHidden: (hidden: boolean) => {
            rerender(tree(hidden));
        },
    };
}

/** The key group itself, excluding the app-level notification region. */
function group(): HTMLElement {
    const section = document.querySelector("section");
    if (!section) throw new Error("ProviderKeys is not rendered");
    return section;
}

function stateValue(): HTMLElement {
    const [status] = within(group()).getAllByRole("status");
    if (!status) throw new Error("No status value");
    return status;
}

function keyField(
    label: string | RegExp = /API key|API 密钥/,
): HTMLInputElement {
    const element = screen.getByLabelText(label);
    if (!(element instanceof HTMLInputElement))
        throw new Error("The key field is not an input");
    return element;
}

/** Empty as a property and as the mirrored `value` attribute. */
function expectKeyCleared(label?: string): void {
    const field = label === undefined ? keyField() : keyField(label);
    expect(field.value).toBe("");
    expect(field.getAttribute("value") ?? "").toBe("");
}

function notification(): string | null | undefined {
    return screen
        .getAllByRole("status")
        .find((element) => !group().contains(element))?.textContent;
}

async function click(name: string | RegExp, key: string | null = null) {
    if (key !== null) fireEvent.change(keyField(), { target: { value: key } });
    await act(async () => {
        fireEvent.click(screen.getByRole("button", { name }));
        await Promise.resolve();
    });
}

test.each([
    ["en", "Keys", "Current key", "Set · updated", "Replace key"],
    ["zh-CN", "密钥", "当前密钥", "已设置 · 更新于", "替换密钥"],
] as const)(
    "in %s shows loading, then the set state with its update time and no clear action",
    async (locale, title, current, setText, replaceText) => {
        const pending = deferred<ProviderSecretStatusResult>();
        readStatus.mockReturnValue(pending.promise);
        await mount(locale);
        expect(screen.getByRole("heading", { level: 2, name: title }));
        expect(
            screen.getByRole("form", { name: title }).getAttribute("aria-busy"),
        ).toBe("false");
        expect(screen.getByText(current).nextElementSibling).toBe(stateValue());
        expect(stateValue().textContent).toMatch(/…$/);
        // Loading blocks replacing; the field stays focusable but read-only.
        expect(keyField().readOnly).toBe(true);
        await act(async () => {
            pending.resolve(desktop(set));
            await pending.promise;
        });
        expect(readStatus).toHaveBeenCalledWith(ID);
        expect(stateValue().textContent).toBe(
            `${setText} ${formatted(locale, UPDATED)}`,
        );
        const button = screen.getByRole("button", { name: replaceText });
        expect(button.getAttribute("aria-disabled")).toBe("false");
        // Secondary: the page's one primary action is the basic information Save.
        expect(button.className).toMatch(/secondary/);
        expect(keyField().type).toBe("password");
        expect(keyField().value).toBe("");
        expect(keyField().getAttribute("autocomplete")).toBe("new-password");
        expect(keyField().readOnly).toBe(false);
        // The only action is replacing; there is no clear or delete button.
        expect(screen.getAllByRole("button")).toHaveLength(1);
        expect(document.body.textContent).not.toMatch(
            /delete|remove|清除|删除|移除|待配置/i,
        );
    },
);

test("replacing sends only the ID and key, updates the status, notifies and stays on the page", async () => {
    replace.mockResolvedValue({ ...set, updatedAtMs: NEWER });
    const { onBusyChange } = await mount();
    const button = screen.getByRole("button", { name: "Replace key" });
    button.focus();
    await click("Replace key", SECRET);
    expect(replace).toHaveBeenCalledExactlyOnceWith({
        providerId: ID,
        secret: SECRET,
    });
    expect(stateValue().textContent).toBe(
        `Set · updated ${formatted("en", NEWER)}`,
    );
    expect(notification()).toBe("API key replaced.");
    expect(group().textContent).not.toContain("API key replaced.");
    expect(document.activeElement).toBe(button);
    expectKeyCleared();
    expect(onBusyChange).toHaveBeenLastCalledWith(false);
    expect(document.body.textContent).not.toContain(SECRET);
});

test("a missing key from older data offers Set key with the same control", async () => {
    readStatus.mockResolvedValue(desktop(missing));
    replace.mockResolvedValue(set);
    await mount("zh-CN");
    expect(stateValue().textContent).toBe("未设置密钥");
    const field = keyField("API 密钥");
    expect(field.value).toBe("");
    await click("设置密钥", SECRET);
    expect(replace).toHaveBeenCalledExactlyOnceWith({
        providerId: ID,
        secret: SECRET,
    });
    expect(notification()).toBe("已设置 API 密钥。");
    expect(stateValue().textContent).toBe(
        `已设置 · 更新于 ${formatted("zh-CN", UPDATED)}`,
    );
    expect(screen.getByRole("button", { name: "替换密钥" })).toBeDefined();
    expect(keyField("新的 API 密钥").value).toBe("");
});

test("while replacing, controls stay focusable but blocked and busy is reported", async () => {
    const request = deferred<ProviderSecretStatus>();
    replace.mockReturnValue(request.promise);
    const { onBusyChange } = await mount();
    await click("Replace key", SECRET);
    const button = screen.getByRole("button", { name: "Replace key" });
    expect(button.getAttribute("aria-disabled")).toBe("true");
    expect(button.hasAttribute("disabled")).toBe(false);
    expect(keyField().readOnly).toBe(true);
    expect(keyField().value).toBe("");
    expect(
        within(group()).getByText("Replacing the key…").getAttribute("role"),
    ).toBe("status");
    expect(onBusyChange).toHaveBeenLastCalledWith(true);
    fireEvent.click(button);
    expect(replace).toHaveBeenCalledTimes(1);
    await act(async () => {
        request.resolve(set);
        await request.promise;
    });
    expect(onBusyChange).toHaveBeenLastCalledWith(false);
});

test("an empty key is caught before any request and shown next to the field", async () => {
    await mount();
    await click("Replace key", "   ");
    expect(replace).not.toHaveBeenCalled();
    const alert = screen.getByRole("alert");
    expect(alert.textContent).toBe("Enter the API key.");
    const field = keyField();
    expect(field.getAttribute("aria-invalid")).toBe("true");
    expect(field.getAttribute("aria-describedby")).toContain(alert.id);
    expect(document.activeElement).toBe(field);
    expect(field.value).toBe("");
    fireEvent.change(field, { target: { value: "x" } });
    expect(screen.queryByRole("alert")).toBeNull();
});

test("secret_invalid is shown next to the field and the field is cleared", async () => {
    replace.mockRejectedValue(new ProviderRequestError("secret_invalid"));
    await mount();
    await click("Replace key", SECRET);
    const alert = screen.getByRole("alert");
    expect(alert.textContent).toBe(
        "Enter the API key. It must not contain line breaks or other control characters, and must fit the system credential store's size limit.",
    );
    expect(keyField().getAttribute("aria-describedby")).toContain(alert.id);
    expect(keyField().getAttribute("aria-invalid")).toBe("true");
    expectKeyCleared();
    // The status is unchanged: nothing was replaced.
    expect(stateValue().textContent).toMatch(/^Set · updated/);
});

test.each([
    "credential_store_unavailable",
    "credential_store_access_denied",
    "credential_store_failed",
] as const)("%s is shown at the top of the key group", async (code) => {
    replace.mockRejectedValue(new ProviderRequestError(code));
    await mount();
    await click("Replace key", SECRET);
    const alert = screen.getByRole("alert");
    expect(alert.textContent).toMatch(/system credential store/);
    expect(
        alert.compareDocumentPosition(stateValue()) &
            Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(keyField().getAttribute("aria-invalid")).toBe("false");
    expect(keyField().value).toBe("");
    expect(
        screen
            .getByRole("button", { name: "Replace key" })
            .getAttribute("aria-disabled"),
    ).toBe("false");
});

test.each([
    [
        "en",
        "read_failed",
        "The key was not changed because the saved configuration could not be read. Try again.",
    ],
    ["zh-CN", "read_failed", "无法读取已保存的配置，密钥没有更改。请重试。"],
    [
        "en",
        "write_failed",
        "The key was not changed, and the previous key status is unchanged. Try again.",
    ],
    ["zh-CN", "write_failed", "密钥没有更改，之前的密钥状态保持不变。请重试。"],
    [
        "en",
        "not_found",
        "This provider configuration was not found, so its key was not changed. Go back and refresh the list.",
    ],
    [
        "en",
        "storage_unavailable",
        "Local configuration storage is unavailable, so the key was not changed. Restart vibemate.",
    ],
    [
        "zh-CN",
        "invalid_request",
        "请求无效，密钥没有更改。请返回并刷新列表后重试。",
    ],
    [
        "en",
        "desktop_required",
        "API keys cannot be changed in the browser preview, so nothing was changed. Use the vibemate desktop app.",
    ],
] as const)(
    "in %s a replace %s says the key was not changed, below the field",
    async (locale, code, message) => {
        replace.mockRejectedValue(new ProviderRequestError(code));
        await mount(locale);
        await click(locale === "en" ? "Replace key" : "替换密钥", SECRET);
        const alert = screen.getByRole("alert");
        expect(alert.textContent).toBe(message);
        expect(
            keyField().compareDocumentPosition(alert) &
                Node.DOCUMENT_POSITION_FOLLOWING,
        ).toBeTruthy();
        expectKeyCleared();
        expect(
            screen.queryByRole("button", {
                name: /Refresh key status|刷新密钥状态/,
            }),
        ).toBeNull();
        // A definite failure: the status is still the one read before.
        expect(stateValue().textContent).toMatch(
            /^(Set · updated|已设置 · 更新于)/,
        );
    },
);

test.each([
    "secret_outcome_unknown",
    "operation_failed",
    "invalid_response",
] as const)(
    "after %s the status is unknown until it is refreshed",
    async (code) => {
        replace.mockRejectedValue(new ProviderRequestError(code));
        await mount();
        await click("Replace key", SECRET);
        expect(stateValue().textContent).toBe("Status unknown");
        expect(screen.getByRole("alert").textContent).toMatch(
            /unknown|could not confirm/,
        );
        expect(keyField().value).toBe("");
        const refresh = deferred<ProviderSecretStatusResult>();
        readStatus.mockReturnValue(refresh.promise);
        const reads = readStatus.mock.calls.length;
        await click("Refresh key status");
        expect(readStatus).toHaveBeenCalledTimes(reads + 1);
        // While refreshing, replacing is blocked and focus stays in the group.
        const button = screen.getByRole("button", { name: "Replace key" });
        expect(button.getAttribute("aria-disabled")).toBe("true");
        expect(document.activeElement).toBe(button);
        expect(
            within(group()).getByText("Refreshing the key status…"),
        ).toBeDefined();
        fireEvent.change(keyField(), { target: { value: SECRET } });
        fireEvent.submit(screen.getByRole("form"));
        expect(replace).toHaveBeenCalledTimes(1);
        await act(async () => {
            refresh.resolve(desktop({ ...set, updatedAtMs: NEWER }));
            await refresh.promise;
        });
        expect(stateValue().textContent).toBe(
            `Set · updated ${formatted("en", NEWER)}`,
        );
        // "Set" does not prove which key was stored, and the text says so.
        expect(
            within(group()).getByText(
                "The key status was refreshed. “Set” does not prove that the new key was saved; if you are not sure, replace it again.",
            ),
        ).toBeDefined();
        expect(
            screen.queryByRole("button", { name: "Refresh key status" }),
        ).toBeNull();
        expect(document.activeElement).toBe(button);
        expect(button.getAttribute("aria-disabled")).toBe("false");
        expect(notification()).toBeFalsy();
    },
);

test("in Chinese a failed refresh keeps the unknown state and offers another refresh", async () => {
    replace.mockRejectedValue(
        new ProviderRequestError("secret_outcome_unknown"),
    );
    await mount("zh-CN");
    await click("替换密钥", SECRET);
    expect(stateValue().textContent).toBe("状态未知");
    expect(screen.getByRole("alert").textContent).toBe(
        "无法确认密钥是否已替换，系统凭据库中可能已经是新密钥。请刷新密钥状态；如不确定，请再次替换。",
    );
    readStatus.mockRejectedValue(new ProviderRequestError("read_failed"));
    await click("刷新密钥状态");
    expect(stateValue().textContent).toBe("状态未知");
    expect(
        screen.getAllByRole("alert").map((alert) => alert.textContent),
    ).toContain("无法读取密钥状态，请重试。");
    expect(screen.getByRole("button", { name: "刷新密钥状态" })).toBeDefined();
    // Replacing again is the documented remedy, so it stays available.
    replace.mockResolvedValue(set);
    await click("替换密钥", SECRET);
    expect(replace).toHaveBeenCalledTimes(2);
    expect(stateValue().textContent).toMatch(/^已设置/);
    expect(screen.queryByRole("button", { name: "刷新密钥状态" })).toBeNull();
});

test("a failed status read explains it and reads again", async () => {
    readStatus.mockRejectedValue(new ProviderRequestError("read_failed"));
    await mount();
    expect(screen.getByRole("alert").textContent).toBe(
        "The key status could not be read. Try again.",
    );
    expect(
        screen
            .getByRole("button", { name: "Replace key" })
            .getAttribute("aria-disabled"),
    ).toBe("true");
    await click("Replace key", SECRET);
    expect(replace).not.toHaveBeenCalled();
    readStatus.mockResolvedValue(desktop(set));
    await click("Read key status again");
    expect(document.activeElement).toBe(
        screen.getByRole("button", { name: "Replace key" }),
    );
    expect(stateValue().textContent).toMatch(/^Set · updated/);
    expect(screen.queryByRole("alert")).toBeNull();
});

test("browser preview shows that keys cannot be read or changed and never replaces", async () => {
    readStatus.mockResolvedValue({ kind: "preview" });
    await mount();
    expect(stateValue().textContent).toBe(
        "The browser preview cannot read or change API keys. Open this page in the vibemate desktop app.",
    );
    const button = screen.getByRole("button", { name: "Replace key" });
    expect(button.getAttribute("aria-disabled")).toBe("true");
    expect(keyField().readOnly).toBe(true);
    fireEvent.submit(screen.getByRole("form"));
    await click("Replace key");
    expect(replace).not.toHaveBeenCalled();
    expect(notification()).toBeFalsy();
});

test("a typed key is cleared while the page is hidden", async () => {
    const { setHidden } = await mount();
    fireEvent.change(keyField(), { target: { value: SECRET } });
    expect(keyField().getAttribute("value")).toBe(SECRET);
    setHidden(true);
    expectKeyCleared();
    setHidden(false);
    expect(keyField().value).toBe("");
});

test("a result that arrives after unmount is ignored", async () => {
    const request = deferred<ProviderSecretStatus>();
    replace.mockReturnValue(request.promise);
    await mount();
    await click("Replace key", SECRET);
    cleanup();
    await act(async () => {
        request.resolve(set);
        await request.promise;
    });
    expect(document.body.textContent).toBe("");
});

test("the key never appears in page text, notifications or the console", async () => {
    const spies = (["log", "info", "warn", "error", "debug"] as const).map(
        (method) => vi.spyOn(console, method),
    );
    replace
        .mockRejectedValueOnce(new ProviderRequestError("secret_invalid"))
        .mockRejectedValueOnce(
            new ProviderRequestError("secret_outcome_unknown"),
        )
        .mockResolvedValueOnce(set);
    const { onBusyChange } = await mount();
    for (let attempt = 0; attempt < 3; attempt += 1)
        await click("Replace key", SECRET);
    expect(notification()).toBe("API key replaced.");
    expect(document.body.textContent).not.toContain(SECRET);
    expect(JSON.stringify(onBusyChange.mock.calls)).not.toContain(SECRET);
    for (const spy of spies) {
        expect(JSON.stringify(spy.mock.calls)).not.toContain(SECRET);
        spy.mockRestore();
    }
});

test.each([
    [
        "operation_failed",
        "The key status could not be read because the operation did not finish. Nothing was changed. Try again.",
    ],
    [
        "invalid_response",
        "The app received an unrecognized key status. Nothing was changed. Try again.",
    ],
    [
        "not_found",
        "This provider configuration was not found; it may no longer exist. Go back and refresh the list.",
    ],
    [
        "storage_unavailable",
        "Local configuration storage is unavailable. Restart vibemate.",
    ],
] as const)(
    "a status read failing with %s never suggests that anything changed",
    async (code, message) => {
        readStatus.mockRejectedValue(new ProviderRequestError(code));
        await mount();
        expect(screen.getByRole("alert").textContent).toBe(message);
        expect(stateValue().textContent).toBe("Status unknown");
        expect(
            screen.getByRole("button", { name: "Read key status again" }),
        ).toBeDefined();
    },
);

test("a submit ignored during a refresh still clears a typed key", async () => {
    replace.mockRejectedValueOnce(
        new ProviderRequestError("secret_outcome_unknown"),
    );
    await mount();
    await click("Replace key", SECRET);
    const refresh = deferred<ProviderSecretStatusResult>();
    readStatus.mockReturnValue(refresh.promise);
    await click("Refresh key status");
    // Read-only while refreshing; a forced value is still dropped on submit.
    fireEvent.change(keyField(), { target: { value: SECRET } });
    fireEvent.submit(screen.getByRole("form"));
    expect(replace).toHaveBeenCalledTimes(1);
    expectKeyCleared();
    await act(async () => {
        refresh.resolve(desktop(set));
        await refresh.promise;
    });
});
