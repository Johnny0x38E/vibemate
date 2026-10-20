import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";
import { piModelFactsFixture } from "../../test/piFacts";
import {
    planPiSettings,
    applyPiSettings,
    reviewedPiPlan,
    type PiModelFact,
    type PiPlan,
} from "./pi";
vi.mock(import("@tauri-apps/api/core"));
const id = "0123456789abcdef0123456789abcdef";
const target = (file: string) => ({
    path: `/test/pi/${file}`,
    exists: true,
    contentDigest: "a".repeat(64),
    symlink: false,
});
const plan: PiPlan = {
    agent: "pi",
    providerKey: "vibemate-DeepSeek",
    previousProviderKey: null,
    source: {
        providerId: id,
        providerRevision: 2,
        modelId: "my/model",
        modelContentDigest: "b".repeat(64),
    },
    files: [
        {
            target: target("models.json"),
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
            target: target("settings.json"),
            changes: [
                {
                    keyPath: "defaultProvider",
                    before: "mine",
                    after: "vibemate-DeepSeek",
                    secret: false,
                },
                {
                    keyPath: "defaultModel",
                    before: "old",
                    after: "my/model",
                    secret: false,
                },
            ],
        },
    ],
    authTarget: target("auth.json"),
    restartRequired: true,
    selectedThinkingLevel: null,
    modelFacts: piModelFactsFixture,
};
beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(isTauri).mockReturnValue(true);
});

test("browser preview never invokes native commands or invents an applied result", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(await planPiSettings(id, "my/model", null, null)).toEqual({
        kind: "preview",
    });
    await expect(
        applyPiSettings(id, "my/model", null, reviewedPiPlan(plan)),
    ).rejects.toMatchObject({ code: "desktop_required" });
    expect(invoke).not.toHaveBeenCalled();
});
test("validates the whole preview and returns every reviewed token unchanged", async () => {
    vi.mocked(invoke).mockResolvedValue(plan);
    const result = await planPiSettings(id, "my/model", "/test/pi-entry", null);
    expect(result).toEqual({ kind: "desktop", plan });
    expect(invoke).toHaveBeenCalledWith("plan_pi_settings", {
        providerId: id,
        modelId: "my/model",
        entryPath: "/test/pi-entry",
        thinkingLevel: null,
    });
    expect(reviewedPiPlan(plan).files).toHaveLength(2);
    expect(reviewedPiPlan(plan).authTarget.path).toBe("/test/pi/auth.json");
});
test("rejects leaked old credentials, extra fields and mismatched file or source identities", async () => {
    const mutations: ((p: PiPlan) => void)[] = [
        (p) => {
            const c = p.files[0]?.changes[0];
            if (c) c.before = "private-old-key";
        },
        (p) => {
            const f = p.files[1];
            if (f) f.target.path = "/another/settings.json";
        },
        (p) => {
            p.source.providerId = "f".repeat(32);
        },
        (p) => {
            p.files.pop();
        },
        (p) => {
            p.files[0]?.changes.push({
                keyPath: "arbitrary",
                before: null,
                after: "private",
                secret: false,
            });
        },
    ];
    for (const mutate of mutations) {
        const changed = structuredClone(plan);
        mutate(changed);
        vi.mocked(invoke).mockResolvedValue(changed);
        await expect(
            planPiSettings(id, "my/model", null, null),
        ).rejects.toMatchObject({ code: "invalid_response" });
    }
    vi.mocked(invoke).mockResolvedValue({ ...plan, secret: "private" });
    await expect(
        planPiSettings(id, "my/model", null, null),
    ).rejects.toMatchObject({
        code: "invalid_response",
    });
});
test("apply sends all tokens and checks acknowledgments against the reviewed paths", async () => {
    const reviewed = reviewedPiPlan(plan);
    const result = {
        operationId: "c".repeat(32),
        state: "applied",
        backupPath: "/private/models.json",
        restartRequired: true,
        steps: plan.files.map((f, ordinal) => ({
            ordinal,
            targetPath: f.target.path,
            state: "applied",
            errorCode: null,
            backupPath:
                ordinal === 0
                    ? "/private/models.json"
                    : "/private/settings.json",
        })),
    };
    vi.mocked(invoke).mockResolvedValue(result);
    expect(await applyPiSettings(id, "my/model", null, reviewed)).toEqual(
        result,
    );
    expect(invoke).toHaveBeenCalledWith("apply_pi_settings", {
        input: {
            providerId: id,
            modelId: "my/model",
            entryPath: null,
            reviewed,
        },
    });
    const wrong = structuredClone(result);
    if (wrong.steps[1]) wrong.steps[1].targetPath = "/elsewhere/settings.json";
    vi.mocked(invoke).mockResolvedValue(wrong);
    await expect(
        applyPiSettings(id, "my/model", null, reviewed),
    ).rejects.toMatchObject({ code: "invalid_response" });
});
test("preserves stable native failures and discards raw runtime diagnostics", async () => {
    vi.mocked(invoke).mockRejectedValue("credential_conflict");
    await expect(
        planPiSettings(id, "my/model", null, null),
    ).rejects.toMatchObject({
        code: "credential_conflict",
    });
    vi.mocked(invoke).mockRejectedValue(new Error("private details"));
    await expect(
        applyPiSettings(id, "my/model", null, reviewedPiPlan(plan)),
    ).rejects.toMatchObject({ code: "operation_failed" });
});

