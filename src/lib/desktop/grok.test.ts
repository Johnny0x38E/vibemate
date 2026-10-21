import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";
import {
    PlanRequestError,
    type ClaudeCodeApplied,
    type PlanErrorCode,
} from "./agents";
import {
    applyGrokSettings,
    planGrokSettings,
    reviewedGrokPlan,
    type GrokPlan,
} from "./grok";

vi.mock(import("@tauri-apps/api/core"));

const id = "0123456789abcdef0123456789abcdef";
const alias = "vibemate-deepseek-deepseek-flash";
const modelId = "deepseek-flash";
const plan: GrokPlan = {
    agent: "grokBuild",
    modelAlias: alias,
    existingAlias: false,
    contextWindow: { source: "notStated", state: "notStated", value: null },
    imageInput: { source: "notStated", state: "notStated", value: null },
    source: {
        providerId: id,
        providerRevision: 2,
        modelId,
        modelContentDigest: "b".repeat(64),
    },
    precedence: [
        { name: "managed_config.toml", state: "absent" },
        { name: "requirements.toml", state: "present" },
    ],
    notices: [
        { code: "credentialFromStore", detail: null },
        { code: "overridesNotChecked", detail: null },
    ],
    file: {
        target: {
            path: "/Users/test/.grok/config.toml",
            exists: true,
            contentDigest: "a".repeat(64),
            symlink: false,
        },
        changes: [
            {
                keyPath: `model.${alias}.model`,
                before: null,
                after: modelId,
                secret: false,
            },
            {
                keyPath: `model.${alias}.base_url`,
                before: null,
                after: "https://api.deepseek.com",
                secret: false,
            },
            {
                keyPath: `model.${alias}.name`,
                before: null,
                after: "My flash",
                secret: false,
            },
            {
                keyPath: `model.${alias}.api_backend`,
                before: null,
                after: "chat_completions",
                secret: false,
            },
            {
                keyPath: `model.${alias}.api_key`,
                before: null,
                after: "••••••••",
                secret: true,
            },
        ],
    },
    restartRequired: true,
};

beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(isTauri).mockReturnValue(true);
});

/** Parse one value through the desktop boundary and report the refusal code. */
async function rejectWith(value: unknown): Promise<PlanErrorCode> {
    vi.mocked(invoke).mockResolvedValue(value);
    const error = await planGrokSettings(id, modelId, null).then(
        () => null,
        (reason: unknown) => reason,
    );
    expect(error).toBeInstanceOf(PlanRequestError);
    return (error as PlanRequestError).code;
}

test("browser preview invents no plan", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(await planGrokSettings(id, modelId, null)).toEqual({
        kind: "preview",
    });
    expect(invoke).not.toHaveBeenCalled();
});

test("accepts the whole preview and asks for the chosen pair", async () => {
    vi.mocked(invoke).mockResolvedValue(plan);
    expect(await planGrokSettings(id, modelId, "/test/grok")).toEqual({
        kind: "desktop",
        plan,
    });
    expect(invoke).toHaveBeenCalledWith("plan_grok_settings", {
        providerId: id,
        modelId,
        entryPath: "/test/grok",
    });
});

test("accepts written, preserved and undisplayed context without changing approval tokens", async () => {
    const written: GrokPlan = {
        ...plan,
        contextWindow: { source: "library", state: "written", value: "128000" },
        file: {
            ...plan.file,
            changes: [
                ...plan.file.changes,
                {
                    keyPath: `model.${alias}.context_window`,
                    before: null,
                    after: "128000",
                    secret: false,
                },
            ],
        },
    };
    const variants: GrokPlan[] = [
        written,
        ...["65536", "9223372036854775807", null].map((value): GrokPlan => ({
            ...plan,
            existingAlias: true,
            contextWindow: { source: "notStated", state: "unchanged", value },
        })),
    ];
    for (const variant of variants) {
        vi.mocked(invoke).mockResolvedValue(variant);
        await expect(planGrokSettings(id, modelId, null)).resolves.toEqual({
            kind: "desktop",
            plan: variant,
        });
        expect(reviewedGrokPlan(variant)).toEqual(reviewedGrokPlan(plan));
    }
});

