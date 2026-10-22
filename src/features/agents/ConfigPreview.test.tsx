import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import { NotificationProvider } from "../../components/Notifications";
import {
    ApplyRequestError,
    PlanRequestError,
    applyClaudeCodeSettings,
    planClaudeCodeSettings,
    type ApplyStep,
    type ClaudeCodeApplied,
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
import {
    fieldSelectDisabled,
    fieldSelectOptionValues,
    fieldSelectValue,
    setFieldSelectValue,
} from "../../test/fieldSelect";
import { ConfigPreview } from "./ConfigPreview";
import { piModelFactsFixture } from "../../test/piFacts";
import { effortPreferenceFixture } from "../../test/claudePreference";
import {
    getClaudeEffortPreference,
    saveClaudeEffortPreference,
    ClaudePreferenceError,
    type ClaudeEffortPreference,
} from "../../lib/desktop/claudePreference";
vi.mock(import("../../lib/desktop/claudePreference"), async (original) => ({
    ...(await original()),
    getClaudeEffortPreference: vi.fn(),
    saveClaudeEffortPreference: vi.fn(),
}));
const readPreference = vi.mocked(getClaudeEffortPreference);
const rememberPreference = vi.mocked(saveClaudeEffortPreference);
import { readModelConfigurationPreview } from "../../lib/desktop/configurationPreview";
vi.mock(import("../../lib/desktop/configurationPreview"), () => ({
    readModelConfigurationPreview: vi.fn(),
}));
import {
    planPiSettings,
    applyPiSettings,
    reviewedPiPlan,
    type PiModelFact,
    type PiPlan,
    type PiThinkingLevel,
} from "../../lib/desktop/pi";
import {
    planGrokSettings,
    applyGrokSettings,
    reviewedGrokPlan,
    type GrokPlan,
} from "../../lib/desktop/grok";
vi.mock(import("../../lib/desktop/pi"), async (original) => ({
    ...(await original()),
    planPiSettings: vi.fn(),
    applyPiSettings: vi.fn(),
}));

vi.mock(import("../../lib/desktop/grok"), async (original) => ({
    ...(await original()),
    planGrokSettings: vi.fn(),
    applyGrokSettings: vi.fn(),
}));

vi.mock(import("../../lib/desktop/agents"), async (original) => ({
    ...(await original()),
    applyClaudeCodeSettings: vi.fn(),
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
const writePlan = vi.mocked(applyClaudeCodeSettings);
const BACKUP_PATH =
    "/Users/test/Library/Application Support/dev.vibemate.desktop/backups/1700000000000-1-0/settings.json";
const writtenStep: ApplyStep = {
    ordinal: 0,
    targetPath: plan.target.path,
    state: "applied",
    errorCode: null,
    backupPath: BACKUP_PATH,
};
const writtenResult: ClaudeCodeApplied = {
    operationId: "fedcba9876543210fedcba9876543210",
    state: "applied",
    steps: [writtenStep],
    backupPath: BACKUP_PATH,
    restartRequired: true,
};

beforeEach(() => {
    vi.mocked(readModelConfigurationPreview)
        .mockReset()
        .mockImplementation((plan) => {
            const files =
                plan.agent === "pi"
                    ? plan.files
                    : plan.agent === "grokBuild"
                      ? [plan.file]
                      : [{ target: plan.target, changes: plan.changes }];
            return Promise.resolve({
                kind: "desktop",
                files: files.map((file) => ({
                    targetPath: file.target.path,
                    summary: {
                        format: plan.agent === "grokBuild" ? "toml" : "json",
                        reason: null,
                        truncated: false,
                        changes: file.changes.map((change) => ({
                            path: change.keyPath,
                            kind: change.before === null ? "added" : "replaced",
                            before: change.before,
                            after: change.after,
                            secret: change.secret,
                        })),
                    },
                    text: null,
                })),
            });
        });
    loadProviders.mockReset().mockResolvedValue(providersPage([deepseek]));
    loadModels
        .mockReset()
        .mockResolvedValue(modelsPage([row(deepseek.id, FLASH)]));
    readPreference
        .mockReset()
        .mockImplementation(async ({ providerId, modelId }) => {
            // The rendered model selection comes from this completed list request.
            const result =
                loadModels.mock.results[loadModels.mock.results.length - 1];
            if (result?.type !== "return")
                throw new ClaudePreferenceError("read_failed");
            const page = await result.value;
            if (page.kind === "preview") return { kind: "preview" };
            const model = page.page.items.find(
                (item) =>
                    item.providerId === providerId && item.modelId === modelId,
            );
            if (model === undefined)
                throw new ClaudePreferenceError("not_found");
            return {
                kind: "desktop",
                preference: effortPreferenceFixture(model),
            };
        });
    rememberPreference.mockReset();
    buildPlan.mockReset().mockResolvedValue({ kind: "desktop", plan });
    writePlan.mockReset();
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
        expect(buildPlan).toHaveBeenCalledWith(deepseek.id, modelId, null);
    });
}

test("verified DeepSeek Chat bindings offer mapped extended Pi levels without changing the model default", async () => {
    loadProviders.mockResolvedValue(
        providersPage([
            {
                ...deepseek,
                protocol: "chat_completions",
                baseUrl: "https://api.deepseek.com",
            },
        ]),
    );
    loadModels.mockResolvedValue(
        modelsPage([
            { ...piReasoningRow(), resolvedProtocol: "chat_completions" },
        ]),
    );
    await mountPi();
    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Model" }),
        FLASH,
    );
    const thinking = screen.getByRole("combobox", { name: "Thinking level" });
    expect(await fieldSelectOptionValues(thinking)).toContain("max");
    expect(fieldSelectValue(thinking)).toBe("");
    await setFieldSelectValue(thinking, "max");
    vi.mocked(planPiSettings).mockResolvedValue({
        kind: "desktop",
        plan: piPlanWithLevel("max"),
    });
    await userEvent.click(
        screen.getByRole("button", { name: "Preview changes" }),
    );
    await screen.findByRole("region", { name: "Pi model settings" });
    expect(planPiSettings).toHaveBeenLastCalledWith(
        deepseek.id,
        FLASH,
        "/test/pi",
        "max",
    );
});

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
    expect(screen.getByText("credential value hidden")).toBeTruthy();
    expect(
        screen.getByText(
            "Start a new Claude Code session for these settings to take effect.",
        ),
    ).toBeTruthy();
    expect(
        screen.getByText(/is overridden by the environment variable/u),
    ).toBeTruthy();
});

