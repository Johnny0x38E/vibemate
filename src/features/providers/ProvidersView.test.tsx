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
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { NotificationProvider } from "../../components/Notifications";
import {
    fieldSelectOptionValues,
    fieldSelectValue,
} from "../../test/fieldSelect";
import { createAppI18n } from "../../i18n";
import {
    createProvider,
    getProvider,
    listProviders,
    listProviderTemplates,
    ProviderRequestError,
    updateProvider,
    type ProviderPageResult,
    type ProviderRecord,
    type ProviderTemplate,
} from "../../lib/desktop/providers";
import {
    getProviderSecretStatus,
    replaceProviderSecret,
} from "../../lib/desktop/providerSecrets";
import { PROVIDER_PAGE_SIZE, ProvidersView } from "./ProvidersView";

// Keep the real error class and constants; only the IPC calls are replaced.
vi.mock(import("../../lib/desktop/providers"), async (importOriginal) => ({
    ...(await importOriginal()),
    createProvider: vi.fn(),
    getProvider: vi.fn(),
    listProviders: vi.fn(),
    listProviderTemplates: vi.fn(),
    updateProvider: vi.fn(),
}));
const create = vi.mocked(createProvider);
const update = vi.mocked(updateProvider);
const read = vi.mocked(getProvider);
const list = vi.mocked(listProviders);
const readTemplates = vi.mocked(listProviderTemplates);

vi.mock(
    import("../../lib/desktop/providerSecrets"),
    async (importOriginal) => ({
        ...(await importOriginal()),
        getProviderSecretStatus: vi.fn(),
        replaceProviderSecret: vi.fn(),
    }),
);
const readKey = vi.mocked(getProviderSecretStatus);
const replaceKey = vi.mocked(replaceProviderSecret);

// Synthetic; never a real key.
const SECRET = "sk-synthetic-view-4444";

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
        protocols: ["chat_completions"],
        defaultProtocol: "chat_completions",
        extensionFields: [],
    },
];

function row(id: string, overrides: Partial<ProviderRecord> = {}) {
    return {
        id: id.repeat(32).slice(0, 32),
        kind: "deepseek",
        displayName: "Work",
        baseUrl: "https://api.deepseek.com",
        protocol: "chat_completions",
        extensions: {},
        revision: 1,
        createdAtMs: 1000,
        updatedAtMs: 1000,
        ...overrides,
    } satisfies ProviderRecord;
}

function page(
    items: ProviderRecord[],
    nextCursor: string | null = null,
): ProviderPageResult {
    return { kind: "desktop", page: { items, nextCursor } };
}

function deferred<T>() {
    let resolve: (value: T) => void = () => {
        throw new Error("Not initialized");
    };
    const promise = new Promise<T>((res) => {
        resolve = res;
    });
    return { promise, resolve };
}

afterEach(cleanup);
beforeEach(() => {
    for (const mock of [create, update, read, list, readTemplates])
        mock.mockReset();
    readTemplates.mockResolvedValue({ kind: "desktop", templates });
    readKey.mockReset().mockImplementation((providerId) =>
        Promise.resolve({
            kind: "desktop",
            status: { providerId, state: "set", updatedAtMs: 5000 },
        }),
    );
    replaceKey.mockReset();
});

let setViewHidden: (hidden: boolean) => void = () => undefined;

async function mount(locale: "en" | "zh-CN" = "en") {
    const instance = await createAppI18n(locale);
    const tree = (hidden: boolean) => (
        <StrictMode>
            <I18nextProvider i18n={instance}>
                <NotificationProvider>
                    <ProvidersView hidden={hidden} />
                </NotificationProvider>
            </I18nextProvider>
        </StrictMode>
    );
    await act(async () => {
        const { rerender } = render(tree(false));
        setViewHidden = (hidden) => {
            rerender(tree(hidden));
        };
        await Promise.resolve();
    });
    return instance;
}

/** Type the synthetic key into the create form's required field. */
function typeKey(label = "API key", value = SECRET): void {
    fireEvent.change(screen.getByLabelText(label), { target: { value } });
}