test("rejects malformed context summaries and disagreements with the planned write", async () => {
    for (const contextWindow of [
        null,
        { source: "catalog", state: "notStated", value: null },
        { source: "notStated", state: "guessed", value: null },
        { source: "notStated", state: "notStated", value: "128000" },
        { source: "library", state: "written", value: "128000" },
        { source: "library", state: "unchanged", value: null },
        { source: "notStated", state: "unchanged", value: null },
        {
            source: "notStated",
            state: "notStated",
            value: null,
            secret: "must-not-display",
        },
        { source: "notStated", state: "unchanged", value: "private-text" },
        { source: "notStated", state: "unchanged", value: 128000 },
    ]) {
        expect(await rejectWith({ ...plan, contextWindow })).toBe(
            "invalid_response",
        );
    }
    const changes = [
        ...plan.file.changes,
        {
            keyPath: `model.${alias}.context_window`,
            before: null,
            after: "128000",
            secret: false,
        },
    ];
    for (const contextWindow of [
        plan.contextWindow,
        { source: "library", state: "written", value: null },
        { source: "library", state: "written", value: "64000" },
        { source: "library", state: "written", value: "-1" },
    ]) {
        expect(
            await rejectWith({
                ...plan,
                contextWindow,
                file: { ...plan.file, changes },
            }),
        ).toBe("invalid_response");
    }
});

test("validates Grok image declaration against the written vision flag", async () => {
    const written: GrokPlan = {
        ...plan,
        imageInput: { source: "catalog", state: "written", value: true },
        file: {
            ...plan.file,
            changes: [
                ...plan.file.changes,
                {
                    keyPath: `model.${alias}.vision`,
                    before: null,
                    after: "true",
                    secret: false,
                },
            ],
        },
    };
    vi.mocked(invoke).mockResolvedValue(written);
    await expect(planGrokSettings(id, modelId, null)).resolves.toEqual({
        kind: "desktop",
        plan: written,
    });
    for (const imageInput of [
        { source: "catalog", state: "written", value: false },
        { source: "notStated", state: "written", value: true },
        { source: "userDeclaration", state: "unchanged", value: true },
        { source: "notStated", state: "notStated", value: false },
        { source: "catalog", state: "notStated", value: null },
        { source: "notStated", state: "unchanged", value: "private" },
        { source: "catalog", state: "written", value: true, extra: true },
    ]) {
        expect(await rejectWith({ ...written, imageInput })).toBe(
            "invalid_response",
        );
    }
    expect(
        await rejectWith({
            ...plan,
            imageInput: {
                source: "notStated",
                state: "notStated",
                value: null,
            },
            file: {
                ...plan.file,
                changes: [
                    ...plan.file.changes,
                    {
                        keyPath: `model.${alias}.vision`,
                        before: null,
                        after: "true",
                        secret: false,
                    },
                ],
            },
        }),
    ).toBe("invalid_response");
});

test("maps each documented refusal to its own code", async () => {
    for (const code of ["agent_version_unverified", "protocol_unsupported"]) {
        vi.mocked(invoke).mockRejectedValue(code);
        await expect(planGrokSettings(id, modelId, null)).rejects.toMatchObject(
            {
                code,
            },
        );
    }
    vi.mocked(invoke).mockRejectedValue("something_new");
    await expect(planGrokSettings(id, modelId, null)).rejects.toMatchObject({
        code: "operation_failed",
    });
});

test("rejects a payload that is not this adapter's contract", async () => {
    expect(await rejectWith({ ...plan, unexpected: true })).toBe(
        "invalid_response",
    );
    expect(await rejectWith({ ...plan, agent: "pi" })).toBe("invalid_response");
    expect(await rejectWith({ ...plan, restartRequired: false })).toBe(
        "invalid_response",
    );
    // An alias is only trusted in the shape the adapter generates.
    expect(
        await rejectWith({ ...plan, modelAlias: "someone-elses-model" }),
    ).toBe("invalid_response");
    expect(await rejectWith({ ...plan, existingAlias: "false" })).toBe(
        "invalid_response",
    );
    // The settings path must be the file this adapter writes.
    expect(
        await rejectWith({
            ...plan,
            file: {
                ...plan.file,
                target: {
                    ...plan.file.target,
                    path: "/Users/test/.grok/other.toml",
                },
            },
        }),
    ).toBe("invalid_response");
});

