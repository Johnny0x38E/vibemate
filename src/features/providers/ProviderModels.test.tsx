import {
    act,
    cleanup,
    within,
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
    getProviderModelSnapshot,
    listProviderModels,
    refreshSavedProviderModels,
    saveProviderModelSelections,
    type ProviderModel,
    updateProviderModelAlias,
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
    getProviderModelSnapshot: vi.fn(),
    refreshSavedProviderModels: vi.fn(),
    updateProviderModelAlias: vi.fn(),
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
        capabilities: {
            thinking: { state: "unknown", source: null },
            imageInput: { state: "unknown", source: null },
            toolCalls: { state: "unknown", source: null },
        },
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
    vi.mocked(getProviderModelSnapshot).mockReset();
    vi.mocked(updateProviderModelAlias).mockReset();
    vi.mocked(refreshSavedProviderModels).mockReset();
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
    test("edits a saved alias without changing membership and blocks competing actions", async () => {
        const row = model();
        list.mockResolvedValue(desktopPage([row]));
        vi.mocked(getProviderModelSnapshot).mockResolvedValue({
            model: row,
            contentDigest: "a".repeat(64),
        });
        vi.mocked(updateProviderModelAlias).mockResolvedValue({
            model: { ...row, alias: "Work model" },
            contentDigest: "b".repeat(64),
        });
        await renderModels();
        const edit = await screen.findByRole("button", {
            name: "Edit alias for deepseek-chat",
        });
        await waitFor(() => {
            expect(edit.getAttribute("aria-disabled")).toBe("false");
        });
        const checkbox = screen.getByRole("checkbox", {
            name: "Deselect deepseek-chat",
        });
        fireEvent.click(checkbox);
        expect(edit.getAttribute("aria-disabled")).toBe("true");
        fireEvent.click(edit);
        expect(getProviderModelSnapshot).not.toHaveBeenCalled();
        fireEvent.click(checkbox);
        edit.focus();
        fireEvent.click(edit);
        const editor = await screen.findByRole("dialog", {
            name: "Edit alias for deepseek-chat",
        });
        const field = within(editor).getByRole<HTMLInputElement>("textbox", {
            name: "Alias (optional)",
        });
        await waitFor(() => {
            expect(field.readOnly).toBe(false);
        });
        fireEvent.change(field, { target: { value: "Work model" } });
        fireEvent.click(screen.getByRole("button", { name: "Save alias" }));
        await screen.findByText("Work model");
        await waitFor(() => {
            expect(document.activeElement).toBe(edit);
        });
        expect(
            screen
                .getByRole("checkbox", { name: "Deselect deepseek-chat" })
                .getAttribute("aria-checked"),
        ).toBe("true");
        expect(saveSelections).not.toHaveBeenCalled();
        expect(browse).not.toHaveBeenCalled();

        // Reopen and cancel with Escape: Base UI closes the dialog and returns
        // focus to the row action that opened it.
        await waitFor(() => {
            expect(edit.getAttribute("aria-disabled")).toBe("false");
        });
        fireEvent.click(edit);
        const reopened = await screen.findByRole("dialog", {
            name: "Edit alias for deepseek-chat",
        });
        const reopenedField = within(reopened).getByRole<HTMLInputElement>(
            "textbox",
            { name: "Alias (optional)" },
        );
        await waitFor(() => {
            expect(reopenedField.readOnly).toBe(false);
        });
        fireEvent.keyDown(reopenedField, { key: "Escape" });
        await waitFor(() => {
            expect(screen.queryByRole("dialog")).toBeNull();
        });
        await waitFor(() => {
            expect(document.activeElement).toBe(edit);
        });
        expect(updateProviderModelAlias).toHaveBeenCalledTimes(1);
    });

    test("hiding models closes the alias editor and a late reply cannot rename a row", async () => {
        const row = model();
        list.mockResolvedValue(desktopPage([row]));
        vi.mocked(getProviderModelSnapshot).mockResolvedValue({
            model: row,
            contentDigest: "a".repeat(64),
        });
        let finish:
            | ((value: { model: ProviderModel; contentDigest: string }) => void)
            | undefined;
        vi.mocked(updateProviderModelAlias).mockImplementation(
            () =>
                new Promise((resolve) => {
                    finish = resolve;
                }),
        );
        const { rerenderHidden } = await renderModels();
        const edit = await screen.findByRole("button", {
            name: "Edit alias for deepseek-chat",
        });
        await waitFor(() => {
            expect(edit.getAttribute("aria-disabled")).toBe("false");
        });
        fireEvent.click(edit);
        await waitFor(() => {
            expect(
                screen.getByRole<HTMLButtonElement>("button", {
                    name: "Save alias",
                }).disabled,
            ).toBe(false);
        });
        fireEvent.click(screen.getByRole("button", { name: "Save alias" }));
        rerenderHidden(true);
        await act(async () => {
            finish?.({
                model: { ...row, alias: "Late reply" },
                contentDigest: "b".repeat(64),
            });
            await Promise.resolve();
        });
        rerenderHidden(false);
        await screen.findByText("DeepSeek Chat");
        expect(screen.queryByText("Late reply")).toBeNull();
        expect(
            screen.queryByRole("form", {
                name: "Edit alias for deepseek-chat",
            }),
        ).toBeNull();
    });

    test("prompts to fetch when no models are selected", async () => {
        list.mockResolvedValue(desktopPage([]));
        await renderModels();
        await screen.findByText(/Fetch models to browse/i);
    });

    test("shows only declared capabilities with accessible evidence hints", async () => {
        list.mockResolvedValue(
            desktopPage([
                model({
                    capabilities: {
                        thinking: {
                            state: "supported",
                            source: "deepseek_model_effort_levels",
                        },
                        imageInput: {
                            state: "unsupported",
                            source: "deepseek_model_input_modalities",
                        },
                        toolCalls: { state: "unknown", source: null },
                    },
                }),
            ]),
        );
        await renderModels(false, "en");
        await screen.findByRole("checkbox", { name: "Deselect deepseek-chat" });
        const capability = screen.getByRole("img", {
            name: "Thinking support declared by the provider catalog",
        });
        expect(capability).toBeTruthy();
        expect(
            screen.queryByRole("img", {
                name: "Image input support declared by the provider catalog",
            }),
        ).toBeNull();
        expect(
            screen.queryByRole("img", {
                name: "Tool-call support declared by the provider catalog",
            }),
        ).toBeNull();
    });

    test("refreshes saved model metadata and re-reads the list", async () => {
        list.mockResolvedValue(desktopPage([model()]));
        vi.mocked(refreshSavedProviderModels).mockResolvedValue({
            complete: true,
            incompleteReason: null,
            refreshed: 1,
            markedMissing: 0,
            skippedInvalid: 0,
            fetchedAtMs: 2000,
        });
        await renderModels();
        await screen.findByRole("checkbox", {
            name: "Deselect deepseek-chat",
        });
        const readsBefore = list.mock.calls.length;
        fireEvent.click(
            screen.getByRole("button", { name: "Refresh saved models" }),
        );
        await waitFor(() => {
            expect(refreshSavedProviderModels).toHaveBeenCalledWith(ID);
        });
        await waitFor(() => {
            expect(list.mock.calls.length).toBeGreaterThan(readsBefore);
        });
        expect(saveSelections).not.toHaveBeenCalled();
        expect(browse).not.toHaveBeenCalled();
    });

    test("blocks refresh while checkbox drafts exist", async () => {
        list.mockResolvedValue(desktopPage([model()]));
        await renderModels();
        fireEvent.click(
            await screen.findByRole("checkbox", {
                name: "Deselect deepseek-chat",
            }),
        );
        const refreshButton = screen.getByRole("button", {
            name: "Refresh saved models",
        });
        expect(refreshButton.getAttribute("aria-disabled")).toBe("true");
        fireEvent.click(refreshButton);
        expect(refreshSavedProviderModels).not.toHaveBeenCalled();
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
                    capabilities: {
                        thinking: { state: "unknown", source: null },
                        imageInput: { state: "unknown", source: null },
                        toolCalls: { state: "unknown", source: null },
                    },
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

test("manual addition from upstream browse is saved and visible in added models", async () => {
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
    await renderModels(false, "en");
    const fetchButton = await screen.findByRole("button", {
        name: "Fetch models",
    });
    await waitFor(() => {
        expect(fetchButton.getAttribute("aria-disabled")).toBe("false");
    });
    fireEvent.click(fetchButton);
    await screen.findByRole("button", { name: "View added models" });
    fireEvent.click(
        screen.getByText("Add a model manually", { selector: "summary" }),
    );
    fireEvent.change(screen.getByPlaceholderText("Model ID"), {
        target: { value: "vendor/manual" },
    });
    fireEvent.change(screen.getByPlaceholderText("Alias (optional)"), {
        target: { value: "Work" },
    });
    await waitFor(() => {
        expect(
            screen
                .getByRole("button", { name: "Add and save" })
                .getAttribute("aria-disabled"),
        ).toBe("false");
    });
    fireEvent.click(screen.getByRole("button", { name: "Add and save" }));
    await waitFor(() => {
        expect(addManual).toHaveBeenCalledWith({
            providerId: ID,
            modelId: "vendor/manual",
            alias: "Work",
        });
    });
    await waitFor(() => {
        expect(
            screen.getByPlaceholderText("Model ID").getAttribute("value"),
        ).toBe("");
    });
    list.mockResolvedValue(desktopPage([manual]));
    fireEvent.click(screen.getByRole("button", { name: "View added models" }));
    await screen.findByRole("checkbox", { name: "Deselect vendor/manual" });
    expect(saveSelections).not.toHaveBeenCalled();
});

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