async function click(name: string | RegExp): Promise<void> {
    await act(async () => {
        fireEvent.click(screen.getByRole("button", { name }));
        await Promise.resolve();
    });
}

/** The Providers page itself, excluding the app-level notification region. */
function view(): HTMLElement {
    const section = document.querySelector("section");
    if (!section) throw new Error("ProvidersView is not rendered");
    return section;
}

/** The save is announced by the app-level notification, never on the page. */
function expectSavedNotification(text: string): void {
    const status = screen
        .getAllByRole("status")
        .find((element) => !view().contains(element));
    expect(status?.textContent).toBe(text);
    expect(view().textContent).not.toContain(text);
    expect(document.activeElement).not.toBe(status);
}

function rows(): HTMLElement[] {
    return within(screen.getByRole("list")).getAllByRole("listitem");
}

function editButton(seed: string): HTMLButtonElement {
    const button = document.querySelector<HTMLButtonElement>(
        `[data-provider-id="${row(seed).id}"]`,
    );
    if (!button) throw new Error(`Missing edit button for ${seed}`);
    return button;
}

async function clickEdit(seed: string): Promise<void> {
    await act(async () => {
        fireEvent.click(editButton(seed));
        await Promise.resolve();
    });
}

test("shows a loading state, then saved rows distinguished by id, never as connected", async () => {
    const pending = deferred<ProviderPageResult>();
    list.mockReturnValue(pending.promise);
    const instance = await mount();
    expect(within(view()).getByRole("status").textContent).toBe(
        "Reading saved provider configurations…",
    );
    await act(async () => {
        pending.resolve(
            page([
                row("a"),
                row("b", { kind: "openrouter", baseUrl: "https://x.example" }),
            ]),
        );
        await pending.promise;
    });
    expect(list).toHaveBeenLastCalledWith({
        after: null,
        limit: PROVIDER_PAGE_SIZE,
    });
    expect(screen.getByRole("heading", { level: 1, name: "Providers" }));
    expect(
        screen.getByRole("list", { name: "Saved provider configurations" }),
    ).toBeDefined();
    // Duplicate names are allowed; rows stay distinct by stable internal id.
    expect(rows()).toHaveLength(2);
    expect(screen.getAllByRole("heading", { name: "Work" })).toHaveLength(2);
    expect(screen.getByText("OpenRouter · Chat Completions")).toBeDefined();
    expect(editButton("b")).toBeDefined();
    expect(document.body.textContent).not.toMatch(/\bconnected\b|available/i);

    await act(async () => {
        await instance.changeLanguage("zh-CN");
    });
    expect(screen.getByRole("heading", { level: 1, name: "服务商" }));
    expect(editButton("b")).toBeDefined();
    expect(document.body.textContent).toContain("这里只列出已保存的配置");
    expect(document.body.textContent).not.toMatch(/已连接|可用/);
});

test.each([
    ["en", "No provider configurations are saved yet."],
    ["zh-CN", "还没有保存的服务商配置。"],
] as const)("shows an empty state in %s", async (locale, text) => {
    list.mockResolvedValue(page([]));
    await mount(locale);
    expect(screen.getByText(text)).toBeDefined();
    expect(screen.queryByRole("list")).toBeNull();
});

test.each([
    ["en", "Provider settings could not be read. Try again.", "Try again"],
    ["zh-CN", "无法读取服务商配置，请重试。", "重试"],
] as const)(
    "a failed read explains it in %s and retries",
    async (locale, message, retry) => {
        // StrictMode reads twice on mount, so failures are set per phase.
        list.mockRejectedValue(new ProviderRequestError("read_failed"));
        await mount(locale);
        expect(screen.getByRole("alert").textContent).toBe(message);
        list.mockResolvedValue(page([row("a")]));
        await click(retry);
        // The retry button is replaced; focus lands on the page heading.
        expect(document.activeElement).toBe(
            screen.getByRole("heading", { level: 1 }),
        );
        expect(rows()).toHaveLength(1);
        expect(screen.queryByRole("alert")).toBeNull();
    },
);

