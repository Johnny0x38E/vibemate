import { invoke, isTauri } from "@tauri-apps/api/core";
import { hasExactKeys, isObject, isProviderId } from "./providers";

/** Fixed catalog order; detection does not imply configuration deployment support. */
export const AGENT_IDS = [
    "claudeCode",
    "codex",
    "openCode",
    "pi",
    "grokBuild",
    "hermes",
] as const;
/** Stable identity for a read-only detector. */
export type AgentId = (typeof AGENT_IDS)[number];
/** Fixed user files observed for presence only, never returned as contents. */
export const AGENT_CONFIG_FILES: Record<AgentId, readonly string[]> = {
    claudeCode: ["settings.json"],
    codex: ["config.toml", "auth.json"],
    openCode: ["opencode.json", "opencode.jsonc"],
    pi: ["settings.json", "models.json", "mcp.json"],
    grokBuild: ["config.toml", "managed_config.toml", "requirements.toml"],
    hermes: ["config.yaml", ".env", "auth.json"],
};
/** Optional current-window entry paths; Rust never executes any path. */
export interface DiscoveryInput {
    claudePath: string | null;
    codexPath: string | null;
    opencodePath: string | null;
    hermesPath: string | null;
    piPath: string | null;
    grokPath: string | null;
}
/** Filesystem observations, distinct from running or connected state. */
export interface AgentObservation {
    id: AgentId;
    state: "found" | "notFound" | "unavailable";
    source: "manual" | "path" | "userEntry" | "none";
    entryPath: string | null;
    resolvedPath: string | null;
    version: string | null;
    versionEvidence: "packageMetadata" | "filename" | "none";
    capabilityEvidence: "documented" | "unverified";
    configDirectory: string | null;
    directorySource: "default" | "environment" | "unavailable";
    files: {
        name: string;
        state: "present" | "missing" | "unavailable";
    }[];
}
/** Honest preview or six validated observations in fixed catalog order. */
export type DiscoveryResult =
    { kind: "preview" } | { kind: "desktop"; items: AgentObservation[] };
/** Safe error codes; raw runtime errors are never retained. */
export type DiscoveryErrorCode =
    | "invalid_path"
    | "location_unavailable"
    | "operation_failed"
    | "invalid_response";
/** Safe discovery failure for bundled UI translations. */
export class DiscoveryRequestError extends Error {
    readonly code: DiscoveryErrorCode;
    constructor(code: DiscoveryErrorCode) {
        super("Agent discovery failed.");
        this.name = "DiscoveryRequestError";
        this.code = code;
    }
}
function invalid(): never {
    throw new DiscoveryRequestError("invalid_response");
}
function choice<T extends string>(value: unknown, choices: readonly T[]): T {
    for (const item of choices) if (value === item) return item;
    return invalid();
}
/** Absolute, bounded, control-free path text; shared by both validators. */
function isSafePath(value: unknown): value is string {
    return (
        typeof value === "string" &&
        new TextEncoder().encode(value).length <= 4096 &&
        !Array.from(value).some((char) => {
            const point = char.codePointAt(0);
            return (
                point !== undefined &&
                (point < 32 || (point >= 127 && point <= 159))
            );
        }) &&
        /^(\/|[A-Za-z]:[\\/]|\\\\[^\\]+\\)/u.test(value)
    );
}