test("reviews a declared context change and applies the same model-bound plan", async () => {
    const contextPlan: ClaudeCodePlan = {
        ...plan,
        changes: [
            ...plan.changes,
            {
                keyPath: "env.CLAUDE_CODE_MAX_CONTEXT_TOKENS",
                before: "200000",
                after: "1048576",
                secret: false,
            },
        ],
    };
    buildPlan.mockResolvedValue({ kind: "desktop", plan: contextPlan });
    writePlan.mockResolvedValue(writtenResult);
    await mount();
    await chooseAndPreview();
    const field = await screen.findByText("env.CLAUDE_CODE_MAX_CONTEXT_TOKENS");
    expect(field.closest("li")?.textContent).toContain("200000");
    expect(field.closest("li")?.textContent).toContain("1048576");
    await userEvent.click(
        screen.getByRole("button", { name: "Apply these settings" }),
    );
    await waitFor(() => {
        expect(writePlan).toHaveBeenCalledWith(deepseek.id, FLASH, {
            targetPath: contextPlan.target.path,
            targetDigest: contextPlan.target.contentDigest,
            providerRevision: contextPlan.source.providerRevision,
            modelContentDigest: contextPlan.source.modelContentDigest,
        });
    });
});

test("keeps an unmapped model selectable and explains that its native window is preserved", async () => {
    buildPlan.mockResolvedValue({
        kind: "desktop",
        plan: {
            ...plan,
            notices: [{ code: "contextWindowNotMapped", detail: null }],
        },
    });
    await mount();
    await chooseAndPreview();
    expect(
        await screen.findByText(
            /existing window declaration and compaction settings are left unchanged/u,
        ),
    ).toBeTruthy();
    expect(screen.queryByText("env.CLAUDE_CODE_MAX_CONTEXT_TOKENS")).toBeNull();
    expect(
        screen
            .getByRole("button", { name: "Apply these settings" })
            .getAttribute("aria-disabled"),
    ).toBe("false");
});

function reasoningRow(providerId = deepseek.id): ProviderModel {
    return {
        ...row(providerId, FLASH),
        capabilityOverrides: {
            thinking: "supported",
            imageInput: null,
            toolCalls: null,
        },
        thinkingEffort: {
            kind: "listed",
            levels: ["none", "minimal", "low", "high", "max"],
            defaultLevel: "high",
        },
    };
}

function planWithEffort(level: "low" | "max"): ClaudeCodePlan {
    return {
        ...plan,
        changes: [
            ...plan.changes,
            {
                keyPath: "env.CLAUDE_CODE_EFFORT_LEVEL",
                before: "high",
                after: level,
                secret: false,
            },
        ],
    };
}

test("chooses only a declared native effort, clears a replaced preview and applies its reviewed choice", async () => {
    loadModels.mockResolvedValue(modelsPage([reasoningRow()]));
    buildPlan.mockResolvedValue({
        kind: "desktop",
        plan: planWithEffort("max"),
    });
    writePlan.mockResolvedValue(writtenResult);
    await mount();
    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Model" }),
        FLASH,
    );
    const effort = await screen.findByRole("combobox", {
        name: "Reasoning effort",
    });
    // A model default is metadata, not permission to choose for the user.
    expect(fieldSelectValue(effort)).toBe("");
    expect(await fieldSelectOptionValues(effort)).toEqual([
        "",
        "low",
        "high",
        "max",
    ]);
    await setFieldSelectValue(effort, "max");
    await userEvent.click(
        screen.getByRole("button", { name: "Preview changes" }),
    );
    await screen.findByText("env.CLAUDE_CODE_EFFORT_LEVEL");
    expect(buildPlan).toHaveBeenLastCalledWith(deepseek.id, FLASH, "max");
    await setFieldSelectValue(effort, "low");
    expect(
        screen.queryByRole("button", { name: "Apply these settings" }),
    ).toBeNull();
    buildPlan.mockResolvedValue({
        kind: "desktop",
        plan: planWithEffort("low"),
    });
    await userEvent.click(
        screen.getByRole("button", { name: "Preview changes" }),
    );
    await screen.findByText("env.CLAUDE_CODE_EFFORT_LEVEL");
    await userEvent.click(
        screen.getByRole("button", { name: "Apply these settings" }),
    );
    await waitFor(() => {
        expect(writePlan).toHaveBeenCalledWith(
            deepseek.id,
            FLASH,
            expect.objectContaining({ selectedEffort: "low" }),
        );
    });
});