test.each([
    [
        "en",
        "The browser preview cannot read or save provider configurations. Open this page in the vibemate desktop app.",
    ],
    [
        "zh-CN",
        "浏览器预览无法读取或保存服务商配置。请在 vibemate 桌面应用中打开此页面。",
    ],
] as const)(
    "browser preview shows the preview state in %s and offers no save",
    async (locale, text) => {
        readTemplates.mockResolvedValue({ kind: "preview" });
        list.mockResolvedValue({ kind: "preview" });
        await mount(locale);
        expect(within(view()).getByRole("status").textContent).toBe(text);
        expect(screen.queryByRole("list")).toBeNull();
        expect(screen.queryByRole("form")).toBeNull();
        expect(screen.queryAllByRole("button")).toHaveLength(0);
        expect(create).not.toHaveBeenCalled();
    },
);

test("loads more with the opaque cursor, then stops on the last page", async () => {
    const more: (() => Promise<ProviderPageResult>)[] = [
        () => Promise.reject(new ProviderRequestError("read_failed")),
        () => Promise.resolve(page([row("b")], null)),
    ];
    list.mockImplementation(({ after }) =>
        after === null
            ? Promise.resolve(page([row("a")], "cursor-1"))
            : (more.shift() ?? (() => Promise.reject(new Error("extra"))))(),
    );
    await mount();
    expect(rows()).toHaveLength(1);

    await click("Load more");
    expect(list).toHaveBeenLastCalledWith({
        after: "cursor-1",
        limit: PROVIDER_PAGE_SIZE,
    });
    expect(screen.getByRole("alert").textContent).toBe(
        "Provider settings could not be read. Try again.",
    );
    // Loaded rows stay visible and the same cursor is retried.
    expect(rows()).toHaveLength(1);
    const moreButton = screen.getByRole("button", { name: "Load more" });
    moreButton.focus();
    await click("Load more");
    expect(list).toHaveBeenLastCalledWith({
        after: "cursor-1",
        limit: PROVIDER_PAGE_SIZE,
    });
    expect(rows()).toHaveLength(2);
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.queryByRole("button", { name: "Load more" })).toBeNull();
    // The last page removes the button; focus continues at the first new row.
    expect(document.activeElement).toBe(editButton("b"));
});

/** "New" opens the create form directly. */
async function openNew(label = "New configuration"): Promise<void> {
    await click(label);
}

test("New opens the form with the first template, in Rust's order, selected and filled", async () => {
    const [deepseek, openrouter] = templates;
    if (!deepseek || !openrouter)
        throw new Error("fixture needs two templates");
    // Reverse the fixture so the default cannot come from a fixed kind.
    readTemplates.mockResolvedValue({
        kind: "desktop",
        templates: [openrouter, deepseek],
    });
    list.mockResolvedValue(page([row("a")]));
    create.mockResolvedValue(row("c", { kind: "openrouter" }));
    await mount();
    await click("New configuration");

    // The secondary page replaces the list and focuses its title.
    expect(screen.queryByRole("list")).toBeNull();
    expect(document.activeElement).toBe(
        screen.getByRole("heading", {
            level: 1,
            name: "New provider configuration",
        }),
    );
    expect(
        screen.getByRole("heading", { level: 2, name: "Basic information" }),
    ).toBeDefined();
    const kind = screen.getByRole("combobox", { name: "Provider" });
    expect(screen.getAllByRole("combobox")[0]).toBe(kind);
    expect(fieldSelectValue(kind)).toBe("openrouter");
    expect(await fieldSelectOptionValues(kind)).toEqual([
        "openrouter",
        "deepseek",
    ]);
    expect(screen.getByLabelText("Name")).toHaveProperty("value", "OpenRouter");
    expect(screen.getByLabelText("Base URL")).toHaveProperty(
        "value",
        "https://openrouter.ai/api/v1",
    );
    expect(fieldSelectValue(screen.getByLabelText("Protocol"))).toBe(
        "chat_completions",
    );
    typeKey();
    await click("Save");
    expect(create).toHaveBeenCalledWith(
        expect.objectContaining({ kind: "openrouter", secret: SECRET }),
    );
});

