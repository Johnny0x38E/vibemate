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
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import {
    addManualProviderModel,
    browseUpstreamModelsPage,
    cancelProviderModelFetch,
    getProviderModelFetchStatus,
    listProviderModels,
    saveProviderModelSelections,
    type ProviderModel,
} from "../../lib/desktop/models";
import { getProviderSecretStatus } from "../../lib/desktop/providerSecrets";
import { ProviderModels } from "./ProviderModels";

vi.mock(import("../../lib/desktop/models"), async (importOriginal) => ({
    ...(await importOriginal()),
    addManualProviderModel: vi.fn(),
    listProviderModels: vi.fn(),
    saveProviderModelSelections: vi.fn(),
    browseUpstreamModelsPage: vi.fn(),
    cancelProviderModelFetch: vi.fn(),
    getProviderModelFetchStatus: vi.fn(),
}));
vi.mock(
    import("../../lib/desktop/providerSecrets"),
    async (importOriginal) => ({
        ...(await importOriginal()),
        getProviderSecretStatus: vi.fn(),
    }),
);

const addManual = vi.mocked(addManualProviderModel);
const list = vi.mocked(listProviderModels);
const saveSelections = vi.mocked(saveProviderModelSelections);
const browse = vi.mocked(browseUpstreamModelsPage);
const cancelFetch = vi.mocked(cancelProviderModelFetch);
const readFetchStatus = vi.mocked(getProviderModelFetchStatus);
const readKey = vi.mocked(getProviderSecretStatus);

const ID = "0123456789abcdef0123456789abcdef";

function model(overrides: Partial<ProviderModel> = {}): ProviderModel {
    return {
        providerId: ID,
        modelId: "deepseek-chat",
        source: "fetched",
        selected: true,
        alias: null,
        upstreamName: "DeepSeek Chat",
        contextWindow: 128000,
        maxOutputTokens: null,
        inputModalities: null,
        outputModalities: null,
        supportedEndpoints: null,
        routeSupport: "not_applicable",
        resolvedProtocol: "chat_completions",
        upstreamState: "listed",
        lastSeenAtMs: 1000,
        missingSinceMs: null,
        createdAtMs: 1000,
        updatedAtMs: 1000,
        ...overrides,
    };
}

function desktopPage(items: ProviderModel[]) {
    return {
        kind: "desktop" as const,
        page: { items, nextCursor: null, totalMatches: null },
    };
}

async function renderModels(hidden = false, locale: "en" | "zh-CN" = "en") {
    const i18n = await createAppI18n(locale);
    const tree = (isHidden: boolean) => (
        <StrictMode>
            <I18nextProvider i18n={i18n}>
                <ProviderModels providerId={ID} hidden={isHidden} />
            </I18nextProvider>
        </StrictMode>
    );
    const view = render(tree(hidden));
    return {
        ...view,
        rerenderHidden: (isHidden: boolean) => {
            view.rerender(tree(isHidden));
        },
    };
}

afterEach(() => {
    cleanup();
});
beforeEach(() => {
    addManual.mockReset();
    list.mockReset();
    saveSelections.mockReset().mockResolvedValue(undefined);
    browse.mockReset();
    cancelFetch.mockReset().mockResolvedValue({ wasRunning: false });
    readFetchStatus.mockReset().mockResolvedValue({
        kind: "desktop",
        status: { running: false, lastFetch: null },
    });
    readKey.mockReset().mockResolvedValue({
        kind: "desktop",
        status: { providerId: ID, state: "set", updatedAtMs: 1000 },
    });
});

describe("ProviderModels", () => {
    test("prompts to fetch when no models are selected", async () => {
        list.mockResolvedValue(desktopPage([]));
        await renderModels();
        await screen.findByText(/Fetch models to browse/i);
    });

    test("shows preview notice in browser mode", async () => {
        list.mockResolvedValue({ kind: "preview" });
        readFetchStatus.mockResolvedValue({ kind: "preview" });
        await renderModels();
        await screen.findByText(/browser preview cannot read saved models/i);
    });

    test("enters upstream browse when Fetch models is clicked", async () => {
        list.mockResolvedValue(desktopPage([]));
        browse.mockResolvedValue({
            items: [
                {
                    modelId: "new-model",
                    upstreamName: "New",
                    contextWindow: null,
                    maxOutputTokens: null,
                    inputModalities: null,
                    outputModalities: null,
                    supportedEndpoints: null,
                    routeSupport: "not_applicable",
                    resolvedProtocol: "chat_completions",
                },
            ],
            nextOffset: null,
            morePages: false,
        });
        await renderModels();
        await screen.findByText(/No models added yet/i);
        fireEvent.click(screen.getByRole("button", { name: "Fetch models" }));
        await waitFor(() => {
            expect(browse).toHaveBeenCalledWith({
                providerId: ID,
                offset: null,
            });
        });
        await screen.findByText("New");
    });

    test("save sends removals for unchecked selected rows", async () => {
        list.mockResolvedValue(desktopPage([model()]));
        await renderModels();
        const checkbox = await screen.findByRole("checkbox", {
            name: "Deselect deepseek-chat",
        });
        fireEvent.click(checkbox);
        fireEvent.click(screen.getByRole("button", { name: "Save" }));
        await waitFor(() => {
            expect(saveSelections).toHaveBeenCalledWith({
                providerId: ID,
                removeModelIds: ["deepseek-chat"],
                add: [],
            });
        });
    });
});

