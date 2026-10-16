import { invoke, isTauri } from "@tauri-apps/api/core";
import { hasExactKeys, isObject } from "./providers";
import {
    APPLY_ERROR_CODES,
    PLAN_ERROR_CODES,
    ApplyRequestError,
    PlanRequestError,
    appliedResult,
    change,
    planSource,
    targetFile,
    type ClaudeCodeApplied,
    type ConfigChange,
    type PlanSource,
    type PlanTargetFile,
} from "./agents";

/** Pi changes two files and binds auth.json as a read-only precedence dependency. */
export interface PiPlan {
    agent: "pi";
    providerKey: string;
    previousProviderKey: string | null;
    files: { target: PlanTargetFile; changes: ConfigChange[] }[];
    authTarget: PlanTargetFile;
    source: PlanSource;
    restartRequired: boolean;
}
/** Only these complete tokens may approve a write. Paths are never chosen here. */
export interface ReviewedPiPlan {
    providerKey: string;
    previousProviderKey: string | null;
    files: { path: string; contentDigest: string | null }[];
    authTarget: { path: string; contentDigest: string | null };
    providerRevision: number;
    modelContentDigest: string;
}

/** Extract every token, including the file whose credential takes precedence. */
export function reviewedPiPlan(plan: PiPlan): ReviewedPiPlan {
    return {
        providerKey: plan.providerKey,
        previousProviderKey: plan.previousProviderKey,
        files: plan.files.map(({ target }) => ({
            path: target.path,
            contentDigest: target.contentDigest,
        })),
        authTarget: {
            path: plan.authTarget.path,
            contentDigest: plan.authTarget.contentDigest,
        },
        providerRevision: plan.source.providerRevision,
        modelContentDigest: plan.source.modelContentDigest,
    };
}
function invalid(): never {
    throw new PlanRequestError("invalid_response");
}

/** Strictly parse the two-file contract before displaying or approving it. */
function parsePlan(
    value: unknown,
    providerId: string,
    modelId: string,
): PiPlan {
    if (
        !isObject(value) ||
        !hasExactKeys(value, [
            "agent",
            "providerKey",
            "previousProviderKey",
            "files",
            "authTarget",
            "source",
            "restartRequired",
        ]) ||
        value["agent"] !== "pi" ||
        value["restartRequired"] !== true ||
        !Array.isArray(value["files"]) ||
        value["files"].length !== 2
    )
        return invalid();
    const providerKey = value["providerKey"];
    const previousProviderKey = value["previousProviderKey"];
    const validKey = (key: unknown): key is string =>
        typeof key === "string" &&
        key.length <= 320 &&
        /^vibemate-[\p{L}\p{N}_-]+$/u.test(key);
    if (
        !validKey(providerKey) ||
        (previousProviderKey !== null &&
            (!validKey(previousProviderKey) ||
                previousProviderKey === providerKey))
    )
        return invalid();
    const files = value["files"].map((entry: unknown) => {
        if (
            !isObject(entry) ||
            !hasExactKeys(entry, ["target", "changes"]) ||
            !Array.isArray(entry["changes"]) ||
            entry["changes"].length === 0 ||
            entry["changes"].length > 32
        )
            return invalid();
        const changes = entry["changes"].map(change);
        if (new Set(changes.map((c) => c.keyPath)).size !== changes.length)
            return invalid();
        return { target: targetFile(entry["target"]), changes };
    });
    const authTarget = targetFile(value["authTarget"]);
    const source = planSource(value["source"]);
    const [models, settings] = files;
    if (models === undefined || settings === undefined) return invalid();
    const parent = models.target.path.replace(/models\.json$/u, "");
    if (
        parent === models.target.path ||
        settings.target.path !== `${parent}settings.json` ||
        authTarget.path !== `${parent}auth.json` ||
        source.providerId !== providerId ||
        source.modelId !== modelId
    )
        return invalid();
    const key = models.changes.find(
        (c) => c.keyPath === `providers[${providerKey}].apiKey`,
    );
    if (
        key === undefined ||
        !key.secret ||
        (key.before !== null && key.before !== "••••••••")
    )
        return invalid();
    const providerPrefix = `providers[${providerKey}]`;
    const modelPrefix = `${providerPrefix}.models[${modelId}]`;
    const allowed = new Set([
        ...["name", "baseUrl", "api", "apiKey", "authHeader"].map(
            (key) => `${providerPrefix}.${key}`,
        ),
        ...[
            "id",
            "name",
            "baseUrl",
            "api",
            "contextWindow",
            "maxTokens",
            "reasoning",
            "input",
            "compat.thinkingFormat",
            "compat.maxTokensField",
        ].map((key) => `${modelPrefix}.${key}`),
    ]);
    if (
        models.changes.some((c) =>
            c.keyPath === "providers"
                ? previousProviderKey === null ||
                  c.before !== previousProviderKey ||
                  c.after !== providerKey ||
                  c.secret
                : !allowed.has(c.keyPath) ||
                  c.secret !== (c.keyPath === `${providerPrefix}.apiKey`),
        ) ||
        settings.changes.some((c) => {
            if (c.secret) return true;
            if (c.keyPath === "defaultProvider" || c.keyPath === "defaultModel")
                return false;
            return (
                previousProviderKey === null ||
                (c.keyPath !== "enabledModels" &&
                    c.keyPath !== "modelThinkingLevels") ||
                c.before !== previousProviderKey ||
                c.after !== providerKey
            );
        }) ||
        !settings.changes.some(
            (c) => c.keyPath === "defaultProvider" && c.after === providerKey,
        ) ||
        !settings.changes.some(
            (c) => c.keyPath === "defaultModel" && c.after === modelId,
        )
    )
        return invalid();
    return {
        agent: "pi",
        providerKey,
        previousProviderKey,
        files,
        authTarget,
        source,
        restartRequired: true,
    };
}

/** Preview without credentials or writes; browser mode invents no plan. */
export async function planPiSettings(
    providerId: string,
    modelId: string,
    entryPath: string | null,
): Promise<{ kind: "preview" } | { kind: "desktop"; plan: PiPlan }> {
    if (!isTauri()) return { kind: "preview" };
    let value: unknown;
    try {
        value = await invoke<unknown>("plan_pi_settings", {
            providerId,
            modelId,
            entryPath,
        });
    } catch (error: unknown) {
        throw new PlanRequestError(
            PLAN_ERROR_CODES.find((c) => c === error) ?? "operation_failed",
        );
    }
    return { kind: "desktop", plan: parsePlan(value, providerId, modelId) };
}

/** Apply only the reviewed pair and reject a malformed/mismatched acknowledgment. */
export async function applyPiSettings(
    providerId: string,
    modelId: string,
    entryPath: string | null,
    reviewed: ReviewedPiPlan,
): Promise<ClaudeCodeApplied> {
    if (!isTauri()) throw new ApplyRequestError("desktop_required");
    let value: unknown;
    try {
        value = await invoke<unknown>("apply_pi_settings", {
            input: { providerId, modelId, entryPath, reviewed },
        });
    } catch (error: unknown) {
        throw new ApplyRequestError(
            APPLY_ERROR_CODES.find((c) => c === error) ?? "operation_failed",
        );
    }
    const result = appliedResult(value);
    if (
        result.steps.length > reviewed.files.length ||
        result.steps.some((s, i) => s.targetPath !== reviewed.files[i]?.path) ||
        (result.state === "applied" && result.steps.length !== 2) ||
        result.restartRequired !== (result.state === "applied")
    )
        throw new ApplyRequestError("invalid_response");
    return result;
}