test("rejects changes outside the fields this adapter owns", async () => {
    const withChange = (keyPath: string, after: string, secret = false) => ({
        ...plan,
        file: {
            ...plan.file,
            changes: [
                ...plan.file.changes,
                { keyPath, before: null, after, secret },
            ],
        },
    });
    // A request parameter or a shared list is not this plan's field to write.
    expect(
        await rejectWith(withChange(`model.${alias}.temperature`, "0.5")),
    ).toBe("invalid_response");
    expect(
        await rejectWith(withChange("models.disabled_models", '["mine"]')),
    ).toBe("invalid_response");
    expect(
        await rejectWith(withChange(`model.other-model.api_key`, "x", true)),
    ).toBe("invalid_response");
    // The default model belongs to the user and Grok's own picker.
    expect(await rejectWith(withChange("models.default", alias))).toBe(
        "invalid_response",
    );
    expect(
        await rejectWith({ ...plan, file: { ...plan.file, changes: [] } }),
    ).toBe("invalid_response");
});

test("rejects a credential that is not masked or is not unique", async () => {
    const replace = (keyPath: string, patch: Record<string, unknown>) => ({
        ...plan,
        file: {
            ...plan.file,
            changes: plan.file.changes.map((entry) =>
                entry.keyPath === keyPath ? { ...entry, ...patch } : entry,
            ),
        },
    });
    // The planned key position never carries the key itself.
    expect(
        await rejectWith(
            replace(`model.${alias}.api_key`, {
                after: "sk-real-value",
                secret: false,
            }),
        ),
    ).toBe("invalid_response");
    // A previous plaintext key is reported as masked, and anything else is a lie.
    expect(
        await rejectWith(
            replace(`model.${alias}.api_key`, { before: "sk-old-value" }),
        ),
    ).toBe("invalid_response");
    expect(
        await rejectWith(
            replace(`model.${alias}.name`, {
                after: "••••••••",
                secret: true,
            }),
        ),
    ).toBe("invalid_response");
    // A plan without the credential position cannot be applied at all.
    expect(
        await rejectWith({
            ...plan,
            file: {
                ...plan.file,
                changes: plan.file.changes.filter(
                    (entry) => entry.keyPath !== `model.${alias}.api_key`,
                ),
            },
        }),
    ).toBe("invalid_response");
});

test("rejects a plan whose values do not describe the chosen pair", async () => {
    const replace = (keyPath: string, after: string) => ({
        ...plan,
        file: {
            ...plan.file,
            changes: plan.file.changes.map((entry) =>
                entry.keyPath === keyPath ? { ...entry, after } : entry,
            ),
        },
    });
    expect(
        await rejectWith(replace(`model.${alias}.model`, "another-model")),
    ).toBe("invalid_response");
    expect(
        await rejectWith({
            ...plan,
            source: { ...plan.source, providerId: "f".repeat(32) },
        }),
    ).toBe("invalid_response");
});

test("rejects malformed precedence layers and notices", async () => {
    expect(await rejectWith({ ...plan, precedence: [] })).toBe(
        "invalid_response",
    );
    expect(
        await rejectWith({
            ...plan,
            precedence: [
                { name: "requirements.toml", state: "present" },
                { name: "requirements.toml", state: "absent" },
            ],
        }),
    ).toBe("invalid_response");
    expect(
        await rejectWith({
            ...plan,
            precedence: [
                { name: "config.toml", state: "present" },
                { name: "requirements.toml", state: "absent" },
            ],
        }),
    ).toBe("invalid_response");
    expect(
        await rejectWith({
            ...plan,
            precedence: [
                { name: "managed_config.toml", state: "readable" },
                { name: "requirements.toml", state: "absent" },
            ],
        }),
    ).toBe("invalid_response");
    // An unknown notice code would reach the user as an untranslated key.
    expect(
        await rejectWith({
            ...plan,
            notices: [{ code: "unknownThing", detail: null }],
        }),
    ).toBe("invalid_response");
    expect(
        await rejectWith({
            ...plan,
            notices: [{ code: "envKeyOverridden" }],
        }),
    ).toBe("invalid_response");
});