test.each([
    {
        locale: "en" as const,
        fetch: "Fetch models",
        toggle: "Add a model manually",
        modelId: "Model ID",
        alias: "Alias (optional)",
        add: "Add and save",
        view: "View added models",
        checkbox: "Deselect vendor/manual",
    },
    {
        locale: "zh-CN" as const,
        fetch: "获取模型",
        toggle: "手动添加模型",
        modelId: "模型 ID",
        alias: "别名（可选）",
        add: "添加并保存",
        view: "查看已添加",
        checkbox: "取消勾选 vendor/manual",
    },
])(
    "manual addition from upstream browse is saved and visible in added models ($locale)",
    async (labels) => {
        list.mockResolvedValue(desktopPage([]));
        browse.mockResolvedValue({
            items: [],
            nextOffset: null,
            morePages: false,
        });
        const manual = model({
            modelId: "vendor/manual",
            source: "manual",
            alias: "Work",
        });
        addManual.mockResolvedValue(manual);
        await renderModels(false, labels.locale);
        const fetchButton = await screen.findByRole("button", {
            name: labels.fetch,
        });
        await waitFor(() => {
            expect(fetchButton.getAttribute("aria-disabled")).toBe("false");
        });
        fireEvent.click(fetchButton);
        await screen.findByRole("button", { name: labels.view });
        fireEvent.click(
            screen.getByText(labels.toggle, { selector: "summary" }),
        );
        fireEvent.change(screen.getByPlaceholderText(labels.modelId), {
            target: { value: "vendor/manual" },
        });
        fireEvent.change(screen.getByPlaceholderText(labels.alias), {
            target: { value: "Work" },
        });
        await waitFor(() => {
            expect(
                screen
                    .getByRole("button", { name: labels.add })
                    .getAttribute("aria-disabled"),
            ).toBe("false");
        });
        fireEvent.click(screen.getByRole("button", { name: labels.add }));
        await waitFor(() => {
            expect(addManual).toHaveBeenCalledWith({
                providerId: ID,
                modelId: "vendor/manual",
                alias: "Work",
            });
        });
        await waitFor(() => {
            expect(
                screen
                    .getByPlaceholderText(labels.modelId)
                    .getAttribute("value"),
            ).toBe("");
        });
        list.mockResolvedValue(desktopPage([manual]));
        fireEvent.click(screen.getByRole("button", { name: labels.view }));
        await screen.findByRole("checkbox", { name: labels.checkbox });
        expect(saveSelections).not.toHaveBeenCalled();
    },
);

test("manual creation updates the selected row and count without a provider request", async () => {
    list.mockResolvedValue(desktopPage([]));
    const manual = model({
        modelId: "vendor/manual",
        source: "manual",
        alias: "Work",
    });
    addManual.mockResolvedValue(manual);
    const count = vi.fn();
    const i18n = await createAppI18n("en");
    render(
        <I18nextProvider i18n={i18n}>
            <ProviderModels providerId={ID} onSelectedCountChange={count} />
        </I18nextProvider>,
    );
    fireEvent.click(
        await screen.findByText("Add a model manually", {
            selector: "summary",
        }),
    );
    fireEvent.change(screen.getByLabelText("Model ID"), {
        target: { value: "vendor/manual" },
    });
    fireEvent.change(screen.getByLabelText("Alias (optional)"), {
        target: { value: "Work" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add and save" }));
    await screen.findByRole("checkbox", { name: "Deselect vendor/manual" });
    expect(count).toHaveBeenCalledWith({ count: 1, hasMore: false });
    expect(browse).not.toHaveBeenCalled();
    expect(saveSelections).not.toHaveBeenCalled();
    expect(
        screen
            .getByRole("button", { name: "Save" })
            .getAttribute("aria-disabled"),
    ).toBe("true");
});

test("manual rows are removed only when their checkbox draft is saved", async () => {
    list.mockResolvedValue(
        desktopPage([model({ source: "manual", alias: "Custom" })]),
    );
    await renderModels();
    fireEvent.click(
        await screen.findByRole("checkbox", { name: "Deselect deepseek-chat" }),
    );
    expect(saveSelections).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => {
        expect(saveSelections).toHaveBeenCalledWith({
            providerId: ID,
            removeModelIds: ["deepseek-chat"],
            add: [],
        });
    });
});

test("pending checkbox drafts block manual creation", async () => {
    list.mockResolvedValue(desktopPage([model()]));
    await renderModels();
    fireEvent.click(
        await screen.findByRole("checkbox", { name: "Deselect deepseek-chat" }),
    );
    fireEvent.click(
        screen.getByText("Add a model manually", { selector: "summary" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Add and save" }));
    expect(addManual).not.toHaveBeenCalled();
    expect(screen.getByText(/Save or discard checkbox changes/)).toBeDefined();
});

test("manual writes block browsing and late hidden replies do not add rows", async () => {
    list.mockResolvedValue(desktopPage([]));
    let resolve: (model: ProviderModel) => void = () => {};
    addManual.mockImplementation(
        () =>
            new Promise((done) => {
                resolve = done;
            }),
    );
    const view = await renderModels();
    fireEvent.click(
        await screen.findByText("Add a model manually", {
            selector: "summary",
        }),
    );
    fireEvent.change(screen.getByLabelText("Model ID"), {
        target: { value: "vendor/manual" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add and save" }));
    fireEvent.click(screen.getByRole("button", { name: "Fetch models" }));
    expect(browse).not.toHaveBeenCalled();
    view.rerenderHidden(true);
    await act(async () => {
        resolve(model({ source: "manual", modelId: "vendor/manual" }));
        await Promise.resolve();
    });
    await waitFor(() => {
        expect(
            screen.queryByRole("checkbox", { name: "Deselect vendor/manual" }),
        ).toBeNull();
    });
});