test("changing provider or model clears effort drafts even for the same upstream ID", async () => {
    const second = {
        ...deepseek,
        id: openrouter.id,
        displayName: "DeepSeek second account",
    };
    loadProviders.mockResolvedValue(providersPage([deepseek, second]));
    loadModels.mockImplementation(({ providerId }) =>
        Promise.resolve(
            modelsPage([
                reasoningRow(providerId),
                { ...reasoningRow(providerId), modelId: FLASH + "[1m]" },
            ]),
        ),
    );
    await mount();
    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Model" }),
        FLASH,
    );
    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Reasoning effort" }),
        "max",
    );
    await setFieldSelectValue(
        screen.getByRole("combobox", { name: "Provider" }),
        second.id,
    );
    await waitFor(() => {
        expect(loadModels).toHaveBeenLastCalledWith(
            expect.objectContaining({ providerId: second.id }),
        );
    });
    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Model" }),
        FLASH,
    );
    expect(
        fieldSelectValue(
            await screen.findByRole("combobox", { name: "Reasoning effort" }),
        ),
    ).toBe("");
    await setFieldSelectValue(
        screen.getByRole("combobox", { name: "Reasoning effort" }),
        "max",
    );
    await setFieldSelectValue(
        screen.getByRole("combobox", { name: "Model" }),
        FLASH + "[1m]",
    );
    expect(
        fieldSelectValue(
            screen.getByRole("combobox", { name: "Reasoning effort" }),
        ),
    ).toBe("");
    await setFieldSelectValue(
        screen.getByRole("combobox", { name: "Model" }),
        FLASH,
    );
    expect(
        fieldSelectValue(
            screen.getByRole("combobox", { name: "Reasoning effort" }),
        ),
    ).toBe("");
    await userEvent.click(
        screen.getByRole("button", { name: "Preview changes" }),
    );
    await waitFor(() => {
        expect(buildPlan).toHaveBeenLastCalledWith(second.id, FLASH, null);
    });
});

const modelsWithoutEffortChoices: { name: string; model: ProviderModel }[] = [
    {
        name: "unknown reasoning with listed efforts",
        model: {
            ...row(deepseek.id, FLASH),
            thinkingEffort: reasoningRow().thinkingEffort,
        },
    },
    { name: "unknown metadata", model: row(deepseek.id, FLASH) },
    {
        name: "any declaration",
        model: {
            ...reasoningRow(),
            thinkingEffort: { kind: "any", defaultLevel: "max" },
        },
    },
    {
        name: "empty declaration",
        model: {
            ...reasoningRow(),
            thinkingEffort: { kind: "listed", levels: [], defaultLevel: null },
        },
    },
    {
        name: "incompatible levels",
        model: {
            ...reasoningRow(),
            thinkingEffort: {
                kind: "listed",
                levels: ["minimal", "none"],
                defaultLevel: "none",
            },
        },
    },
    {
        name: "unsupported reasoning",
        model: {
            ...reasoningRow(),
            capabilityOverrides: {
                thinking: "unsupported",
                imageInput: null,
                toolCalls: null,
            },
        },
    },
];

test.each(modelsWithoutEffortChoices)(
    "does not invent choices for $name",
    async ({ model }) => {
        loadModels.mockResolvedValue(modelsPage([model]));
        await mount();
        await setFieldSelectValue(
            await screen.findByRole("combobox", { name: "Model" }),
            FLASH,
        );
        const effort = await screen.findByRole("combobox", {
            name: "Reasoning effort",
        });
        expect(await fieldSelectOptionValues(effort)).toEqual([""]);
    },
);

