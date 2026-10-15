import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";
import {
    APPLY_ERROR_CODES,
    PLAN_ERROR_CODES,
    RESTORE_ERROR_CODES,
    applyClaudeCodeRestore,
    applyClaudeCodeSettings,
    listClaudeCodeOperations,
    planClaudeCodeRestore,
    planClaudeCodeSettings,
    reviewedPlanTokens,
    type ApplyStep,
    type ClaudeCodeApplied,
    type ClaudeCodePlan,
    type ClaudeCodeRestorePlan,
    type ConfigChange,
    type RecordedOperation,
    type RestoreOutcome,
    type RecordedOperationStep,
    type RestorePlanStep,
    type RestoreStepOutcome,
    discoverAgents,
    type AgentObservation,
} from "./agents";
import { agentReport, automaticAgentPaths } from "../../test/agentDiscovery";
vi.mock(import("@tauri-apps/api/core"));
const pi: AgentObservation = {
    id: "pi",
    state: "found",
    source: "manual",
    entryPath: "/home/test/pi",
    resolvedPath: "/home/test/package/dist/bundle/cli.js",
    version: "1.1.0",
    versionEvidence: "packageMetadata",
    capabilityEvidence: "documented",
    configDirectory: "/home/test/.pi/agent",
    directorySource: "default",
    files: ["settings.json", "models.json", "mcp.json"].map((name) => ({
        name,
        state: "missing",
    })),
};
const grok: AgentObservation = {
    ...pi,
    id: "grokBuild",
    source: "none",
    state: "notFound",
    entryPath: null,
    resolvedPath: null,
    version: null,
    versionEvidence: "none",
    capabilityEvidence: "unverified",
    configDirectory: "/home/test/.grok",
    files: ["config.toml", "managed_config.toml", "requirements.toml"].map(
        (name) => ({ name, state: "missing" }),
    ),
};
const input = { ...automaticAgentPaths, piPath: "/home/test/pi" };
beforeEach(() => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(invoke)
        .mockReset()
        .mockResolvedValue(agentReport([pi, grok]));
});
test("browser preview never scans or invents installations", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(await discoverAgents(input)).toEqual({ kind: "preview" });
    expect(invoke).not.toHaveBeenCalled();
});
test("sends only selected paths and validates six adapter observations", async () => {
    expect(await discoverAgents(input)).toEqual({
        kind: "desktop",
        items: agentReport([pi, grok]),
    });
    expect(invoke).toHaveBeenCalledExactlyOnceWith("discover_agents", {
        input,
    });
});
test.each([
    "invalid_path",
    "location_unavailable",
    "operation_failed",
    "private-path-and-secret",
    new Error("private-runtime-error"),
])("sanitizes failure %s", async (error) => {
    vi.mocked(invoke).mockRejectedValue(error);
    await expect(discoverAgents(input)).rejects.toMatchObject({
        code:
            error === "invalid_path" || error === "location_unavailable"
                ? error
                : "operation_failed",
        message: "Agent discovery failed.",
    });
});
test.each([
    null,
    [],
    [pi],
    [pi, pi],
    [grok, pi],
    [pi, grok, grok],
    [{ ...pi, secret: "synthetic-never-display" }, grok],
    [{ ...pi, entryPath: "relative/pi" }, grok],
    [{ ...pi, resolvedPath: "/tmp/unsafe\npi" }, grok],
    [{ ...pi, configDirectory: "/" + "x".repeat(4096) }, grok],
    [{ ...pi, version: "synthetic-secret" }, grok],
    [{ ...pi, version: "1.1.0\n" }, grok],
    [{ ...pi, version: "1.2.0" }, grok],
    [{ ...pi, versionEvidence: "filename" }, grok],
    [{ ...pi, state: "connected" }, grok],
    [{ ...pi, source: "none" }, grok],
    [{ ...pi, state: "notFound" }, grok],
    [{ ...pi, resolvedPath: null }, grok],
    [{ ...pi, configDirectory: null }, grok],
    [{ ...pi, files: [] }, grok],
    [
        { ...pi, files: pi.files.map((f) => ({ ...f, content: "private" })) },
        grok,
    ],
    [
        {
            ...pi,
            files: [
                { name: "auth.json", state: "present" },
                ...pi.files.slice(1),
            ],
        },
        grok,
    ],
    [
        {
            ...pi,
            files: [
                { name: "settings.json", state: "invalid" },
                ...pi.files.slice(1),
            ],
        },
        grok,
    ],
    [pi, { ...grok, capabilityEvidence: "documented" }],
])(
    "rejects malformed or contradictory response %#",
    async (response: unknown) => {
        // Expand legacy two-entry cases so malformed fields are tested after
        // the six-entry length check, rather than all failing at that check.
        const report = agentReport();
        const expanded =
            Array.isArray(response) && response.length === 2
                ? [
                      report[0],
                      report[1],
                      report[2],
                      response[0],
                      response[1],
                      report[5],
                  ]
                : response;
        vi.mocked(invoke).mockResolvedValue(expanded);
        await expect(discoverAgents(input)).rejects.toMatchObject({
            code: "invalid_response",
        });
    },
);
test("preserves spaces, Unicode, Windows drive and UNC paths as display data", async () => {
    for (const entryPath of [
        "/home/test user/入口/pi",
        "C:\\Users\\test user\\pi.cmd",
        "\\\\server\\share\\pi.cmd",
    ]) {
        vi.mocked(invoke).mockResolvedValue(
            agentReport([{ ...pi, entryPath, resolvedPath: entryPath }, grok]),
        );
        const result = await discoverAgents(input);
        expect(
            result.kind === "desktop" ? result.items[3]?.entryPath : null,
        ).toBe(entryPath);
    }
});
test("accepts unavailable directory and unverified filename evidence without declaring support", async () => {
    const row: AgentObservation = {
        ...grok,
        state: "found",
        source: "path",
        entryPath: "/home/test/grok",
        resolvedPath: "/home/test/grok-1.0.50-linux-x86_64",
        version: "1.0.50",
        versionEvidence: "filename",
        configDirectory: null,
        directorySource: "unavailable",
        files: grok.files.map((f) => ({ ...f, state: "unavailable" })),
    };
    vi.mocked(invoke).mockResolvedValue(agentReport([pi, row]));
    expect(await discoverAgents(input)).toEqual({
        kind: "desktop",
        items: agentReport([pi, row]),
    });
});

