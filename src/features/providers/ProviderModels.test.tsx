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
    cancelProviderModelFetch,
    fetchProviderModels,
    getProviderModelFetchStatus,
    listProviderModels,
    ModelRequestError,
    setProviderModelsSelected,
    type ListProviderModelsInput,
    type ProviderModel,
} from "../../lib/desktop/models";
import { getProviderSecretStatus } from "../../lib/desktop/providerSecrets";
import { MODEL_PAGE_SIZE, ProviderModels } from "./ProviderModels";

vi.mock(import("../../lib/desktop/models"), async (importOriginal) => ({
    ...(await importOriginal()),
    listProviderModels: vi.fn(),
    setProviderModelsSelected: vi.fn(),
    fetchProviderModels: vi.fn(),
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

const list = vi.mocked(listProviderModels);
const setSelected = vi.mocked(setProviderModelsSelected);
const fetchModels = vi.mocked(fetchProviderModels);
const cancelFetch = vi.mocked(cancelProviderModelFetch);
const readFetchStatus = vi.mocked(getProviderModelFetchStatus);
const readKey = vi.mocked(getProviderSecretStatus);

const ID = "0123456789abcdef0123456789abcdef";

function model(overrides: Partial<ProviderModel> = {}): ProviderModel {
    return {
        providerId: ID,
        modelId: "deepseek-chat",
        source: "fetched",
        selected: false,
        alias: null,
        upstreamName: "DeepSeek Chat",
        contextWindow: 128000,
        maxOutputTokens: null,
        inputModalities: null,
        outputModalities: null,
        supportedEndpoints: null,
        routeSupport: "not_applicable",
        upstreamState: "listed",
        lastSeenAtMs: 1000,
        missingSinceMs: null,
        createdAtMs: 1000,
        updatedAtMs: 1000,
        ...overrides,
    };
}

function desktopPage(
    items: ProviderModel[],
    nextCursor: string | null = null,
    totalMatches: number | null = null,
) {
    return {
        kind: "desktop" as const,
        page: { items, nextCursor, totalMatches },
    };
}

function statusWithFetch() {
    return {
        kind: "desktop" as const,
        status: {
            running: false,
            lastFetch: {
                fetchedAtMs: 1000,
                complete: true,
                listedCount: 10,
            },
        },
    };
}

async function renderModels(hidden = false) {
    const i18n = await createAppI18n("en");
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
    vi.useRealTimers();
});
beforeEach(() => {
    list.mockReset();
    setSelected.mockReset();
    fetchModels.mockReset();
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
    test("prompts to fetch before showing a saved catalog", async () => {
        list.mockResolvedValue(desktopPage([]));
        await renderModels();
        await screen.findByText(/Fetch models to download/i);
        const search = screen.getByRole("searchbox");
        if (!(search instanceof HTMLInputElement)) {
            throw new Error("Expected search input");
        }
        expect(search.disabled).toBe(true);
    });

    test("shows preview notice in browser mode", async () => {
        list.mockResolvedValue({ kind: "preview" });
        readFetchStatus.mockResolvedValue({ kind: "preview" });
        await renderModels();
        await screen.findByText(/browser preview cannot read saved models/i);
    });

    test("debounces local filter after a fetch", async () => {
        readFetchStatus.mockResolvedValue(statusWithFetch());
        list.mockResolvedValue(desktopPage([]));
        await renderModels();
        await screen.findByText(/No models are saved/i);
        list.mockClear();
        list.mockResolvedValue(
            desktopPage([model({ modelId: "deepseek-chat" })], null, 1),
        );
        const search = screen.getByRole("searchbox");
        if (!(search instanceof HTMLInputElement)) {
            throw new Error("Expected search input");
        }
        expect(search.disabled).toBe(false);
        fireEvent.change(search, {
            target: { value: "chat" },
        });
        const searchRequest: ListProviderModelsInput = {
            providerId: ID,
            after: null,
            limit: MODEL_PAGE_SIZE,
            filter: "all",
            query: "chat",
        };
        await waitFor(
            () => {
                expect(list).toHaveBeenCalledWith(searchRequest);
            },
            { timeout: 800 },
        );
        await screen.findByText("DeepSeek Chat");
    });

    test("fetch merges models and reloads the list", async () => {
        list.mockResolvedValue(desktopPage([]));
        fetchModels.mockResolvedValue({
            complete: true,
            incompleteReason: null,
            listed: 2,
            added: 2,
            updated: 0,
            markedMissing: 0,
            pruned: 0,
            skippedInvalid: 0,
            fetchedAtMs: 2000,
        });
        readFetchStatus
            .mockResolvedValueOnce({
                kind: "desktop",
                status: { running: false, lastFetch: null },
            })
            .mockResolvedValue(statusWithFetch());
        list.mockResolvedValueOnce(desktopPage([])).mockResolvedValue(
            desktopPage([model()]),
        );
        await renderModels();
        fireEvent.click(screen.getByRole("button", { name: "Fetch models" }));
        await waitFor(() => {
            expect(fetchModels).toHaveBeenCalledWith(ID);
        });
        await screen.findByText("DeepSeek Chat");
    });

    test("filters to selected models only", async () => {
        readFetchStatus.mockResolvedValue(statusWithFetch());
        list.mockResolvedValue(desktopPage([]));
        await renderModels();
        await screen.findByText(/No models are saved/i);
        list.mockClear();
        list.mockResolvedValue(desktopPage([model({ selected: true })]));
        fireEvent.click(screen.getByRole("button", { name: "Selected" }));
        await screen.findByText("DeepSeek Chat");
        const selectedRequest: ListProviderModelsInput = {
            providerId: ID,
            after: null,
            limit: MODEL_PAGE_SIZE,
            filter: "selected",
        };
        expect(list).toHaveBeenLastCalledWith(selectedRequest);
    });

    test("loads more pages when not searching", async () => {
        readFetchStatus.mockResolvedValue(statusWithFetch());
        const first = model({ modelId: "a-model", upstreamName: null });
        const second = model({ modelId: "b-model", upstreamName: null });
        list.mockImplementation((input) => {
            if (input.after === null) {
                return Promise.resolve(desktopPage([first], "cursor-a"));
            }
            return Promise.resolve(desktopPage([second], null));
        });
        await renderModels();
        await screen.findByText("a-model");
        fireEvent.click(screen.getByRole("button", { name: "Load more" }));
        await screen.findByText("b-model");
    });

    test("toggles selection optimistically and rolls back on failure", async () => {
        readFetchStatus.mockResolvedValue(statusWithFetch());
        const row = model({ selected: false });
        list.mockResolvedValue(desktopPage([row]));
        setSelected.mockRejectedValue(
            new ModelRequestError("model_route_not_supported"),
        );
        await renderModels();
        const checkbox = await screen.findByRole("checkbox", {
            name: "Select deepseek-chat",
        });
        fireEvent.click(checkbox);
        await waitFor(() => {
            expect(checkbox.getAttribute("aria-checked")).toBe("true");
        });
        await screen.findByText(/cannot be selected with the current protocol/);
        await waitFor(() => {
            expect(checkbox.getAttribute("aria-checked")).toBe("false");
        });
    });

    test("ignores late list results while hidden", async () => {
        let resolvePage: (
            value: ReturnType<typeof desktopPage>,
        ) => void = () => {
            throw new Error("Not initialized");
        };
        const pending = new Promise<ReturnType<typeof desktopPage>>((res) => {
            resolvePage = res;
        });
        list.mockReturnValue(pending);
        const { rerenderHidden } = await renderModels(false);
        rerenderHidden(true);
        await act(async () => {
            resolvePage(desktopPage([model()]));
            await pending;
        });
        expect(screen.queryByText("DeepSeek Chat")).toBeNull();
    });
});