test("remembering changes only the starting preference, locks the binding while saving and reloads it after reopening", async () => {
    const model = reasoningRow();
    const before = effortPreferenceFixture(model);
    const saved = {
        ...before,
        selectedEffort: "max" as const,
        revision: "a".repeat(32),
    };
    let complete: (preference: ClaudeEffortPreference) => void = () => {
        throw new Error("save not started");
    };
    rememberPreference.mockReturnValue(
        new Promise((resolve) => {
            complete = resolve;
        }),
    );
    loadModels.mockResolvedValue(modelsPage([model]));
    buildPlan.mockResolvedValue({
        kind: "desktop",
        plan: planWithEffort("max"),
    });
    await mount();
    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Model" }),
        FLASH,
    );
    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Reasoning effort" }),
        "max",
    );
    await userEvent.click(
        screen.getByRole("button", { name: "Preview changes" }),
    );
    await screen.findByText("env.CLAUDE_CODE_EFFORT_LEVEL");
    await userEvent.dblClick(
        screen.getByRole("button", { name: "Remember choice" }),
    );
    expect(
        screen.queryByRole("button", { name: "Apply these settings" }),
    ).toBeNull();
    expect(
        screen
            .getByRole("combobox", { name: "Provider" })
            .getAttribute("aria-disabled"),
    ).toBe("true");
    expect(
        screen
            .getByRole("combobox", { name: "Model" })
            .getAttribute("aria-disabled"),
    ).toBe("true");
    expect(
        screen
            .getByRole("button", { name: "Preview changes" })
            .getAttribute("aria-disabled"),
    ).toBe("true");
    expect(rememberPreference).toHaveBeenCalledTimes(1);
    expect(rememberPreference).toHaveBeenCalledWith({
        providerId: deepseek.id,
        modelId: FLASH,
        expectedModelDigest: before.snapshot.contentDigest,
        expectedRevision: null,
        selectedEffort: "max",
    });
    complete(saved);
    await screen.findByText(
        "Choice remembered. Agent configuration has not changed.",
    );
    expect(writePlan).not.toHaveBeenCalled();
    cleanup();
    readPreference.mockResolvedValue({ kind: "desktop", preference: saved });
    await mount();
    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Model" }),
        FLASH,
    );
    expect(
        fieldSelectValue(
            await screen.findByRole("combobox", { name: "Reasoning effort" }),
        ),
    ).toBe("max");
    await userEvent.click(
        screen.getByRole("button", { name: "Preview changes" }),
    );
    await waitFor(() => {
        expect(buildPlan).toHaveBeenLastCalledWith(deepseek.id, FLASH, "max");
    });
    expect(rememberPreference).toHaveBeenCalledTimes(1);
    expect(writePlan).not.toHaveBeenCalled();
});

test("failed preference reads block preview and offer reload instead of inventing inheritance", async () => {
    readPreference.mockRejectedValue(new ClaudePreferenceError("read_failed"));
    await mount();
    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Model" }),
        FLASH,
    );
    await screen.findByText(
        "The remembered effort could not be read. Reload the saved choice.",
    );
    await userEvent.click(
        screen.getByRole("button", { name: "Preview changes" }),
    );
    expect(buildPlan).not.toHaveBeenCalled();
    readPreference.mockResolvedValue({
        kind: "desktop",
        preference: effortPreferenceFixture(reasoningRow()),
    });
    await userEvent.click(
        screen.getByRole("button", { name: "Reload saved choice" }),
    );
    expect(
        fieldSelectValue(
            await screen.findByRole("combobox", { name: "Reasoning effort" }),
        ),
    ).toBe("");
    await userEvent.click(
        screen.getByRole("button", { name: "Preview changes" }),
    );
    await waitFor(() => {
        expect(buildPlan).toHaveBeenCalledWith(deepseek.id, FLASH, null);
    });
});

test("an obsolete remembered effort is visible and cannot preview until explicitly changed", async () => {
    loadModels.mockResolvedValue(modelsPage([reasoningRow()]));
    const stale = {
        ...effortPreferenceFixture({
            ...reasoningRow(),
            thinkingEffort: {
                kind: "listed",
                levels: ["low"],
                defaultLevel: "low",
            },
        }),
        selectedEffort: "max" as const,
        revision: "a".repeat(32),
    };
    readPreference.mockResolvedValue({ kind: "desktop", preference: stale });
    rememberPreference.mockResolvedValue({
        ...stale,
        selectedEffort: null,
        revision: "b".repeat(32),
    });
    await mount();
    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Model" }),
        FLASH,
    );
    const effort = await screen.findByRole("combobox", {
        name: "Reasoning effort",
    });
    expect(effort.textContent).toContain("max (not mapped)");
    await userEvent.click(
        screen.getByRole("button", { name: "Preview changes" }),
    );
    expect(buildPlan).not.toHaveBeenCalled();
    await setFieldSelectValue(effort, "");
    await userEvent.click(
        screen.getByRole("button", { name: "Remember choice" }),
    );
    await waitFor(() => {
        expect(rememberPreference).toHaveBeenCalledWith(
            expect.objectContaining({
                expectedRevision: stale.revision,
                selectedEffort: null,
            }),
        );
    });
    await screen.findByText(
        "Choice remembered. Agent configuration has not changed.",
    );
    expect(writePlan).not.toHaveBeenCalled();
});

test("a preference conflict retains the draft and reload explicitly adopts the newer saved value", async () => {
    loadModels.mockResolvedValue(modelsPage([reasoningRow()]));
    rememberPreference.mockRejectedValue(
        new ClaudePreferenceError("preference_changed"),
    );
    await mount();
    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Model" }),
        FLASH,
    );
    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Reasoning effort" }),
        "max",
    );
    await userEvent.click(
        screen.getByRole("button", { name: "Remember choice" }),
    );
    await screen.findByText(
        "Another save changed this preference. Reload the saved choice before editing again.",
    );
    expect(
        fieldSelectValue(
            screen.getByRole("combobox", { name: "Reasoning effort" }),
        ),
    ).toBe("max");
    expect(
        screen
            .getByRole("button", { name: "Remember choice" })
            .getAttribute("aria-disabled"),
    ).toBe("true");
    readPreference.mockResolvedValue({
        kind: "desktop",
        preference: {
            ...effortPreferenceFixture(reasoningRow()),
            selectedEffort: "low",
            revision: "b".repeat(32),
        },
    });
    await userEvent.click(
        screen.getByRole("button", { name: "Reload saved choice" }),
    );
    await waitFor(() => {
        expect(
            fieldSelectValue(
                screen.getByRole("combobox", { name: "Reasoning effort" }),
            ),
        ).toBe("low");
    });
    expect(writePlan).not.toHaveBeenCalled();
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
    // Preview stays inline below the form (no nested dialog to dismiss).
    expect(screen.queryByRole("dialog")).toBeNull();

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