test.each(["claudeCode", "codex", "openCode", "hermes"] as const)(
    "validates %s evidence and files rather than reusing Grok schema",
    async (id) => {
        const report = agentReport();
        const row = report.find((entry) => entry.id === id);
        if (row === undefined) throw Error("missing test entry");
        for (const patch of [
            { files: pi.files },
            {
                state: "found",
                source: "path",
                entryPath: "/tmp/entry",
                resolvedPath: "/tmp/entry",
                version: "1.1.0",
                versionEvidence: "packageMetadata",
                capabilityEvidence: "documented",
            },
            { secret: "synthetic-do-not-display" },
            { configDirectory: "relative/config" },
        ]) {
            vi.mocked(invoke).mockResolvedValue(
                report.map((entry) =>
                    entry.id === id ? { ...row, ...patch } : entry,
                ),
            );
            await expect(discoverAgents(input)).rejects.toMatchObject({
                code: "invalid_response",
            });
        }
    },
);

test("rejects complete reports with duplicate, reordered or extra identities", async () => {
    const report = agentReport();
    for (const invalidReport of [
        report.map((entry) => (entry.id === "codex" ? report[0] : entry)),
        [...report].reverse(),
        [...report, report[0]],
        report.filter((entry) => entry.id !== "hermes"),
    ]) {
        vi.mocked(invoke).mockResolvedValue(invalidReport);
        await expect(discoverAgents(input)).rejects.toMatchObject({
            code: "invalid_response",
        });
    }
});