function path(value: unknown): string | null {
    if (value === null) return null;
    if (!isSafePath(value)) return invalid();
    return value;
}
function observation(value: unknown, id: AgentId): AgentObservation {
    if (
        !isObject(value) ||
        !hasExactKeys(value, [
            "id",
            "state",
            "source",
            "entryPath",
            "resolvedPath",
            "version",
            "versionEvidence",
            "capabilityEvidence",
            "configDirectory",
            "directorySource",
            "files",
        ]) ||
        value["id"] !== id
    )
        return invalid();
    const state = choice(value["state"], ["found", "notFound", "unavailable"]);
    const source = choice(value["source"], [
        "manual",
        "path",
        "userEntry",
        "none",
    ]);
    const entryPath = path(value["entryPath"]);
    const resolvedPath = path(value["resolvedPath"]);
    const configDirectory = path(value["configDirectory"]);
    const directorySource = choice(value["directorySource"], [
        "default",
        "environment",
        "unavailable",
    ]);
    const version = value["version"];
    if (
        version !== null &&
        (typeof version !== "string" ||
            version.length > 64 ||
            !/^\d{1,10}\.\d{1,10}\.\d{1,10}(?:-[A-Za-z0-9.-]+)?$/u.test(
                version,
            ))
    )
        return invalid();
    const versionEvidence = choice(value["versionEvidence"], [
        "packageMetadata",
        "filename",
        "none",
    ]);
    const capabilityEvidence = choice(value["capabilityEvidence"], [
        "documented",
        "unverified",
    ]);
    if (
        (state === "found" && (entryPath === null || resolvedPath === null)) ||
        (state !== "found" && (resolvedPath !== null || version !== null)) ||
        (source === "none" && (state !== "notFound" || entryPath !== null)) ||
        (version === null) !== (versionEvidence === "none") ||
        (versionEvidence === "packageMetadata" && id !== "pi") ||
        (versionEvidence === "filename" && id !== "grokBuild") ||
        (capabilityEvidence === "documented" &&
            (id !== "pi" ||
                version !== "1.1.0" ||
                versionEvidence !== "packageMetadata")) ||
        (directorySource === "unavailable") !== (configDirectory === null)
    )
        return invalid();
    const names = AGENT_CONFIG_FILES[id];
    const rawFiles = value["files"];
    if (!Array.isArray(rawFiles) || rawFiles.length !== names.length)
        return invalid();
    const files = names.map((name, index) => {
        const file: unknown = rawFiles[index];
        if (
            !isObject(file) ||
            !hasExactKeys(file, ["name", "state"]) ||
            file["name"] !== name
        )
            return invalid();
        const fileState = choice(file["state"], [
            "present",
            "missing",
            "unavailable",
        ]);
        if (configDirectory === null && fileState !== "unavailable")
            return invalid();
        return { name, state: fileState };
    });
    return {
        id,
        state,
        source,
        entryPath,
        resolvedPath,
        version,
        versionEvidence,
        capabilityEvidence,
        configDirectory,
        directorySource,
        files,
    };
}

/**
 * Observe Agent entries only on desktop. Browser preview does not issue IPC or
 * invent installations. Runtime validation discards unexpected fields, unsafe
 * paths and contradictory evidence; Rust remains authoritative for input checks.
 */
export async function discoverAgents(
    input: DiscoveryInput,
): Promise<DiscoveryResult> {
    if (!isTauri()) return { kind: "preview" };
    let value: unknown;
    try {
        value = await invoke<unknown>("discover_agents", { input });
    } catch (error: unknown) {
        throw new DiscoveryRequestError(
            error === "invalid_path" || error === "location_unavailable"
                ? error
                : "operation_failed",
        );
    }
    if (!Array.isArray(value) || value.length !== AGENT_IDS.length)
        return invalid();
    return {
        kind: "desktop",
        items: AGENT_IDS.map((id, index) => observation(value[index], id)),
    };
}

/**
 * Credential placeholder Rust substitutes for every secret value. The plan
 * never carries a real credential; a mismatch here means the two sides disagree
 * about the contract and the plan is rejected instead of rendered.
 */
const SECRET_PLACEHOLDER = "••••••••";
/** Longest path, value or notice text the interface will render. */
const MAX_TEXT_BYTES = 4096;
/** One key-level change in the target file, already sanitized by Rust. */
export interface ConfigChange {
    keyPath: string;
    before: string | null;
    after: string;
    secret: boolean;
}
/** Settings file a plan would change, with its change token. */
export interface PlanTargetFile {
    path: string;
    exists: boolean;
    contentDigest: string | null;
    symlink: boolean;
}
/** Provider and model revisions the plan was built from. */
export interface PlanSource {
    providerId: string;
    providerRevision: number;
    modelId: string;
    modelContentDigest: string;
}
/** Notice kinds Rust attaches to a plan; the interface owns the wording. */
export const PLAN_NOTICE_CODES = [
    "credentialFromStore",
    "overridesNotChecked",
    "modelKeyOverridden",
    "providerLimited",
] as const;
export type PlanNoticeCode = (typeof PLAN_NOTICE_CODES)[number];
/** Something the user must know before applying a plan. */
export interface PlanNotice {
    code: PlanNoticeCode;
    detail: string | null;
}
/** Read-only Claude Code configuration plan; applying it is a later task. */
export interface ClaudeCodePlan {
    agent: "claudeCode";
    target: PlanTargetFile;
    source: PlanSource;
    changes: ConfigChange[];
    notices: PlanNotice[];
    restartRequired: boolean;
}
/** Safe codes emitted by Rust's `PlanError`. */
export const PLAN_ERROR_CODES = [
    "invalid_request",
    "gateway_unverified",
    "base_url_unverified",
    "target_path_invalid",
    "target_unreadable",
    "target_too_large",
    "target_conflict",
    "storage_unavailable",
    "provider_not_found",
    "model_not_found",
    "location_unavailable",
    "operation_failed",
] as const;
/** Rust failures plus errors detected at this IPC boundary. */
export type PlanErrorCode =
    (typeof PLAN_ERROR_CODES)[number] | "invalid_response";