test("applies the shown plan with its own tokens", async () => {
    writePlan.mockResolvedValue(writtenResult);
    await mount();
    await chooseAndPreview();
    await userEvent.click(
        await screen.findByRole("button", { name: "Apply these settings" }),
    );

    // The tokens travel back with the request, so Rust can refuse a stale plan.
    await waitFor(() => {
        expect(writePlan).toHaveBeenCalledExactlyOnceWith(deepseek.id, FLASH, {
            targetPath: plan.target.path,
            targetDigest: plan.target.contentDigest,
            providerRevision: plan.source.providerRevision,
            modelContentDigest: plan.source.modelContentDigest,
        });
    });
    expect(
        await screen.findByText(/Written\. The settings file now holds/u),
    ).toBeTruthy();
    expect(screen.getByText(/Previous content kept at/u).textContent).toContain(
        BACKUP_PATH,
    );
    expect(screen.getByText(/Start a new Claude Code session/u)).toBeTruthy();
    // The applied plan is gone; another write needs a fresh preview.
    expect(
        screen.queryByRole("button", { name: "Apply these settings" }),
    ).toBeNull();
});

test("reports a refused write with the step's own message", async () => {
    writePlan.mockResolvedValue({
        ...writtenResult,
        state: "failed",
        steps: [
            {
                ...writtenStep,
                state: "failed",
                errorCode: "target_changed",
                backupPath: null,
            },
        ],
        backupPath: null,
        restartRequired: false,
    });
    await mount();
    await chooseAndPreview();
    await userEvent.click(
        await screen.findByRole("button", { name: "Apply these settings" }),
    );

    expect(await screen.findByText(/Nothing was written/u)).toBeTruthy();
    expect(
        screen.getByText(/the file changed since the preview/u).textContent,
    ).toContain(plan.target.path);
    // Nothing changed on disk, so no new session is asked for.
    expect(screen.queryByText(/Start a new Claude Code session/u)).toBeNull();
});

test("drops a plan the write found stale", async () => {
    writePlan.mockRejectedValue(new ApplyRequestError("plan_stale"));
    await mount();
    await chooseAndPreview();
    await userEvent.click(
        await screen.findByRole("button", { name: "Apply these settings" }),
    );

    expect(
        await screen.findByText(
            /changed since the preview, so nothing was written/u,
        ),
    ).toBeTruthy();
    // A stale plan cannot be applied, so it is not offered again.
    expect(screen.queryByText("env.ANTHROPIC_MODEL")).toBeNull();
    expect(
        screen.queryByRole("button", { name: "Apply these settings" }),
    ).toBeNull();
});

test("keeps the plan when the key is missing", async () => {
    writePlan.mockRejectedValue(new ApplyRequestError("credential_missing"));
    await mount();
    await chooseAndPreview();
    await userEvent.click(
        await screen.findByRole("button", { name: "Apply these settings" }),
    );

    expect(
        await screen.findByText(/No API key is saved for this provider/u),
    ).toBeTruthy();
    // The file was untouched, so the reviewed plan stays usable after a fix.
    expect(screen.getByText("env.ANTHROPIC_MODEL")).toBeTruthy();
});

test("ignores a second apply while the first is in flight", async () => {
    let finish: (result: ClaudeCodeApplied) => void = () => {};
    writePlan.mockImplementation(
        () =>
            new Promise((resolve) => {
                finish = resolve;
            }),
    );
    await mount();
    await chooseAndPreview();
    const button = await screen.findByRole("button", {
        name: "Apply these settings",
    });
    await userEvent.click(button);
    await userEvent.click(button);
    expect(writePlan).toHaveBeenCalledTimes(1);

    finish(writtenResult);
    expect(
        await screen.findByText(/Written\. The settings file now holds/u),
    ).toBeTruthy();
});