/** One valid plan shape; Rust owns these camelCase fields. */
const plan: ClaudeCodePlan = {
    agent: "claudeCode",
    target: {
        path: "/home/test/.claude/settings.json",
        exists: true,
        contentDigest:
            "c2e7c5c20563a37b7b8d8e378fad52a60fa0a2c6e30b71f4b46521dfa7280409",
        symlink: false,
    },
    source: {
        providerId: "0123456789abcdef0123456789abcdef",
        providerRevision: 3,
        modelId: "deepseek-flash",
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
            after: "deepseek-flash",
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
const modelChange: ConfigChange = {
    keyPath: "env.ANTHROPIC_MODEL",
    before: "opus",
    after: "deepseek-flash",
    secret: false,
};

test("browser preview never requests or invents a plan", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(
        await planClaudeCodeSettings(
            plan.source.providerId,
            plan.source.modelId,
        ),
    ).toEqual({ kind: "preview" });
    expect(invoke).not.toHaveBeenCalled();
});

test("sends only the selected identities and returns the validated plan", async () => {
    vi.mocked(invoke).mockResolvedValue(plan);
    expect(
        await planClaudeCodeSettings(
            plan.source.providerId,
            plan.source.modelId,
        ),
    ).toEqual({ kind: "desktop", plan });
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
        "plan_claude_code_settings",
        {
            providerId: plan.source.providerId,
            modelId: plan.source.modelId,
        },
    );
});

test("accepts a plan for a settings file that does not exist yet", async () => {
    const missing: ClaudeCodePlan = {
        ...plan,
        target: {
            ...plan.target,
            exists: false,
            contentDigest: null,
            symlink: false,
        },
        changes: plan.changes.map((change) => ({ ...change, before: null })),
    };
    vi.mocked(invoke).mockResolvedValue(missing);
    expect(
        await planClaudeCodeSettings(
            plan.source.providerId,
            plan.source.modelId,
        ),
    ).toEqual({ kind: "desktop", plan: missing });
});

test("preserves model IDs with separators as display data", async () => {
    const slashed: ClaudeCodePlan = {
        ...plan,
        source: { ...plan.source, modelId: "deepseek/deepseek-flash:latest" },
        changes: plan.changes.map((change) =>
            change.keyPath === "env.ANTHROPIC_MODEL"
                ? { ...change, after: "deepseek/deepseek-flash:latest" }
                : change,
        ),
    };
    vi.mocked(invoke).mockResolvedValue(slashed);
    const result = await planClaudeCodeSettings(
        plan.source.providerId,
        plan.source.modelId,
    );
    expect(result.kind === "desktop" ? result.plan.source.modelId : null).toBe(
        "deepseek/deepseek-flash:latest",
    );
});

test.each(PLAN_ERROR_CODES)("returns the refusal code %s", async (code) => {
    vi.mocked(invoke).mockRejectedValue(code);
    await expect(
        planClaudeCodeSettings(plan.source.providerId, plan.source.modelId),
    ).rejects.toMatchObject({
        code,
        message: "Agent configuration preview failed.",
    });
});

test.each([
    "private-refusal",
    "sk-synthetic-never-display",
    new Error("private-runtime-error"),
])("sanitizes unexpected failure %s", async (error) => {
    vi.mocked(invoke).mockRejectedValue(error);
    await expect(
        planClaudeCodeSettings(plan.source.providerId, plan.source.modelId),
    ).rejects.toMatchObject({ code: "operation_failed" });
});

test.each([
    { ...plan, extra: "synthetic" },
    { ...plan, agent: "codex" },
    { ...plan, restartRequired: "yes" },
    { ...plan, changes: [] },
    { ...plan, notices: "none" },
    { ...plan, target: { ...plan.target, contentDigest: null } },
    { ...plan, target: { ...plan.target, exists: false } },
    { ...plan, target: { ...plan.target, path: "relative/settings.json" } },
    {
        ...plan,
        target: {
            ...plan.target,
            contentDigest: plan.source.modelContentDigest.toUpperCase(),
        },
    },
    { ...plan, target: { ...plan.target, secret: plan.target.path } },
    { ...plan, source: { ...plan.source, providerId: "not-a-provider-id" } },
    { ...plan, source: { ...plan.source, providerRevision: 0 } },
    { ...plan, source: { ...plan.source, modelContentDigest: "0".repeat(63) } },
    { ...plan, source: { ...plan.source, modelId: "deepseek flash" } },
    { ...plan, source: { ...plan.source, modelId: "" } },
    { ...plan, changes: [{ ...modelChange, secret: true }] },
    { ...plan, changes: [{ ...modelChange, after: "••••••••" }] },
    { ...plan, changes: [{ ...modelChange, extra: true }] },
    { ...plan, changes: [{ ...modelChange, keyPath: "" }] },
    { ...plan, notices: [{ code: "unknownNotice", detail: null }] },
    { ...plan, notices: [{ code: "providerLimited", detail: 7 }] },
    { ...plan, notices: [{ code: "providerLimited" }] },
    {
        ...plan,
        notices: [{ code: "providerLimited", detail: "x".repeat(300) }],
    },
])("rejects malformed or contradictory plan %#", async (response: unknown) => {
    vi.mocked(invoke).mockResolvedValue(response);
    await expect(
        planClaudeCodeSettings(plan.source.providerId, plan.source.modelId),
    ).rejects.toMatchObject({
        name: "PlanRequestError",
        code: "invalid_response",
        message: "Agent configuration preview failed.",
    });
});