/** A sanitized refusal; the plan behind it never reaches the interface. */
export class PlanRequestError extends Error {
    readonly code: PlanErrorCode;

    /** Construct a safe error without keeping its original payload or cause. */
    constructor(code: PlanErrorCode) {
        super("Agent configuration preview failed.");
        this.name = "PlanRequestError";
        this.code = code;
    }
}
/** Honest browser preview or one validated read-only plan. */
export type ClaudeCodePlanResult =
    { kind: "preview" } | { kind: "desktop"; plan: ClaudeCodePlan };

/** Reject a plan payload; the plan error type stays separate from discovery's. */
function planInvalid(): never {
    throw new PlanRequestError("invalid_response");
}

function boundedText(value: unknown, max = MAX_TEXT_BYTES): string {
    if (
        typeof value !== "string" ||
        new TextEncoder().encode(value).length > max ||
        Array.from(value).some((char) => {
            const point = char.codePointAt(0);
            return (
                point !== undefined &&
                ((point < 32 && point !== 9 && point !== 10) ||
                    (point >= 127 && point <= 159))
            );
        })
    )
        return planInvalid();
    return value;
}

function digest(value: unknown): string {
    if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value))
        return planInvalid();
    return value;
}

/** Model IDs match Rust's rules: bounded, no whitespace and no control characters. */
function modelId(value: unknown): string {
    if (
        typeof value !== "string" ||
        Array.from(value).length === 0 ||
        Array.from(value).length > 256 ||
        /\s/u.test(value)
    )
        return planInvalid();
    return boundedText(value, 1024);
}

function change(value: unknown): ConfigChange {
    if (
        !isObject(value) ||
        !hasExactKeys(value, ["keyPath", "before", "after", "secret"])
    )
        return planInvalid();
    const keyPath = boundedText(value["keyPath"], 256);
    if (keyPath.length === 0 || /\s/u.test(keyPath)) return planInvalid();
    const before =
        value["before"] === null ? null : boundedText(value["before"]);
    const after = boundedText(value["after"]);
    const secret = value["secret"];
    if (typeof secret !== "boolean") return planInvalid();
    if (secret !== (after === SECRET_PLACEHOLDER)) return planInvalid();
    return { keyPath, before, after, secret };
}

function notice(value: unknown): PlanNotice {
    if (!isObject(value) || !hasExactKeys(value, ["code", "detail"]))
        return planInvalid();
    const code = PLAN_NOTICE_CODES.find((known) => known === value["code"]);
    if (code === undefined) return planInvalid();
    const detail =
        value["detail"] === null ? null : boundedText(value["detail"], 256);
    return { code, detail };
}

function targetFile(value: unknown): PlanTargetFile {
    if (
        !isObject(value) ||
        !hasExactKeys(value, ["path", "exists", "contentDigest", "symlink"])
    )
        return planInvalid();
    if (!isSafePath(value["path"])) return planInvalid();
    const targetPath = value["path"];
    const exists = value["exists"];
    const symlink = value["symlink"];
    if (typeof exists !== "boolean" || typeof symlink !== "boolean")
        return planInvalid();
    const contentDigest =
        value["contentDigest"] === null ? null : digest(value["contentDigest"]);
    // Rust reports a digest exactly when the file exists on disk.
    if (exists !== (contentDigest !== null)) return planInvalid();
    return { path: targetPath, exists, contentDigest, symlink };
}

function planSource(value: unknown): PlanSource {
    if (
        !isObject(value) ||
        !hasExactKeys(value, [
            "providerId",
            "providerRevision",
            "modelId",
            "modelContentDigest",
        ])
    )
        return planInvalid();
    const providerRevision = value["providerRevision"];
    if (
        !isProviderId(value["providerId"]) ||
        typeof providerRevision !== "number" ||
        !Number.isSafeInteger(providerRevision) ||
        providerRevision < 1
    )
        return planInvalid();
    return {
        providerId: value["providerId"],
        providerRevision,
        modelId: modelId(value["modelId"]),
        modelContentDigest: digest(value["modelContentDigest"]),
    };
}