test("Pi reviews both files and applies them once with the auth dependency", async () => {
    const piPlan: PiPlan = {
        agent: "pi",
        providerKey: "vibemate-DeepSeek",
        previousProviderKey: null,
        source: plan.source,
        restartRequired: true,
        selectedThinkingLevel: null,
        modelFacts: piModelFactsFixture,
        authTarget: { ...plan.target, path: "/test/pi/auth.json" },
        files: [
            {
                target: { ...plan.target, path: "/test/pi/models.json" },
                changes: [
                    {
                        keyPath: "apiKey",
                        before: null,
                        after: "••••••••",
                        secret: true,
                    },
                ],
            },
            {
                target: { ...plan.target, path: "/test/pi/settings.json" },
                changes: [
                    {
                        keyPath: "defaultModel",
                        before: "previous",
                        after: FLASH,
                        secret: false,
                    },
                ],
            },
        ],
    };
    vi.mocked(planPiSettings).mockResolvedValue({
        kind: "desktop",
        plan: piPlan,
    });
    let finish: ((r: ClaudeCodeApplied) => void) | undefined;
    vi.mocked(applyPiSettings).mockImplementation(
        () =>
            new Promise((resolve) => {
                finish = resolve;
            }),
    );
    const i18n = await createAppI18n("en");
    render(
        <I18nextProvider i18n={i18n}>
            <NotificationProvider>
                <ConfigPreview agent="pi" entryPath="/test/pi" />
            </NotificationProvider>
        </I18nextProvider>,
    );
    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Model" }),
        FLASH,
    );
    await userEvent.click(
        screen.getByRole("button", { name: "Preview changes" }),
    );
    await screen.findByText("/test/pi/models.json");
    await screen.findByText("/test/pi/settings.json");
    expect(planPiSettings).toHaveBeenCalledWith(
        deepseek.id,
        FLASH,
        "/test/pi",
        null,
    );
    const apply = screen.getByRole("button", { name: "Apply these settings" });
    await userEvent.click(apply);
    await userEvent.click(screen.getByRole("button", { name: "Applying…" }));
    expect(applyPiSettings).toHaveBeenCalledTimes(1);
    expect(applyPiSettings).toHaveBeenCalledWith(
        deepseek.id,
        FLASH,
        "/test/pi",
        reviewedPiPlan(piPlan),
    );
    finish?.({
        ...writtenResult,
        steps: [
            { ...writtenStep, targetPath: "/test/pi/models.json" },
            {
                ...writtenStep,
                ordinal: 1,
                targetPath: "/test/pi/settings.json",
                backupPath: "/private/settings.json",
            },
        ],
    });
    await screen.findByText(
        "Written. Both Pi configuration files now hold the planned settings.",
    );
    expect((await screen.findByRole("status")).textContent).toBe(
        "Configuration written.",
    );
    expect(
        screen.getByText(/Previous content kept at \/private\/settings.json/u),
    ).toBeTruthy();
    expect(
        screen.queryByText(
            "Start a new Claude Code session for these settings to take effect.",
        ),
    ).toBeNull();
});

test("Grok previews its TOML file and applies it once", async () => {
    const alias = "vibemate-deepseek-deepseek-flash";
    const grokPlan: GrokPlan = {
        agent: "grokBuild",
        modelAlias: alias,
        existingAlias: true,
        contextWindow: {
            source: "notStated",
            state: "unchanged",
            value: "64000",
        },
        imageInput: { source: "catalog", state: "written", value: true },
        reasoningEfforts: {
            source: "library",
            state: "written",
            levels: ["high", "medium", "low"],
            defaultLevel: "high",
        },
        source: plan.source,
        restartRequired: true,
        precedence: [
            { name: "managed_config.toml", state: "absent" },
            { name: "requirements.toml", state: "present" },
        ],
        notices: [
            { code: "credentialFromStore", detail: null },
            { code: "envKeyOverridden", detail: alias },
            { code: "overridesNotChecked", detail: null },
        ],
        file: {
            target: { ...plan.target, path: "/Users/test/.grok/config.toml" },
            changes: [
                {
                    keyPath: `model.${alias}.model`,
                    before: null,
                    after: FLASH,
                    secret: false,
                },
                {
                    keyPath: `model.${alias}.api_key`,
                    before: "••••••••",
                    after: "••••••••",
                    secret: true,
                },
                {
                    keyPath: `model.${alias}.vision`,
                    before: null,
                    after: "true",
                    secret: false,
                },
                {
                    keyPath: `model.${alias}.reasoning_efforts`,
                    before: null,
                    after: "high, medium, low; default high",
                    secret: false,
                },
            ],
        },
    };
    const grokPreview = vi.mocked(planGrokSettings);
    grokPreview.mockReset().mockResolvedValue({
        kind: "desktop",
        plan: grokPlan,
    });
    const writeGrokPlan = vi.mocked(applyGrokSettings);
    writeGrokPlan.mockReset();
    let finish: ((r: ClaudeCodeApplied) => void) | undefined;
    writeGrokPlan.mockImplementation(
        () =>
            new Promise((resolve) => {
                finish = resolve;
            }),
    );
    const i18n = await createAppI18n("en");
    render(
        <I18nextProvider i18n={i18n}>
            <ConfigPreview agent="grokBuild" entryPath="/test/grok" />
        </I18nextProvider>,
    );
    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Model" }),
        FLASH,
    );
    await userEvent.click(
        screen.getByRole("button", { name: "Preview changes" }),
    );
    await screen.findByText("/Users/test/.grok/config.toml");
    expect(grokPreview).toHaveBeenCalledWith(deepseek.id, FLASH, "/test/grok");
    const modelSettings = screen.getByRole("region", {
        name: "Grok Build model settings",
    });
    expect(modelSettings.textContent).toContain("64,000 tokens");
    expect(modelSettings.textContent).toContain("Text and image");
    expect(modelSettings.textContent).toContain("Reasoning efforts");
    expect(modelSettings.textContent).toContain(
        "High, Medium, Low (default: High)",
    );
    expect(modelSettings.textContent).toContain(
        "model.vibemate-deepseek-deepseek-flash.reasoning_efforts",
    );
    expect(modelSettings.textContent).toContain(
        "model.vibemate-deepseek-deepseek-flash.vision",
    );
    expect(modelSettings.textContent).toContain(
        "Not written: the value already in the file stays.",
    );
    // The planned credential is described, never carried as a value; the key it
    // replaces is shown only as the mask.
    const key = screen
        .getByText(`model.${alias}.api_key`)
        .closest("li")?.textContent;
    expect(key).toContain("credential value hidden");
    expect(key).toContain("••••••••");
    // The user's default model is not part of this plan.
    expect(screen.queryByText("models.default")).toBeNull();
    expect(
        screen.getByText(/already names an environment variable/u),
    ).toBeTruthy();
    expect(
        screen.getByText(
            "Start a new Grok Build session for this entry to take effect. CLI options, shell environment variables and policy files can override the settings file.",
        ),
    ).toBeTruthy();
    // The one apply action consumes this plan; a second click must not repeat it.
    const apply = screen.getByRole("button", { name: "Apply these settings" });
    await userEvent.click(apply);
    await userEvent.click(screen.getByRole("button", { name: "Applying…" }));
    expect(writeGrokPlan).toHaveBeenCalledTimes(1);
    expect(writeGrokPlan).toHaveBeenCalledWith(
        deepseek.id,
        FLASH,
        "/test/grok",
        reviewedGrokPlan(grokPlan),
    );
    finish?.({
        ...writtenResult,
        steps: [
            {
                ...writtenStep,
                targetPath: "/Users/test/.grok/config.toml",
                backupPath: "/private/grok/config.toml",
            },
        ],
    });
    await screen.findByText(
        "Written. Grok Build's configuration now holds the planned model entry.",
    );
    expect(
        screen.getByText(
            /Previous content kept at \/private\/grok\/config.toml/u,
        ),
    ).toBeTruthy();
    // The Grok card never borrows another adapter's outcome or restart sentence.
    expect(
        screen.queryByText(
            "Start a new Claude Code session for these settings to take effect.",
        ),
    ).toBeNull();
});

