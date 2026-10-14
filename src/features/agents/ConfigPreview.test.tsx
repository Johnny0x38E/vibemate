import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import {
    PlanRequestError,
    planClaudeCodeSettings,
    type ClaudeCodePlan,
} from "../../lib/desktop/agents";
import {
    ModelRequestError,
    listProviderModels,
    type ProviderModel,
    type ProviderModelPage,
} from "../../lib/desktop/models";
import {
    ProviderRequestError,
    listProviders,
    type ProviderPage,
    type ProviderRecord,
} from "../../lib/desktop/providers";
import { fieldSelectValue, setFieldSelectValue } from "../../test/fieldSelect";
import { ConfigPreview } from "./ConfigPreview";

vi.mock(import("../../lib/desktop/agents"), async (original) => ({
    ...(await original()),
    planClaudeCodeSettings: vi.fn(),
}));
vi.mock(import("../../lib/desktop/models"), async (original) => ({
    ...(await original()),
    listProviderModels: vi.fn(),
}));
vi.mock(import("../../lib/desktop/providers"), async (original) => ({
    ...(await original()),
    listProviders: vi.fn(),
}));

const deepseek: ProviderRecord = {
    id: "0123456789abcdef0123456789abcdef",
    kind: "deepseek",
    displayName: "DeepSeek 主账号",
    baseUrl: "https://api.deepseek.com/anthropic",
    protocol: "anthropic_messages",
    extensions: {},
    revision: 3,
    createdAtMs: 1,
    updatedAtMs: 2,
    selectedModelCount: 1,
};
const openrouter: ProviderRecord = {
    ...deepseek,
    id: "fedcba9876543210fedcba9876543210",
    kind: "openrouter",
    displayName: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1",
};
const evidence = { state: "unknown" as const, source: null };
const FLASH = "deepseek-flash";
const OPUS = "anthropic/claude-opus-latest";

function row(providerId: string, modelId: string): ProviderModel {
    return {
        providerId,
        modelId,
        source: "fetched",
        selected: true,
        alias: null,
        upstreamName: null,
        contextWindow: null,
        maxOutputTokens: null,
        inputModalities: null,
        outputModalities: null,
        supportedEndpoints: null,
        capabilities: {
            thinking: evidence,
            imageInput: evidence,
            toolCalls: evidence,
        },
        // No user declarations; the provider catalog stays the only source.
        capabilityOverrides: {
            thinking: null,
            imageInput: null,
            toolCalls: null,
        },
        thinkingEffort: null,
        routeSupport: "supported",
        resolvedProtocol: "anthropic_messages",
        upstreamState: "listed",
        lastSeenAtMs: null,
        missingSinceMs: null,
        createdAtMs: 1,
        updatedAtMs: 2,
    };
}

function providersPage(items: ProviderRecord[]): {
    kind: "desktop";
    page: ProviderPage;
} {
    return {
        kind: "desktop",
        page: { items, nextCursor: null, total: items.length },
    };
}

function modelsPage(items: ProviderModel[]): {
    kind: "desktop";
    page: ProviderModelPage;
} {
    return {
        kind: "desktop",
        page: { items, nextCursor: null, totalMatches: null },
    };
}

const plan: ClaudeCodePlan = {
    agent: "claudeCode",
    target: {
        path: "/Users/test/.claude/settings.json",
        exists: true,
        contentDigest:
            "c2e7c5c20563a37b7b8d8e378fad52a60fa0a2c6e30b71f4b46521dfa7280409",
        symlink: false,
    },
    source: {
        providerId: deepseek.id,
        providerRevision: 3,
        modelId: FLASH,
        modelContentDigest:
            "0f4f87db4567232a7f1756aa1534ec1314777b39c3bf5209f87cf9739321cddc",
    },
    changes: [
        {
            keyPath: "env.ANTHROPIC_BASE_URL",
            before: null,
            after: "https://api.deepseek.com/anthropic",
            secret: false,
        },
        {
            keyPath: "env.ANTHROPIC_AUTH_TOKEN",
            before: null,
            after: "••••••••",
            secret: true,
        },
        {
            keyPath: "env.ANTHROPIC_MODEL",
            before: "opus",
            after: FLASH,
            secret: false,
        },
    ],
    notices: [
        { code: "credentialFromStore", detail: null },
        { code: "overridesNotChecked", detail: null },
        { code: "modelKeyOverridden", detail: "opus" },
    ],
    restartRequired: true,
};

const loadProviders = vi.mocked(listProviders);
const loadModels = vi.mocked(listProviderModels);
const buildPlan = vi.mocked(planClaudeCodeSettings);

beforeEach(() => {
    loadProviders.mockReset().mockResolvedValue(providersPage([deepseek]));
    loadModels
        .mockReset()
        .mockResolvedValue(modelsPage([row(deepseek.id, FLASH)]));
    buildPlan.mockReset().mockResolvedValue({ kind: "desktop", plan });
});
afterEach(cleanup);

async function mount(): Promise<void> {
    const i18n = await createAppI18n("en");
    render(
        <StrictMode>
            <I18nextProvider i18n={i18n}>
                <ConfigPreview />
            </I18nextProvider>
        </StrictMode>,
    );
}