test("creating re-reads the list and shows the normalized saved name", async () => {
    let stored = page([row("a")]);
    list.mockImplementation(() => Promise.resolve(stored));
    create.mockImplementation(() => {
        const saved = row("c", { displayName: "Team" });
        stored = page([row("a"), saved]);
        return Promise.resolve(saved);
    });
    await mount();
    await openNew();
    fireEvent.change(screen.getByLabelText("Name"), {
        target: { value: "  Team " },
    });
    typeKey();
    const reads = list.mock.calls.length;
    await click("Save");

    expect(screen.queryByRole("form")).toBeNull();
    // A notification outside the page announces the save; the list keeps no
    // lingering message, and focus lands on the new row's Edit.
    expectSavedNotification("Saved “Team”.");
    expect(document.activeElement).toBe(editButton("c"));
    expect(list).toHaveBeenCalledTimes(reads + 1);
    expect(list).toHaveBeenLastCalledWith({
        after: null,
        limit: PROVIDER_PAGE_SIZE,
    });
    expect(rows()).toHaveLength(2);
    expect(rows()[1]?.textContent).toContain("Team");
});

test("editing one of two same-named rows keeps its identity", async () => {
    const second = row("b", { revision: 4 });
    let stored = page([row("a"), second]);
    list.mockImplementation(() => Promise.resolve(stored));
    update.mockImplementation((input) => {
        const saved = {
            ...second,
            displayName: input.displayName,
            revision: 5,
        };
        stored = page([row("a"), saved]);
        return Promise.resolve(saved);
    });
    await mount();
    await clickEdit("b");
    expect(document.activeElement).toBe(
        screen.getByRole("heading", { level: 1, name: "DeepSeek" }),
    );
    expect(screen.queryByText(second.id)).toBeNull();
    fireEvent.change(screen.getByLabelText("Name"), {
        target: { value: "Home" },
    });
    const reads = list.mock.calls.length;
    await click("Save");

    expect(update).toHaveBeenCalledWith(
        expect.objectContaining({ id: second.id, expectedRevision: 4 }),
    );
    expect(list).toHaveBeenCalledTimes(reads + 1);
    const [first, edited] = rows();
    expect(first?.textContent).toContain("Work");
    expect(first?.textContent).not.toMatch(/\bID\b/);
    expect(edited?.textContent).toContain("Home");
    expect(edited?.textContent).not.toMatch(/\bID\b/);
    expectSavedNotification("Saved “Home”.");
    // Focus returns to the edited row, not to its same-named twin.
    expect(document.activeElement).toBe(editButton("b"));
});

test("a new configuration beyond the loaded pages focuses the page heading", async () => {
    // The new row sorts after the unloaded pages, so the re-read does not show it.
    list.mockResolvedValue(page([row("a")], "cursor-1"));
    create.mockResolvedValue(row("c", { displayName: "Team" }));
    await mount();
    await openNew();
    typeKey();
    await click("Save");

    expect(rows()).toHaveLength(1);
    expectSavedNotification("Saved “Team”.");
    expect(document.activeElement).toBe(
        screen.getByRole("heading", { level: 1, name: "Providers" }),
    );
});

test("focus stays on the saved row once a slower re-read is applied", async () => {
    const reread = deferred<ProviderPageResult>();
    let calls = 0;
    list.mockImplementation(() => {
        calls += 1;
        // StrictMode reads twice on mount; the third read follows the save.
        return calls <= 2 ? Promise.resolve(page([row("a")])) : reread.promise;
    });
    update.mockResolvedValue(row("a", { displayName: "Renamed", revision: 2 }));
    await mount();
    await clickEdit("a");
    await click("Save");
    const edit = (): HTMLElement => editButton("a");
    expect(document.activeElement).toBe(edit());

    await act(async () => {
        reread.resolve(
            page([row("a", { displayName: "Renamed", revision: 2 }), row("b")]),
        );
        await reread.promise;
    });
    expect(rows()).toHaveLength(2);
    expect(document.activeElement).toBe(edit());
});