test("agent_version_unverified follows the current agent", async () => {
    vi.mocked(planPiSettings).mockRejectedValue(
        new PlanRequestError("agent_version_unverified"),
    );
    vi.mocked(planGrokSettings).mockRejectedValue(
        new PlanRequestError("agent_version_unverified"),
    );

    const i18nPi = await createAppI18n("en");
    const { unmount: unmountPi } = render(
        <I18nextProvider i18n={i18nPi}>
            <ConfigPreview agent="pi" entryPath="/test/pi" />
        </I18nextProvider>,
    );
    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Model" }),
        FLASH,
    );
    await userEvent.click(
        screen.getByRole("button", { name: "Preview changes" }),
    );
    const piAlert = await screen.findByRole("alert");
    expect(piAlert.textContent).toContain("Pi");
    expect(piAlert.textContent).toMatch(/compatible model\/settings format/u);
    unmountPi();

    const i18nGrok = await createAppI18n("en");
    render(
        <I18nextProvider i18n={i18nGrok}>
            <ConfigPreview agent="grokBuild" entryPath="/test/grok" />
        </I18nextProvider>,
    );
    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Model" }),
        FLASH,
    );
    await userEvent.click(
        screen.getByRole("button", { name: "Preview changes" }),
    );
    const grokAlert = await screen.findByRole("alert");
    expect(grokAlert.textContent).not.toContain("Pi");
    expect(grokAlert.textContent).toContain("Grok Build");
    expect(grokAlert.textContent).toContain("1.0.60");
    expect(grokAlert.textContent).toContain(".metadata_version");
    expect(grokAlert.textContent).toContain(
        "docs/user-guide/26-config-reference.md",
    );
});

/** A Pi model whose declared efforts mix levels Pi takes by default with `max`, which needs a mapping. */
function piReasoningRow(modelId = FLASH): ProviderModel {
    return {
        ...row(deepseek.id, modelId),
        capabilityOverrides: {
            thinking: "supported",
            imageInput: null,
            toolCalls: null,
        },
        thinkingEffort: {
            kind: "listed",
            levels: ["none", "low", "high", "max"],
            defaultLevel: "high",
        },
    };
}

/** A Pi preview for the chosen level; the settings file carries exactly that per-model change. */
/** The shared fact list, with the thinking row describing the chosen level. */
function piFactsForLevel(level: PiThinkingLevel | null): PiModelFact[] {
    if (level === null) return piModelFactsFixture;
    return piModelFactsFixture.map((fact) =>
        fact.item === "thinkingLevel"
            ? {
                  ...fact,
                  source: "choice",
                  state: "written",
                  value: level,
              }
            : fact,
    );
}

function piPlanWithLevel(level: PiThinkingLevel | null): PiPlan {
    return {
        agent: "pi",
        providerKey: "vibemate-DeepSeek",
        previousProviderKey: null,
        source: plan.source,
        restartRequired: true,
        selectedThinkingLevel: level,
        modelFacts: piFactsForLevel(level),
        authTarget: { ...plan.target, path: "/test/pi/auth.json" },
        files: [
            {
                target: { ...plan.target, path: "/test/pi/models.json" },
                changes: [
                    {
                        keyPath: "providers[vibemate-DeepSeek].apiKey",
                        before: "••••••••",
                        after: "••••••••",
                        secret: true,
                    },
                ],
            },
            {
                target: { ...plan.target, path: "/test/pi/settings.json" },
                changes: [
                    {
                        keyPath: "defaultProvider",
                        before: "previous",
                        after: "vibemate-DeepSeek",
                        secret: false,
                    },
                    {
                        keyPath: "defaultModel",
                        before: "previous",
                        after: FLASH,
                        secret: false,
                    },
                    ...(level === null
                        ? []
                        : [
                              {
                                  keyPath: `modelThinkingLevels.vibemate-DeepSeek/${FLASH}`,
                                  before: null,
                                  after: level,
                                  secret: false,
                              },
                          ]),
                ],
            },
        ],
    };
}