function claudeCodePlan(value: unknown): ClaudeCodePlan {
    if (
        !isObject(value) ||
        !hasExactKeys(value, [
            "agent",
            "target",
            "source",
            "changes",
            "notices",
            "restartRequired",
        ]) ||
        value["agent"] !== "claudeCode" ||
        typeof value["restartRequired"] !== "boolean"
    )
        return planInvalid();
    const changes = value["changes"];
    const notices = value["notices"];
    if (!Array.isArray(changes) || changes.length === 0) return planInvalid();
    if (!Array.isArray(notices)) return planInvalid();
    return {
        agent: "claudeCode",
        target: targetFile(value["target"]),
        source: planSource(value["source"]),
        changes: changes.map(change),
        notices: notices.map(notice),
        restartRequired: value["restartRequired"],
    };
}

/**
 * Preview the Claude Code settings a Provider/model link would write.
 *
 * Rust reads the settings file and returns a sanitized plan: nothing is written,
 * no credential value crosses this boundary and no request is sent. Browser
 * preview does not issue IPC and does not invent a plan.
 */
export async function planClaudeCodeSettings(
    providerId: string,
    modelId: string,
): Promise<ClaudeCodePlanResult> {
    if (!isTauri()) return { kind: "preview" };
    let value: unknown;
    try {
        value = await invoke<unknown>("plan_claude_code_settings", {
            providerId,
            modelId,
        });
    } catch (error: unknown) {
        throw new PlanRequestError(
            PLAN_ERROR_CODES.find((code) => code === error) ??
                "operation_failed",
        );
    }
    return { kind: "desktop", plan: claudeCodePlan(value) };
}

/** What one file of an apply step ended up as. */
export const APPLY_STEP_STATES = ["pending", "applied", "failed"] as const;
export type ApplyStepState = (typeof APPLY_STEP_STATES)[number];
/** State of a whole apply step; the interface explains what each one means. */
export const APPLY_STATES = [
    "planned",
    "applied",
    "failed",
    "partial",
    "unknown",
    "compensated",
] as const;
export type ApplyState = (typeof APPLY_STATES)[number];
/** Write-layer codes a step can carry, mirroring Rust's `WriteError`. */
export const WRITE_ERROR_CODES = [
    "invalid_request",
    "target_changed",
    "symlink_refused",
    "target_read_only",
    "target_path_invalid",
    "target_unreadable",
    "target_too_large",
    "backup_unavailable",
    "write_failed",
    "outcome_unknown",
    "storage_unavailable",
    "log_write_failed",
    "log_read_failed",
    "invalid_stored_operation",
] as const;
export type WriteErrorCode = (typeof WRITE_ERROR_CODES)[number];
/** Safe codes emitted by Rust's apply step. */
export const APPLY_ERROR_CODES = [
    "plan_stale",
    "credential_missing",
    "credential_unavailable",
    "invalid_request",
    "provider_not_found",
    "model_not_found",
    "gateway_unverified",
    "base_url_unverified",
    "target_unreadable",
    "target_conflict",
    "location_unavailable",
    "storage_unavailable",
    "operation_failed",
] as const;
/** Apply failures plus errors detected at this IPC boundary. */
export type ApplyErrorCode =
    | (typeof APPLY_ERROR_CODES)[number]
    | "desktop_required"
    | "invalid_response";
/** Safe apply failure; the write outcome behind it never reaches the interface. */
export class ApplyRequestError extends Error {
    readonly code: ApplyErrorCode;

    /** Construct a safe error without keeping its original payload or cause. */
    constructor(code: ApplyErrorCode) {
        super("Applying the Agent configuration failed.");
        this.name = "ApplyRequestError";
        this.code = code;
    }
}
/** Tokens the interface received with the plan the user reviewed. */
export interface ReviewedPlanTokens {
    targetPath: string;
    targetDigest: string | null;
    providerRevision: number;
    modelContentDigest: string;
}
/** What one file of a settled apply step ended up as. */
export interface ApplyStep {
    ordinal: number;
    targetPath: string;
    state: ApplyStepState;
    errorCode: WriteErrorCode | null;
    backupPath: string | null;
}
/** Result of applying a reviewed plan; the real key never crosses this boundary. */
export interface ClaudeCodeApplied {
    operationId: string;
    state: ApplyState;
    steps: ApplyStep[];
    backupPath: string | null;
    restartRequired: boolean;
}