const BACKUP_PATH =
    "/home/test/Library/Application Support/dev.vibemate.desktop/backups/1700000000000-1-0/settings.json";
const appliedStep: ApplyStep = {
    ordinal: 0,
    targetPath: plan.target.path,
    state: "applied",
    errorCode: null,
    backupPath: BACKUP_PATH,
};
const applied: ClaudeCodeApplied = {
    operationId: "fedcba9876543210fedcba9876543210",
    state: "applied",
    steps: [appliedStep],
    backupPath: BACKUP_PATH,
    restartRequired: true,
};

test("browser preview refuses to apply and issues no call", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    await expect(
        applyClaudeCodeSettings(
            plan.source.providerId,
            plan.source.modelId,
            reviewedPlanTokens(plan),
        ),
    ).rejects.toMatchObject({
        name: "ApplyRequestError",
        code: "desktop_required",
    });
    expect(invoke).not.toHaveBeenCalled();
});

test("sends the reviewed tokens and returns the validated outcome", async () => {
    vi.mocked(invoke).mockResolvedValue(applied);
    const reviewed = reviewedPlanTokens(plan);
    expect(reviewed).toEqual({
        targetPath: plan.target.path,
        targetDigest: plan.target.contentDigest,
        providerRevision: plan.source.providerRevision,
        modelContentDigest: plan.source.modelContentDigest,
    });
    expect(
        await applyClaudeCodeSettings(
            plan.source.providerId,
            plan.source.modelId,
            reviewed,
        ),
    ).toEqual(applied);
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
        "apply_claude_code_settings",
        {
            input: {
                providerId: plan.source.providerId,
                modelId: plan.source.modelId,
                reviewed,
            },
        },
    );
});

test("accepts a refused write as a settled outcome", async () => {
    const refused: ClaudeCodeApplied = {
        ...applied,
        state: "failed",
        steps: [
            {
                ...appliedStep,
                state: "failed",
                errorCode: "symlink_refused",
                backupPath: null,
            },
        ],
        backupPath: null,
    };
    vi.mocked(invoke).mockResolvedValue(refused);
    expect(
        await applyClaudeCodeSettings(
            plan.source.providerId,
            plan.source.modelId,
            reviewedPlanTokens(plan),
        ),
    ).toEqual(refused);
});

test("accepts a rollback that left one file alone", async () => {
    const partial: ClaudeCodeApplied = {
        ...applied,
        state: "partial",
        steps: [
            appliedStep,
            {
                ordinal: 1,
                targetPath: "/home/test/.claude/models.json",
                state: "failed",
                errorCode: "target_changed",
                backupPath: null,
            },
        ],
    };
    vi.mocked(invoke).mockResolvedValue(partial);
    expect(
        await applyClaudeCodeSettings(
            plan.source.providerId,
            plan.source.modelId,
            reviewedPlanTokens(plan),
        ),
    ).toEqual(partial);
});

test.each(APPLY_ERROR_CODES)(
    "returns the apply refusal code %s",
    async (code) => {
        vi.mocked(invoke).mockRejectedValue(code);
        await expect(
            applyClaudeCodeSettings(
                plan.source.providerId,
                plan.source.modelId,
                reviewedPlanTokens(plan),
            ),
        ).rejects.toMatchObject({
            code,
            message: "Applying the Agent configuration failed.",
        });
    },
);