test("accepts a reviewed readable-name migration without changing central identities", async () => {
    const migrated = structuredClone(plan);
    const old = `vibemate-${id}`;
    migrated.previousProviderKey = old;
    migrated.files[0]?.changes.push({
        keyPath: "providers",
        before: old,
        after: migrated.providerKey,
        secret: false,
    });
    migrated.files[1]?.changes.push({
        keyPath: "modelThinkingLevels",
        before: old,
        after: migrated.providerKey,
        secret: false,
    });
    vi.mocked(invoke).mockResolvedValue(migrated);
    expect(await planPiSettings(id, "my/model", null, null)).toEqual({
        kind: "desktop",
        plan: migrated,
    });
    expect(reviewedPiPlan(migrated)).toMatchObject({
        providerKey: "vibemate-DeepSeek",
        previousProviderKey: old,
    });
    const wrong = structuredClone(migrated);
    const rename = wrong.files[0]?.changes.find(
        (c) => c.keyPath === "providers",
    );
    if (rename) rename.before = "vibemate-unowned";
    vi.mocked(invoke).mockResolvedValue(wrong);
    await expect(
        planPiSettings(id, "my/model", null, null),
    ).rejects.toMatchObject({
        code: "invalid_response",
    });
});

test("a chosen thinking level must come with exactly one matching per-model settings change", async () => {
    const levelKey = "modelThinkingLevels.vibemate-DeepSeek/my/model";
    const chosen = structuredClone(plan);
    chosen.selectedThinkingLevel = "high";
    chosen.files[1]?.changes.push({
        keyPath: levelKey,
        before: "low",
        after: "high",
        secret: false,
    });
    vi.mocked(invoke).mockResolvedValue(chosen);
    expect(await planPiSettings(id, "my/model", null, "high")).toEqual({
        kind: "desktop",
        plan: chosen,
    });
    expect(reviewedPiPlan(chosen).selectedThinkingLevel).toBe("high");
    expect(invoke).toHaveBeenLastCalledWith("plan_pi_settings", {
        providerId: id,
        modelId: "my/model",
        entryPath: null,
        thinkingLevel: "high",
    });

    // The level change is missing, or names a different level than the token.
    const missing = structuredClone(chosen);
    if (missing.files[1]) {
        missing.files[1].changes = missing.files[1].changes.filter(
            (c) => c.keyPath !== levelKey,
        );
    }
    const mismatched = structuredClone(chosen);
    const level = mismatched.files[1]?.changes.find(
        (c) => c.keyPath === levelKey,
    );
    if (level) level.after = "low";
    // A level change without a chosen level would silently overwrite a saved value.
    const unexpected = structuredClone(plan);
    unexpected.files[1]?.changes.push({
        keyPath: levelKey,
        before: null,
        after: "low",
        secret: false,
    });
    for (const invalidPlan of [missing, mismatched, unexpected]) {
        vi.mocked(invoke).mockResolvedValue(invalidPlan);
        await expect(
            planPiSettings(id, "my/model", null, "high"),
        ).rejects.toMatchObject({ code: "invalid_response" });
    }

    // Only Pi's own tokens cross the boundary; `none` is the catalog spelling.
    vi.mocked(invoke).mockResolvedValue({
        ...plan,
        selectedThinkingLevel: "none",
    });
    await expect(
        planPiSettings(id, "my/model", null, null),
    ).rejects.toMatchObject({ code: "invalid_response" });
});

test("apply sends the approved thinking level with the reviewed tokens", async () => {
    const reviewed = {
        ...reviewedPiPlan(plan),
        selectedThinkingLevel: "high" as const,
    };
    vi.mocked(invoke).mockResolvedValue({
        operationId: "d".repeat(32),
        state: "applied",
        backupPath: null,
        restartRequired: true,
        steps: plan.files.map((f, ordinal) => ({
            ordinal,
            targetPath: f.target.path,
            state: "applied",
            errorCode: null,
            backupPath: null,
        })),
    });
    await applyPiSettings(id, "my/model", null, reviewed);
    expect(invoke).toHaveBeenCalledWith("apply_pi_settings", {
        input: {
            providerId: id,
            modelId: "my/model",
            entryPath: null,
            reviewed,
        },
    });
});

test("accepts the model facts in Rust's order and refuses every other shape", async () => {
    vi.mocked(invoke).mockResolvedValue(plan);
    expect(await planPiSettings(id, "my/model", null, null)).toEqual({
        kind: "desktop",
        plan,
    });
    // Each mutation breaks one rule: row count or order, a target that does not
    // match its item, a value that disagrees with its state, or an unknown token.
    const mutations: ((facts: PiModelFact[]) => void)[] = [
        (facts) => {
            facts.pop();
        },
        (facts) => {
            facts.reverse();
        },
        (facts) => {
            const row = facts[0];
            if (row) row.target = "input";
        },
        (facts) => {
            const row = facts[0];
            if (row) row.value = null;
        },
        (facts) => {
            const row = facts[3];
            if (row) row.value = "text";
        },
        (facts) => {
            const row = facts[2];
            if (row) row.source = "notStated";
        },
        (facts) => {
            const row = facts[2];
            if (row) row.source = "guessed" as PiModelFact["source"];
        },
        (facts) => {
            const row = facts[1];
            if (row) row.value = "x".repeat(300);
        },
        (facts) => {
            const row = facts[1] as PiModelFact & { extra?: string };
            row.extra = "private";
        },
    ];
    for (const mutate of mutations) {
        const changed = structuredClone(plan);
        mutate(changed.modelFacts);
        vi.mocked(invoke).mockResolvedValue(changed);
        await expect(
            planPiSettings(id, "my/model", null, null),
        ).rejects.toMatchObject({ code: "invalid_response" });
    }
});