test("an unknown create outcome re-reads the list before a retry is offered", async () => {
    let stored = page([]);
    list.mockImplementation(() => Promise.resolve(stored));
    create.mockImplementation(() => {
        // The write landed, but its acknowledgment was lost.
        stored = page([row("c", { displayName: "DeepSeek" })]);
        return Promise.reject(new ProviderRequestError("operation_failed"));
    });
    await mount("zh-CN");
    await openNew("新建配置");
    typeKey("API 密钥");
    const reads = list.mock.calls.length;
    await click("保存");
    expect(list).toHaveBeenCalledTimes(reads + 1);
    expect(list).toHaveBeenLastCalledWith({
        after: null,
        limit: PROVIDER_PAGE_SIZE,
    });
    expect(screen.getByRole("button", { name: "再次保存" })).toBeDefined();
    expect(create).toHaveBeenCalledTimes(1);

    // Going back shows the refreshed row that may be the result, unread again.
    await click("返回服务商列表");
    expect(rows()).toHaveLength(1);
    expect(list).toHaveBeenCalledTimes(reads + 1);
    expect(document.activeElement).toBe(
        screen.getByRole("button", { name: "新建配置" }),
    );
});

test("going back is blocked while a save is pending", async () => {
    const pending = deferred<ProviderRecord>();
    list.mockResolvedValue(page([row("a")]));
    update.mockReturnValue(pending.promise);
    await mount();
    await clickEdit("a");
    await click("Save");
    const back = screen.getByRole("button", { name: "Back to provider list" });
    expect(back.getAttribute("aria-disabled")).toBe("true");
    await click("Back to provider list");
    expect(screen.getByRole("form", { name: "Basic information" }));
    await act(async () => {
        pending.resolve(row("a", { revision: 2 }));
        await pending.promise;
    });
    expect(document.activeElement).toBe(editButton("a"));
});

test("a reload discards a load-more result that arrives late", async () => {
    const late = deferred<ProviderPageResult>();
    let stored = page([row("a")], "cursor-1");
    list.mockImplementation(({ after }) =>
        after === null ? Promise.resolve(stored) : late.promise,
    );
    create.mockImplementation(() => {
        stored = page([row("a"), row("c")], "cursor-2");
        return Promise.resolve(row("c"));
    });
    await mount();
    await click("Load more");
    await openNew();
    typeKey();
    await click("Save");
    expect(rows()).toHaveLength(2);
    await act(async () => {
        late.resolve(page([row("d", { displayName: "Stale" })], null));
        await late.promise;
    });
    expect(screen.queryByText("Stale")).toBeNull();
    expect(rows()).toHaveLength(2);
});

test("a failed template read blocks New and Edit until it is retried", async () => {
    readTemplates.mockRejectedValue(
        new ProviderRequestError("storage_unavailable"),
    );
    list.mockResolvedValue(page([row("a")]));
    await mount();
    expect(screen.getByRole("alert").textContent).toBe(
        "Local configuration storage is unavailable. Restart vibemate.",
    );
    const add = screen.getByRole("button", { name: "New configuration" });
    expect(add.getAttribute("aria-disabled")).toBe("true");
    await click("New configuration");
    expect(screen.getByRole("list")).toBeDefined();
    expect(editButton("a").getAttribute("aria-disabled")).toBe("true");
    readTemplates.mockResolvedValue({ kind: "desktop", templates });
    await click("Try reading available providers again");
    expect(add.getAttribute("aria-disabled")).toBe("false");
});

test("Back and Cancel return focus to New without re-reading the list", async () => {
    list.mockResolvedValue(page([row("a")]));
    await mount();
    const reads = list.mock.calls.length;
    const add = screen.getByRole("button", { name: "New configuration" });
    add.focus();

    await click("New configuration");
    await click("Back to provider list");
    expect(document.activeElement).toBe(
        screen.getByRole("button", { name: "New configuration" }),
    );

    await openNew();
    await click("Cancel");
    expect(screen.queryByRole("form")).toBeNull();
    expect(document.activeElement).toBe(
        screen.getByRole("button", { name: "New configuration" }),
    );

    await openNew();
    await click("Back to provider list");
    expect(document.activeElement).toBe(
        screen.getByRole("button", { name: "New configuration" }),
    );
    expect(rows()).toHaveLength(1);
    expect(list).toHaveBeenCalledTimes(reads);
});

