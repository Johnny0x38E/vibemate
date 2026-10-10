import {
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
});
beforeEach(() => {
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
                },
            ],
            nextOffset: null,
            morePages: false,
        });
        await renderModels();
        await screen.findByText(/No models selected yet/i);
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