/** Mount the Pi configuration form with a desktop-like notification host. */
async function mountPi(): Promise<void> {
    const i18n = await createAppI18n("en");
    render(
        <I18nextProvider i18n={i18n}>
            <NotificationProvider>
                <ConfigPreview agent="pi" entryPath="/test/pi" />
            </NotificationProvider>
        </I18nextProvider>,
    );
}

test("Pi offers only declared levels it accepts by default and previews the one chosen", async () => {
    loadModels.mockResolvedValue(modelsPage([piReasoningRow()]));
    vi.mocked(planPiSettings).mockResolvedValue({
        kind: "desktop",
        plan: piPlanWithLevel("high"),
    });
    vi.mocked(applyPiSettings).mockResolvedValue({
        ...writtenResult,
        steps: [
            { ...writtenStep, targetPath: "/test/pi/models.json" },
            {
                ...writtenStep,
                ordinal: 1,
                targetPath: "/test/pi/settings.json",
            },
        ],
    });
    await mountPi();
    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Model" }),
        FLASH,
    );
    const thinking = await screen.findByRole("combobox", {
        name: "Thinking level",
    });
    // `none` is the catalog spelling of Pi's `off`; `max` needs a Pi mapping.
    expect(fieldSelectValue(thinking)).toBe("");
    expect(await fieldSelectOptionValues(thinking)).toEqual([
        "",
        "off",
        "low",
        "high",
    ]);
    expect(
        screen.getByText(/no verified Pi-to-Provider mapping for: max/u),
    ).toBeTruthy();
    await setFieldSelectValue(thinking, "high");
    await userEvent.click(
        screen.getByRole("button", { name: "Preview changes" }),
    );
    await screen.findByText(`modelThinkingLevels.vibemate-DeepSeek/${FLASH}`);
    expect(planPiSettings).toHaveBeenLastCalledWith(
        deepseek.id,
        FLASH,
        "/test/pi",
        "high",
    );
    expect(screen.getByText("Written to modelThinkingLevels.")).toBeTruthy();
    await userEvent.click(
        screen.getByRole("button", { name: "Apply these settings" }),
    );
    await waitFor(() => {
        expect(applyPiSettings).toHaveBeenCalledWith(
            deepseek.id,
            FLASH,
            "/test/pi",
            expect.objectContaining({ selectedThinkingLevel: "high" }),
        );
    });
});

test("changing the Pi model discards the chosen level and its preview", async () => {
    loadModels.mockResolvedValue(
        modelsPage([piReasoningRow(FLASH), piReasoningRow("deepseek-pro")]),
    );
    vi.mocked(planPiSettings).mockResolvedValue({
        kind: "desktop",
        plan: piPlanWithLevel("low"),
    });
    await mountPi();
    const model = await screen.findByRole("combobox", { name: "Model" });
    await setFieldSelectValue(model, FLASH);
    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Thinking level" }),
        "low",
    );
    await userEvent.click(
        screen.getByRole("button", { name: "Preview changes" }),
    );
    await screen.findByText(`modelThinkingLevels.vibemate-DeepSeek/${FLASH}`);

    await setFieldSelectValue(model, "deepseek-pro");
    expect(
        screen.queryByRole("button", { name: "Apply these settings" }),
    ).toBeNull();
    expect(
        fieldSelectValue(
            await screen.findByRole("combobox", { name: "Thinking level" }),
        ),
    ).toBe("");
    vi.mocked(planPiSettings).mockResolvedValue({
        kind: "desktop",
        plan: piPlanWithLevel(null),
    });
    await userEvent.click(
        screen.getByRole("button", { name: "Preview changes" }),
    );
    await waitFor(() => {
        expect(planPiSettings).toHaveBeenLastCalledWith(
            deepseek.id,
            "deepseek-pro",
            "/test/pi",
            null,
        );
    });
});

test("a Pi model without declared reasoning explains why no level can be chosen", async () => {
    loadModels.mockResolvedValue(modelsPage([row(deepseek.id, FLASH)]));
    vi.mocked(planPiSettings).mockResolvedValue({
        kind: "desktop",
        plan: piPlanWithLevel(null),
    });
    await mountPi();
    await setFieldSelectValue(
        await screen.findByRole("combobox", { name: "Model" }),
        FLASH,
    );
    const thinking = await screen.findByRole("combobox", {
        name: "Thinking level",
    });
    expect(fieldSelectDisabled(thinking)).toBe(true);
    expect(
        screen.getByText(
            "Reasoning support is not declared for this model, so no Pi thinking level can be chosen.",
        ),
    ).toBeTruthy();
    // Leaving the level at "Do not overwrite" still previews the unchanged setting.
    await userEvent.click(
        screen.getByRole("button", { name: "Preview changes" }),
    );
    await waitFor(() => {
        expect(planPiSettings).toHaveBeenLastCalledWith(
            deepseek.id,
            FLASH,
            "/test/pi",
            null,
        );
    });
});