test.each([
    "private-refusal",
    "sk-synthetic-never-display",
    new Error("private-runtime-error"),
])("sanitizes unexpected apply failure %s", async (error) => {
    vi.mocked(invoke).mockRejectedValue(error);
    await expect(
        applyClaudeCodeSettings(
            plan.source.providerId,
            plan.source.modelId,
            reviewedPlanTokens(plan),
        ),
    ).rejects.toMatchObject({ code: "operation_failed" });
});

test.each([
    { ...applied, extra: "synthetic" },
    { ...applied, operationId: "fedcba9876543210fedcba987654321" },
    { ...applied, operationId: "FEDCBA9876543210FEDCBA9876543210" },
    { ...applied, state: "running" },
    { ...applied, state: "planned" },
    { ...applied, state: "failed" },
    { ...applied, restartRequired: "yes" },
    { ...applied, steps: [] },
    { ...applied, steps: [{ ...appliedStep, ordinal: 1 }] },
    { ...applied, steps: [{ ...appliedStep, extra: true }] },
    { ...applied, steps: [{ ...appliedStep, state: "pending" }] },
    { ...applied, steps: [{ ...appliedStep, state: "failed" }] },
    {
        ...applied,
        steps: [{ ...appliedStep, errorCode: "write_failed" }],
    },
    {
        ...applied,
        steps: [{ ...appliedStep, errorCode: "private_refusal" }],
    },
    {
        ...applied,
        steps: [{ ...appliedStep, targetPath: "settings.json" }],
    },
    {
        ...applied,
        steps: [{ ...appliedStep, backupPath: "backups/settings.json" }],
    },
    { ...applied, backupPath: "/home/test/elsewhere/settings.json" },
    { ...applied, backupPath: "relative/settings.json" },
])("rejects malformed or contradictory apply outcome %#", async (response) => {
    vi.mocked(invoke).mockResolvedValue(response);
    await expect(
        applyClaudeCodeSettings(
            plan.source.providerId,
            plan.source.modelId,
            reviewedPlanTokens(plan),
        ),
    ).rejects.toMatchObject({
        name: "ApplyRequestError",
        code: "invalid_response",
        message: "Applying the Agent configuration failed.",
    });
});

const recordedStep: RecordedOperationStep = {
    ordinal: 0,
    targetPath: plan.target.path,
    state: "applied",
    restoreState: null,
};
const recorded: RecordedOperation = {
    operationId: "fedcba9876543210fedcba9876543210",
    state: "applied",
    providerId: plan.source.providerId,
    providerLabel: "DeepSeek Work",
    createdAt: 1_700_000_000_000,
    updatedAt: 1_700_000_000_500,
    restorable: true,
    steps: [recordedStep],
};
const restorePlanStep: RestorePlanStep = {
    ordinal: 0,
    targetPath: plan.target.path,
    action: "restore",
    reason: null,
    currentDigest: "a".repeat(64),
    appliedDigest: "b".repeat(64),
    restoreState: null,
    restoredProviderId: null,
    restoredProviderLabel: null,
};
const restorePlan: ClaudeCodeRestorePlan = {
    operationId: recorded.operationId,
    state: "applied",
    createdAt: recorded.createdAt,
    restorable: true,
    providerId: recorded.providerId,
    providerLabel: recorded.providerLabel,
    steps: [restorePlanStep],
};
const restoredStep: RestoreStepOutcome = {
    ordinal: 0,
    targetPath: plan.target.path,
    action: "restore",
    state: "restored",
    errorCode: null,
    leftoverPath: null,
};
const restoreOutcome: RestoreOutcome = {
    operationId: recorded.operationId,
    state: "compensated",
    steps: [restoredStep],
    leftovers: [],
};

test("browser preview lists no history and refuses to restore", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(await listClaudeCodeOperations()).toEqual({ kind: "preview" });
    expect(await planClaudeCodeRestore(recorded.operationId)).toEqual({
        kind: "preview",
    });
    await expect(
        applyClaudeCodeRestore(recorded.operationId, []),
    ).rejects.toMatchObject({
        name: "RestoreRequestError",
        code: "desktop_required",
    });
    expect(invoke).not.toHaveBeenCalled();
});