test("leaving an edit returns focus to that row's Edit button", async () => {
    list.mockResolvedValue(page([row("a"), row("b")]));
    await mount("zh-CN");
    await clickEdit("b");
    expect(document.activeElement).toBe(
        screen.getByRole("heading", { level: 1, name: "DeepSeek" }),
    );
    expect(screen.getByText("服务商").nextElementSibling?.textContent).toBe(
        "DeepSeek",
    );
    await click("取消");
    expect(document.activeElement).toBe(editButton("b"));
    await clickEdit("a");
    await click("返回服务商列表");
    expect(document.activeElement).toBe(editButton("a"));
});

test("switching language keeps the open page and typed values", async () => {
    list.mockResolvedValue(page([row("a")]));
    const instance = await mount();
    await openNew();
    fireEvent.change(screen.getByLabelText("Name"), {
        target: { value: "Draft" },
    });
    await act(async () => {
        await instance.changeLanguage("zh-CN");
    });
    expect(
        screen.getByRole("heading", { level: 1, name: "新建服务商配置" }),
    ).toBeDefined();
    expect(screen.getByLabelText("名称")).toHaveProperty("value", "Draft");
});

test.each([
    {
        locale: "en",
        add: "New configuration",
        back: "Back",
        backName: "Back to provider list",
    },
    {
        locale: "zh-CN",
        add: "新建配置",
        back: "返回",
        backName: "返回服务商列表",
    },
] as const)(
    "in $locale action icons are decorative and the back link's name starts with its visible text",
    async ({ locale, add, back, backName }) => {
        list.mockResolvedValue(page([row("a")]));
        await mount(locale);
        for (const button of [
            screen.getByRole("button", { name: add }),
            editButton("a"),
        ]) {
            const icon = button.querySelector("svg");
            expect(icon?.getAttribute("aria-hidden")).toBe("true");
        }
        // The visible label stays plain text next to the icon.
        expect(screen.getByRole("button", { name: add }).textContent).toBe(add);

        await click(add);
        const backButton = screen.getByRole("button", { name: backName });
        expect(backButton.textContent).toBe(back);
        expect(
            backButton.querySelector("svg")?.getAttribute("aria-hidden"),
        ).toBe("true");
        // Label in name (WCAG 2.5.3): the accessible name begins with the text.
        expect(backName.startsWith(back)).toBe(true);
    },
);