/** Reject an apply payload; the error type stays separate from the preview's. */
function applyInvalid(): never {
    throw new ApplyRequestError("invalid_response");
}

function applyStep(value: unknown, ordinal: number): ApplyStep {
    if (
        !isObject(value) ||
        !hasExactKeys(value, [
            "ordinal",
            "targetPath",
            "state",
            "errorCode",
            "backupPath",
        ]) ||
        value["ordinal"] !== ordinal
    )
        return applyInvalid();
    const targetPath = value["targetPath"];
    if (!isSafePath(targetPath)) return applyInvalid();
    const state = APPLY_STEP_STATES.find((known) => known === value["state"]);
    if (state === undefined) return applyInvalid();
    const errorCode =
        value["errorCode"] === null
            ? null
            : WRITE_ERROR_CODES.find((known) => known === value["errorCode"]);
    if (errorCode === undefined) return applyInvalid();
    // A failed step explains itself; a step that applied or is still pending has
    // no failure to report.
    if ((state === "failed") !== (errorCode !== null)) return applyInvalid();
    const backupPath = value["backupPath"];
    if (backupPath !== null && !isSafePath(backupPath)) return applyInvalid();
    return { ordinal, targetPath, state, errorCode, backupPath };
}

function appliedResult(value: unknown): ClaudeCodeApplied {
    if (
        !isObject(value) ||
        !hasExactKeys(value, [
            "operationId",
            "state",
            "steps",
            "backupPath",
            "restartRequired",
        ]) ||
        typeof value["restartRequired"] !== "boolean"
    )
        return applyInvalid();
    const operationId = value["operationId"];
    if (typeof operationId !== "string" || !/^[0-9a-f]{32}$/u.test(operationId))
        return applyInvalid();
    const state = APPLY_STATES.find((known) => known === value["state"]);
    // An apply call always settles its operation; `planned` only exists in the log.
    if (state === undefined || state === "planned") return applyInvalid();
    const rawSteps = value["steps"];
    if (!Array.isArray(rawSteps) || rawSteps.length === 0)
        return applyInvalid();
    const steps = rawSteps.map((entry: unknown, ordinal: number) =>
        applyStep(entry, ordinal),
    );
    const backupPath = value["backupPath"];
    if (backupPath !== null && !isSafePath(backupPath)) return applyInvalid();
    // Cross-field checks: a backup the interface is told about belongs to one of
    // the steps, a settled success means every attempted file applied, and a
    // plain failure means none did.
    if (
        (backupPath !== null &&
            !steps.some((step) => step.backupPath === backupPath)) ||
        (state === "applied" &&
            steps.some((step) => step.state !== "applied")) ||
        (state === "failed" && steps.some((step) => step.state === "applied"))
    )
        return applyInvalid();
    return {
        operationId,
        state,
        steps,
        backupPath,
        restartRequired: value["restartRequired"],
    };
}

/** Build the tokens an apply call must send back from a validated plan. */
export function reviewedPlanTokens(plan: ClaudeCodePlan): ReviewedPlanTokens {
    return {
        targetPath: plan.target.path,
        targetDigest: plan.target.contentDigest,
        providerRevision: plan.source.providerRevision,
        modelContentDigest: plan.source.modelContentDigest,
    };
}

/**
 * Apply a reviewed Claude Code plan.
 *
 * Rust checks the tokens again, reads the Provider key from OS credential
 * storage and replaces the settings file, keeping a private copy of the previous
 * content. Browser preview refuses: it can neither write a file nor reach the
 * credential store, and it never claims a plan was applied. A failure that the
 * write layer turned into a state (for example a stale file) is a successful
 * call with that state, not a thrown error.
 */
export async function applyClaudeCodeSettings(
    providerId: string,
    modelId: string,
    reviewed: ReviewedPlanTokens,
): Promise<ClaudeCodeApplied> {
    if (!isTauri()) throw new ApplyRequestError("desktop_required");
    let value: unknown;
    try {
        value = await invoke<unknown>("apply_claude_code_settings", {
            input: { providerId, modelId, reviewed },
        });
    } catch (error: unknown) {
        throw new ApplyRequestError(
            APPLY_ERROR_CODES.find((code) => code === error) ??
                "operation_failed",
        );
    }
    return appliedResult(value);
}