test("lists the recorded writes with their labels and states", async () => {
    vi.mocked(invoke).mockResolvedValue([recorded]);
    expect(await listClaudeCodeOperations()).toEqual({
        kind: "desktop",
        operations: [recorded],
    });
    expect(invoke).toHaveBeenCalledExactlyOnceWith("claude_code_operations");
});

test("returns a validated preview for a recorded write", async () => {
    vi.mocked(invoke).mockResolvedValue(restorePlan);
    expect(await planClaudeCodeRestore(recorded.operationId)).toEqual({
        kind: "desktop",
        plan: restorePlan,
    });
    expect(invoke).toHaveBeenCalledExactlyOnceWith("plan_claude_code_restore", {
        operationId: recorded.operationId,
    });
});

test("sends the reviewed digests back and returns the outcome", async () => {
    const reviewed = [
        { ordinal: 0, currentDigest: restorePlanStep.currentDigest },
    ];
    vi.mocked(invoke).mockResolvedValue(restoreOutcome);
    expect(
        await applyClaudeCodeRestore(recorded.operationId, reviewed),
    ).toEqual(restoreOutcome);
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
        "apply_claude_code_restore",
        { input: { operationId: recorded.operationId, reviewed } },
    );
});

test("accepts a restore that left a conflicting file alone", async () => {
    const partial: RestoreOutcome = {
        ...restoreOutcome,
        state: "partial",
        steps: [
            restoredStep,
            {
                ordinal: 1,
                targetPath: "/home/test/.claude/other.json",
                action: "restore",
                state: "conflict",
                errorCode: "target_changed",
                leftoverPath: null,
            },
        ],
    };
    vi.mocked(invoke).mockResolvedValue(partial);
    expect(
        await applyClaudeCodeRestore(recorded.operationId, [
            { ordinal: 0, currentDigest: null },
        ]),
    ).toEqual(partial);
});

test("accepts a restore that reports a copy it could not remove", async () => {
    const leftover = "/home/test/backups/1700000000000-1-0/settings.json";
    const withLeftover: RestoreOutcome = {
        ...restoreOutcome,
        steps: [{ ...restoredStep, leftoverPath: leftover }],
        leftovers: [leftover],
    };
    vi.mocked(invoke).mockResolvedValue(withLeftover);
    expect(await applyClaudeCodeRestore(recorded.operationId, [])).toEqual(
        withLeftover,
    );
});

test.each(RESTORE_ERROR_CODES)(
    "returns the restore refusal code %s",
    async (code) => {
        vi.mocked(invoke).mockRejectedValue(code);
        await expect(
            planClaudeCodeRestore(recorded.operationId),
        ).rejects.toMatchObject({
            name: "RestoreRequestError",
            code,
            message: "The configuration restore failed.",
        });
    },
);

test.each(["private-refusal", new Error("private-runtime-error")])(
    "sanitizes unexpected restore failure %s",
    async (error) => {
        vi.mocked(invoke).mockRejectedValue(error);
        await expect(listClaudeCodeOperations()).rejects.toMatchObject({
            code: "operation_failed",
        });
    },
);

const malformedListings: unknown[] = [
    "not-a-list",
    { operations: [recorded] },
    [{ ...recorded, extra: "synthetic" }],
    [{ ...recorded, operationId: "FEDCBA9876543210FEDCBA9876543210" }],
    [{ ...recorded, state: "running" }],
    [{ ...recorded, providerLabel: "Named", providerId: null }],
    [{ ...recorded, updatedAt: 1 }],
    [{ ...recorded, restorable: "yes" }],
    [{ ...recorded, steps: [] }],
    [{ ...recorded, steps: [{ ...recordedStep, extra: true }] }],
    [{ ...recorded, steps: [{ ...recordedStep, ordinal: 1 }] }],
    [
        {
            ...recorded,
            steps: [
                {
                    ...recordedStep,
                    state: "pending",
                    restoreState: "restored",
                },
            ],
        },
    ],
    [
        {
            ...recorded,
            steps: [{ ...recordedStep, restoreState: "running" }],
        },
    ],
    [
        {
            ...recorded,
            steps: [{ ...recordedStep, targetPath: "settings.json" }],
        },
    ],
];