test("rows show the provider's icon before the name; an unknown kind shows none", async () => {
    const unknown = {
        ...row("c", { displayName: "Future" }),
        // A kind from a newer build must not break the list.
        kind: "future-provider",
    } as unknown as ProviderRecord;
    list.mockResolvedValue(
        page([row("a"), row("b", { kind: "openrouter" }), unknown]),
    );
    await mount();
    const [deepseek, openrouter, future] = rows().map((item) =>
        within(item).getByRole("heading", { level: 2 }),
    );
    const files = (heading: HTMLElement | undefined): (string | null)[] =>
        Array.from(
            heading?.querySelectorAll("img") ?? [],
            (image) => image.getAttribute("src")?.replace(/^.*\//, "") ?? null,
        );
    expect(files(deepseek)).toEqual(["deepseek.svg"]);
    expect(files(openrouter)).toEqual([
        "openrouter.svg",
        "openrouter-volt.svg",
    ]);
    expect(files(future)).toEqual([]);
    // The icon comes first and adds nothing to the heading's name.
    expect(deepseek?.firstElementChild?.querySelector("img")).not.toBeNull();
    expect(screen.getByRole("heading", { level: 2, name: "Future" })).toBe(
        future,
    );
    expect(
        screen.getAllByRole("heading", { level: 2, name: "Work" }),
    ).toHaveLength(2);
});

test("the detail page and the create form show the provider icon next to the provider", async () => {
    list.mockResolvedValue(page([row("b", { kind: "openrouter" })]));
    await mount();
    await clickEdit("b");
    const value = screen.getByText("Provider").nextElementSibling;
    expect(value?.textContent).toBe("OpenRouter");
    expect(value?.querySelectorAll("img")).toHaveLength(2);
    await click("Back to provider list");

    await click("New configuration");
    const combobox = screen.getByRole("combobox", { name: "Provider" });
    const triggerIcon = (): string | null | undefined =>
        combobox.querySelector("img")?.getAttribute("src");
    expect(triggerIcon()).toMatch(/\/deepseek\.svg$/);
    fireEvent.click(combobox);
    const openRouterOption = screen.getByRole("option", {
        name: "OpenRouter",
    });
    expect(openRouterOption.querySelectorAll("img")).toHaveLength(2);
    await userEvent.setup().click(openRouterOption);
    expect(triggerIcon()).toMatch(/\/openrouter\.svg$/);
});

test("the edit page combines configuration and key with one Save while the list shows no key state", async () => {
    list.mockResolvedValue(page([row("a")]));
    await mount();
    expect(readKey).not.toHaveBeenCalled();
    expect(view().textContent).not.toMatch(/key|密钥|待配置/i);
    await clickEdit("a");
    expect(readKey).toHaveBeenCalledWith(row("a").id);
    const basic = screen.getByRole("form", { name: "Basic information" });
    expect(screen.getAllByRole("form")).toHaveLength(1);
    const key = within(basic).getByLabelText("API key");
    expect(key).toHaveProperty("value", "");
    expect(key).toHaveProperty("placeholder", "••••••••");
    expect(
        screen.queryByText(
            /updated|This field is cleared|system credential store/i,
        ),
    ).toBeNull();
    const primaries = screen
        .getAllByRole("button")
        .filter((button) => /primary/.test(button.className));
    expect(primaries.map((button) => button.textContent)).toEqual(["Save"]);
});

test("one Save sends both edited settings and a replacement key and leaves only after success", async () => {
    const pending = deferred<ProviderRecord>();
    list.mockResolvedValue(page([row("a")]));
    update.mockReturnValue(pending.promise);
    await mount();
    await clickEdit("a");
    fireEvent.change(screen.getByLabelText("Name"), {
        target: { value: "Renamed" },
    });
    typeKey();
    await click("Save");
    expect(update).toHaveBeenCalledExactlyOnceWith({
        id: row("a").id,
        expectedRevision: row("a").revision,
        displayName: "Renamed",
        baseUrl: row("a").baseUrl,
        protocol: row("a").protocol,
        extensions: {},
        secret: SECRET,
    });
    expect(replaceKey).not.toHaveBeenCalled();
    const back = screen.getByRole("button", { name: "Back to provider list" });
    expect(back.getAttribute("aria-disabled")).toBe("true");
    await click("Back to provider list");
    expect(
        screen.getByRole("form", { name: "Basic information" }),
    ).toBeDefined();
    expect(screen.getByLabelText("API key")).toHaveProperty("value", "");
    const edited = {
        ...row("a"),
        displayName: "Renamed",
        revision: row("a").revision + 1,
    };
    list.mockResolvedValue(page([edited]));
    await act(async () => {
        pending.resolve(edited);
        await pending.promise;
    });
    expect(screen.queryByRole("form")).toBeNull();
    expect(screen.getByText("Saved “Renamed”.")).toBeDefined();
    expect(document.body.textContent).not.toContain(SECRET);
});

test("a typed key is cleared when the page is hidden or left, while other drafts stay", async () => {
    list.mockResolvedValue(page([row("a")]));
    await mount();
    await openNew();
    fireEvent.change(screen.getByLabelText("Name"), {
        target: { value: "Draft" },
    });
    typeKey();
    setViewHidden(true);
    setViewHidden(false);
    expect(screen.getByLabelText("API key")).toHaveProperty("value", "");
    expect(screen.getByLabelText("Name")).toHaveProperty("value", "Draft");

    typeKey();
    await click("Back to provider list");
    await openNew();
    expect(screen.getByLabelText("API key")).toHaveProperty("value", "");
    await click("Cancel");

    await clickEdit("a");
    typeKey("API key");
    setViewHidden(true);
    setViewHidden(false);
    expect(screen.getByLabelText("API key")).toHaveProperty("value", "");
    typeKey("API key");
    await click("Back to provider list");
    await clickEdit("a");
    expect(screen.getByLabelText("API key")).toHaveProperty("value", "");
    expect(replaceKey).not.toHaveBeenCalled();
});
