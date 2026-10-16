import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";
import {
    planPiSettings,
    applyPiSettings,
    reviewedPiPlan,
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
};
beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(isTauri).mockReturnValue(true);
});

test("browser preview never invokes native commands or invents an applied result", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(await planPiSettings(id, "my/model", null)).toEqual({
        kind: "preview",
    });
    await expect(
        applyPiSettings(id, "my/model", null, reviewedPiPlan(plan)),
    ).rejects.toMatchObject({ code: "desktop_required" });
    expect(invoke).not.toHaveBeenCalled();
});
test("validates the whole preview and returns every reviewed token unchanged", async () => {
    vi.mocked(invoke).mockResolvedValue(plan);
    const result = await planPiSettings(id, "my/model", "/test/pi-entry");
    expect(result).toEqual({ kind: "desktop", plan });
    expect(invoke).toHaveBeenCalledWith("plan_pi_settings", {
        providerId: id,
        modelId: "my/model",
        entryPath: "/test/pi-entry",
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
            planPiSettings(id, "my/model", null),
        ).rejects.toMatchObject({ code: "invalid_response" });
    }
    vi.mocked(invoke).mockResolvedValue({ ...plan, secret: "private" });
    await expect(planPiSettings(id, "my/model", null)).rejects.toMatchObject({
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
    await expect(planPiSettings(id, "my/model", null)).rejects.toMatchObject({
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
    expect(await planPiSettings(id, "my/model", null)).toEqual({
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
    await expect(planPiSettings(id, "my/model", null)).rejects.toMatchObject({
        code: "invalid_response",
    });
});