test.each(malformedListings)(
    "rejects malformed or contradictory listing %#",
    async (response) => {
        vi.mocked(invoke).mockResolvedValue(response);
        await expect(listClaudeCodeOperations()).rejects.toMatchObject({
            name: "RestoreRequestError",
            code: "invalid_response",
        });
    },
);

const malformedPreviews: unknown[] = [
    { ...restorePlan, extra: "synthetic" },
    { ...restorePlan, operationId: "not-an-id" },
    { ...restorePlan, state: "running" },
    { ...restorePlan, providerLabel: "Named", providerId: null },
    { ...restorePlan, steps: [] },
    { ...restorePlan, restorable: false },
    { ...restorePlan, steps: [{ ...restorePlanStep, action: "rewrite" }] },
    { ...restorePlan, steps: [{ ...restorePlanStep, action: "conflict" }] },
    {
        ...restorePlan,
        steps: [{ ...restorePlanStep, reason: "target_changed" }],
    },
    {
        // A code the preview cannot produce (only a failed write-back reports
        // it) is not a preview reason.
        ...restorePlan,
        steps: [
            { ...restorePlanStep, action: "blocked", reason: "write_failed" },
        ],
    },
    {
        ...restorePlan,
        steps: [
            {
                ...restorePlanStep,
                action: "blocked",
                reason: "private_refusal",
            },
        ],
    },
    {
        ...restorePlan,
        steps: [{ ...restorePlanStep, action: "remove", appliedDigest: null }],
    },
    { ...restorePlan, steps: [{ ...restorePlanStep, currentDigest: null }] },
    {
        ...restorePlan,
        steps: [{ ...restorePlanStep, currentDigest: "A".repeat(64) }],
    },
    {
        ...restorePlan,
        steps: [
            {
                ...restorePlanStep,
                restoredProviderId: null,
                restoredProviderLabel: "Named",
            },
        ],
    },
];

test.each(malformedPreviews)(
    "rejects malformed or contradictory preview %#",
    async (response) => {
        vi.mocked(invoke).mockResolvedValue(response);
        await expect(
            planClaudeCodeRestore(recorded.operationId),
        ).rejects.toMatchObject({
            name: "RestoreRequestError",
            code: "invalid_response",
        });
    },
);

const malformedOutcomes: unknown[] = [
    { ...restoreOutcome, extra: "synthetic" },
    { ...restoreOutcome, operationId: "0".repeat(31) },
    { ...restoreOutcome, state: "planned" },
    { ...restoreOutcome, state: "applied" },
    { ...restoreOutcome, state: "running" },
    { ...restoreOutcome, steps: [] },
    { ...restoreOutcome, leftovers: "none" },
    {
        ...restoreOutcome,
        steps: [{ ...restoredStep, ordinal: 1 }],
    },
    {
        ...restoreOutcome,
        steps: [
            {
                ...restoredStep,
                state: "restored",
                errorCode: "target_changed",
            },
        ],
    },
    {
        ...restoreOutcome,
        steps: [{ ...restoredStep, state: "conflict", errorCode: null }],
    },
    {
        ...restoreOutcome,
        steps: [{ ...restoredStep, errorCode: "private_refusal" }],
    },
    {
        ...restoreOutcome,
        steps: [
            { ...restoredStep, leftoverPath: "/home/test/backups/left.json" },
        ],
    },
    {
        ...restoreOutcome,
        steps: [
            {
                ...restoredStep,
                leftoverPath: "relative/left.json",
            },
        ],
    },
    {
        ...restoreOutcome,
        steps: [
            { ...restoredStep, leftoverPath: "/home/test/backups/left.json" },
        ],
        leftovers: ["/home/test/backups/other.json"],
    },
];

test.each(malformedOutcomes)(
    "rejects malformed or contradictory outcome %#",
    async (response) => {
        vi.mocked(invoke).mockResolvedValue(response);
        await expect(
            applyClaudeCodeRestore(recorded.operationId, []),
        ).rejects.toMatchObject({
            name: "RestoreRequestError",
            code: "invalid_response",
        });
    },
);
