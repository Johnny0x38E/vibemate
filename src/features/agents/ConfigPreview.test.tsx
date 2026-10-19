import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
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
import { fieldSelectValue, setFieldSelectValue } from "../../test/fieldSelect";
import { ConfigPreview } from "./ConfigPreview";
import { readModelConfigurationPreview } from "../../lib/desktop/configurationPreview";
vi.mock(import("../../lib/desktop/configurationPreview"), () => ({
    readModelConfigurationPreview: vi.fn(),
}));
import {
    planPiSettings,
    applyPiSettings,
    reviewedPiPlan,
    type PiPlan,
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
    // Preview opens in a modal; dismiss it before changing Provider.
    await userEvent.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() => {
        expect(screen.queryByRole("dialog")).toBeNull();
    });

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
    await screen.findByText("/test/pi/models.json");
    await screen.findByText("/test/pi/settings.json");
    expect(planPiSettings).toHaveBeenCalledWith(deepseek.id, FLASH, "/test/pi");
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
                    keyPath: "models.default",
                    before: "someone-elses",
                    after: alias,
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
    // The planned credential is described, never carried as a value; the key it
    // replaces is shown only as the mask.
    const key = screen
        .getByText(`model.${alias}.api_key`)
        .closest("li")?.textContent;
    expect(key).toContain("credential value hidden");
    expect(key).toContain("••••••••");
    // The default it replaces and the alias it now points at are both visible.
    const fallback = screen.getByText("models.default").closest("li");
    expect(fallback?.textContent).toContain("someone-elses");
    expect(fallback?.textContent).toContain(alias);
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
        "Written. Grok Build's config.toml now holds the planned entry and default.",
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