test("accepts the notice that reports an env_key takeover", async () => {
    const taken = {
        ...plan,
        existingAlias: true,
        notices: [{ code: "envKeyOverridden" as const, detail: alias }],
    };
    vi.mocked(invoke).mockResolvedValue(taken);
    await expect(planGrokSettings(id, modelId, null)).resolves.toEqual({
        kind: "desktop",
        plan: taken,
    });
});

test("the reviewed tokens name one file, its digest and the alias", () => {
    expect(reviewedGrokPlan(plan)).toEqual({
        modelAlias: alias,
        file: {
            path: "/Users/test/.grok/config.toml",
            contentDigest: "a".repeat(64),
        },
        providerRevision: 2,
        modelContentDigest: "b".repeat(64),
    });
});

test("browser preview refuses to invent an applied Grok write", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    await expect(
        applyGrokSettings(id, modelId, null, reviewedGrokPlan(plan)),
    ).rejects.toMatchObject({ code: "desktop_required" });
    expect(invoke).not.toHaveBeenCalled();
});

/** Acknowledge one write, then report how the boundary judged the answer. */
async function acknowledge(result: unknown): Promise<ClaudeCodeApplied> {
    vi.mocked(invoke).mockResolvedValue(result);
    return applyGrokSettings(id, modelId, null, reviewedGrokPlan(plan));
}

test("accepts a write that replaced exactly the reviewed file", async () => {
    const result = await acknowledge({
        operationId: "c".repeat(32),
        state: "applied",
        steps: [
            {
                ordinal: 0,
                targetPath: "/Users/test/.grok/config.toml",
                state: "applied",
                errorCode: null,
                backupPath: "/private/backups/config.toml",
            },
        ],
        backupPath: "/private/backups/config.toml",
        restartRequired: true,
    });
    expect(result.state).toBe("applied");
    expect(invoke).toHaveBeenCalledWith("apply_grok_settings", {
        input: {
            providerId: id,
            modelId,
            entryPath: null,
            reviewed: reviewedGrokPlan(plan),
        },
    });
});

test("refuses an acknowledgment that does not describe one written file", async () => {
    const step = {
        ordinal: 0,
        targetPath: "/Users/test/.grok/config.toml",
        state: "applied",
        errorCode: null,
        backupPath: null,
    };
    for (const answer of [
        // `applied` must report the file it wrote.
        { state: "applied", steps: [], restartRequired: true },
        // A restart hint only follows a file that now holds the planned key.
        { state: "applied", steps: [step], restartRequired: false },
        // Nothing was written, so no step may claim it was.
        {
            state: "failed",
            steps: [{ ...step, state: "applied" }],
            restartRequired: false,
        },
        // This adapter plans one file, never two.
        {
            state: "applied",
            steps: [
                step,
                {
                    ...step,
                    ordinal: 1,
                    targetPath: "/Users/test/.grok/other.toml",
                },
            ],
            restartRequired: true,
        },
        // A path the preview never named is not this plan's file.
        {
            state: "applied",
            steps: [
                { ...step, targetPath: "/Users/test/.grok/requirements.toml" },
            ],
            restartRequired: true,
        },
    ]) {
        await expect(
            acknowledge({
                operationId: "c".repeat(32),
                backupPath: null,
                ...answer,
            }),
        ).rejects.toMatchObject({ code: "invalid_response" });
    }
});

test("maps a Grok apply refusal to its own translated code", async () => {
    for (const code of ["plan_stale", "credential_missing"]) {
        vi.mocked(invoke).mockRejectedValue(code);
        await expect(
            applyGrokSettings(id, modelId, null, reviewedGrokPlan(plan)),
        ).rejects.toMatchObject({ code });
    }
});
