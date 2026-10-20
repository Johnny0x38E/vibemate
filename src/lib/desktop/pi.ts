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

/**
 * Pi's thinking-level tokens, as its settings and model schemas spell them.
 * `xhigh` and `max` are valid tokens; a model accepts them only with a Pi mapping.
 */
export const PI_THINKING_LEVELS = [
    "off",
    "minimal",
    "low",
    "medium",
    "high",
    "xhigh",
    "max",
] as const;
/** One Pi thinking level. */
export type PiThinkingLevel = (typeof PI_THINKING_LEVELS)[number];

/** The setting a model fact describes; Rust builds one row for each, in this order. */
export const PI_FACT_ITEMS = [
    "contextWindow",
    "maxOutputTokens",
    "reasoning",
    "imageInput",
    "thinkingLevel",
] as const;
/** One fact item. */
export type PiFactItem = (typeof PI_FACT_ITEMS)[number];
/** Where a central value came from; `notStated` means there is no central value. */
export const PI_FACT_SOURCES = [
    "library",
    "catalog",
    "userDeclaration",
    "choice",
    "notStated",
] as const;
/** One fact source. */
export type PiFactSource = (typeof PI_FACT_SOURCES)[number];
/** What the plan does with the value: write it, keep the native one, or leave Pi's default. */
export const PI_FACT_STATES = ["written", "unchanged", "notStated"] as const;
/** One fact state. */
export type PiFactState = (typeof PI_FACT_STATES)[number];
/** The native field each item maps to. Rust sends exactly these spellings. */
const PI_FACT_TARGETS = {
    contextWindow: "contextWindow",
    maxOutputTokens: "maxTokens",
    reasoning: "reasoning",
    imageInput: "input",
    thinkingLevel: "modelThinkingLevels",
} as const satisfies Record<PiFactItem, string>;
/** Longest display value a fact row may carry. */
const MAX_FACT_VALUE_CHARS = 256;

/** One row describing a Pi setting: its central source and what this plan does with it. */
export interface PiModelFact {
    item: PiFactItem;
    source: PiFactSource;
    state: PiFactState;
    target: (typeof PI_FACT_TARGETS)[PiFactItem];
    /** The value written or kept, as display text; `null` when nothing is stated. */
    value: string | null;
}

/** Pi changes two files and binds auth.json as a read-only precedence dependency. */
export interface PiPlan {
    agent: "pi";
    providerKey: string;
    previousProviderKey: string | null;
    files: { target: PlanTargetFile; changes: ConfigChange[] }[];
    authTarget: PlanTargetFile;
    source: PlanSource;
    restartRequired: boolean;
    /** The saved per-model level this plan writes, or `null` to keep the saved value. */
    selectedThinkingLevel: PiThinkingLevel | null;
    /** Where each model fact came from and what this plan does with it. */
    modelFacts: PiModelFact[];
}
/** Only these complete tokens may approve a write. Paths are never chosen here. */
export interface ReviewedPiPlan {
    providerKey: string;
    previousProviderKey: string | null;
    files: { path: string; contentDigest: string | null }[];
    authTarget: { path: string; contentDigest: string | null };
    providerRevision: number;
    modelContentDigest: string;
    selectedThinkingLevel: PiThinkingLevel | null;
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
        selectedThinkingLevel: plan.selectedThinkingLevel,
    };
}
function invalid(): never {
    throw new PlanRequestError("invalid_response");
}

/** A known Pi level, or `null` for "do not overwrite"; anything else is a contract break. */
function thinkingLevel(value: unknown): PiThinkingLevel | null {
    if (value === null) return null;
    const level = PI_THINKING_LEVELS.find((known) => known === value);
    if (level === undefined) return invalid();
    return level;
}

/**
 * Strictly parse the five fact rows. Each row must carry the native target its
 * item maps to, and its value, source and state must agree with one another.
 */
function modelFacts(value: unknown): PiModelFact[] {
    if (!Array.isArray(value) || value.length !== PI_FACT_ITEMS.length)
        return invalid();
    const facts = value.map((entry: unknown): PiModelFact => {
        if (
            !isObject(entry) ||
            !hasExactKeys(entry, ["item", "source", "state", "target", "value"])
        )
            return invalid();
        const item = PI_FACT_ITEMS.find((known) => known === entry["item"]);
        const source = PI_FACT_SOURCES.find(
            (known) => known === entry["source"],
        );
        const state = PI_FACT_STATES.find((known) => known === entry["state"]);
        if (item === undefined || source === undefined || state === undefined)
            return invalid();
        if (entry["target"] !== PI_FACT_TARGETS[item]) return invalid();
        const text = entry["value"];
        if (text !== null && typeof text !== "string") return invalid();
        if (text !== null && text.length > MAX_FACT_VALUE_CHARS)
            return invalid();
        // A stated item has a value; an unstated one has none; a written value has a source.
        if ((state === "notStated") !== (text === null)) return invalid();
        if (state === "written" && source === "notStated") return invalid();
        return {
            item,
            source,
            state,
            target: PI_FACT_TARGETS[item],
            value: text,
        };
    });
    if (facts.some((fact, index) => fact.item !== PI_FACT_ITEMS[index]))
        return invalid();
    return facts;
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
            "selectedThinkingLevel",
            "modelFacts",
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
    const selectedThinkingLevel = thinkingLevel(value["selectedThinkingLevel"]);
    const facts = modelFacts(value["modelFacts"]);
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
    // The saved level is a per-model settings key. It exists exactly when the
    // user chose a level, and it must carry that same level.
    const thinkingKey = `modelThinkingLevels.${providerKey}/${modelId}`;
    const thinkingChanges = settings.changes.filter(
        (c) => c.keyPath === thinkingKey,
    );
    const [thinkingChange] = thinkingChanges;
    if (
        selectedThinkingLevel === null
            ? thinkingChanges.length !== 0
            : thinkingChanges.length !== 1 ||
              thinkingChange === undefined ||
              thinkingChange.secret ||
              thinkingChange.after !== selectedThinkingLevel
    )
        return invalid();
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
            // Checked by the per-model rule below; never a rename entry.
            if (c.keyPath === thinkingKey) return false;
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
        selectedThinkingLevel,
        modelFacts: facts,
    };
}

/**
 * Preview without credentials or writes; browser mode invents no plan.
 * `thinkingLevel` is the explicit choice, or `null` to leave the saved level alone.
 */
export async function planPiSettings(
    providerId: string,
    modelId: string,
    entryPath: string | null,
    thinkingLevel: PiThinkingLevel | null,
): Promise<{ kind: "preview" } | { kind: "desktop"; plan: PiPlan }> {
    if (!isTauri()) return { kind: "preview" };
    let value: unknown;
    try {
        value = await invoke<unknown>("plan_pi_settings", {
            providerId,
            modelId,
            entryPath,
            thinkingLevel,
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