/** Choose a model as a user would and build the preview for that pair. */
async function chooseAndPreview(modelId = FLASH): Promise<void> {
    const model = await screen.findByRole("combobox", { name: "Model" });
    await setFieldSelectValue(model, modelId);
    await userEvent.click(
        screen.getByRole("button", { name: "Preview changes" }),
    );
    await waitFor(() => {
        expect(buildPlan).toHaveBeenCalledWith(deepseek.id, modelId);
    });
}

test("browser preview explains that nothing can be read", async () => {
    loadProviders.mockResolvedValue({ kind: "preview" });
    await mount();
    expect(
        await screen.findByText(/browser preview cannot read or write/u),
    ).toBeTruthy();
    expect(loadModels).not.toHaveBeenCalled();
    expect(buildPlan).not.toHaveBeenCalled();
});

test("previews the chosen pair and shows what would change", async () => {
    await mount();
    const provider = await screen.findByRole("combobox", {
        name: "Provider",
    });
    expect(fieldSelectValue(provider)).toBe(deepseek.id);
    await waitFor(() => {
        expect(loadModels).toHaveBeenLastCalledWith({
            providerId: deepseek.id,
            after: null,
            limit: 200,
            filter: "selected",
        });
    });
    // The model is never chosen for the user: it must be picked explicitly.
    expect(
        fieldSelectValue(
            await screen.findByRole("combobox", { name: "Model" }),
        ),
    ).toBe("");

    await chooseAndPreview();

    await screen.findByText("env.ANTHROPIC_MODEL");
    // Each changed key is one row; the replaced value and the planned one are
    // separate text nodes so the stylesheet can tell them apart.
    const modelChange = screen.getByText("env.ANTHROPIC_MODEL").closest("li");
    expect(modelChange?.textContent).toContain("opus");
    expect(modelChange?.textContent).toContain("deepseek-flash");
    expect(screen.getByText("env.ANTHROPIC_BASE_URL")).toBeTruthy();
    expect(screen.getByText("env.ANTHROPIC_AUTH_TOKEN")).toBeTruthy();
    expect(screen.getByText("/Users/test/.claude/settings.json")).toBeTruthy();
    expect(screen.getByText("Exists; other keys stay unchanged")).toBeTruthy();
    expect(
        screen.getByText(
            "The API key from OS credential storage, written as plain text",
        ),
    ).toBeTruthy();
    expect(
        screen.getByText(
            "Start a new Claude Code session for these settings to take effect.",
        ),
    ).toBeTruthy();
    expect(
        screen.getByText(/is overridden by the environment variable/u),
    ).toBeTruthy();
});

test("shows why a combination was refused", async () => {
    buildPlan.mockRejectedValue(new PlanRequestError("gateway_unverified"));
    await mount();
    await chooseAndPreview();
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe(
        "Claude Code has no verified setup for this provider, so no plan was built.",
    );
    expect(screen.queryByText("env.ANTHROPIC_MODEL")).toBeNull();
});

test("follows the chosen instance and drops the plan it replaces", async () => {
    loadProviders.mockResolvedValue(providersPage([deepseek, openrouter]));
    loadModels.mockImplementation((input) =>
        Promise.resolve(
            modelsPage([
                row(
                    input.providerId,
                    input.providerId === deepseek.id ? FLASH : OPUS,
                ),
            ]),
        ),
    );
    await mount();
    await chooseAndPreview();
    await screen.findByText("env.ANTHROPIC_MODEL");

    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Provider" }),
        openrouter.id,
    );
    await waitFor(() => {
        expect(loadModels).toHaveBeenLastCalledWith(
            expect.objectContaining({ providerId: openrouter.id }),
        );
    });
    expect(screen.queryByText("env.ANTHROPIC_MODEL")).toBeNull();
    expect(
        fieldSelectValue(
            await screen.findByRole("combobox", { name: "Model" }),
        ),
    ).toBe("");
});

test("recovers from a failed instance read", async () => {
    loadProviders.mockRejectedValue(
        new ProviderRequestError("storage_unavailable"),
    );
    await mount();
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe(
        "Local configuration storage is unavailable. Restart vibemate.",
    );
    loadProviders.mockResolvedValue(providersPage([deepseek]));
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(
        await screen.findByRole("combobox", { name: "Provider" }),
    ).toBeTruthy();
});

test("recovers from a failed model read", async () => {
    loadModels.mockRejectedValue(new ModelRequestError("read_failed"));
    await mount();
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe("Models could not be read. Try again.");
    loadModels.mockResolvedValue(modelsPage([row(deepseek.id, FLASH)]));
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("combobox", { name: "Model" })).toBeTruthy();
});

test("explains an empty instance list instead of offering a preview", async () => {
    loadProviders.mockResolvedValue(providersPage([]));
    await mount();
    expect(
        await screen.findByText(/No provider configuration is saved yet/u),
    ).toBeTruthy();
    expect(screen.queryByRole("combobox")).toBeNull();
    expect(
        screen.queryByRole("button", { name: "Preview changes" }),
    ).toBeNull();
});

test("explains an instance without added models", async () => {
    loadModels.mockResolvedValue(modelsPage([]));
    await mount();
    expect(await screen.findByText(/has no added model yet/u)).toBeTruthy();
    expect(
        screen.queryByRole("button", { name: "Preview changes" }),
    ).toBeNull();
});

test("turns an unexpected failure into a translated message", async () => {
    loadModels.mockRejectedValue(new Error("private-runtime-error"));
    await mount();
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe(
        "The operation could not finish. Try again.",
    );
    expect(screen.queryByText(/private-runtime-error/u)).toBeNull();
});
