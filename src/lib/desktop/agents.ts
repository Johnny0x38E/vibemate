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
    "envKeyOverridden",
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
    "agent_version_unverified",
    "protocol_unsupported",
    "credential_conflict",
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

/** Validate a shared Agent configuration payload; reject unknown or secret-bearing fields. */
export function change(value: unknown): ConfigChange {
    if (
        !isObject(value) ||
        !hasExactKeys(value, ["keyPath", "before", "after", "secret"])
    )
        return planInvalid();
    const keyPath = boundedText(value["keyPath"], 2048);
    if (keyPath.length === 0 || /\s/u.test(keyPath)) return planInvalid();
    const before =
        value["before"] === null ? null : boundedText(value["before"]);
    const after = boundedText(value["after"]);
    const secret = value["secret"];
    if (typeof secret !== "boolean") return planInvalid();
    if (secret !== (after === SECRET_PLACEHOLDER)) return planInvalid();
    return { keyPath, before, after, secret };
}

/** Validate one plan notice; the interface owns the wording for `code`. */
export function notice(value: unknown): PlanNotice {
    if (!isObject(value) || !hasExactKeys(value, ["code", "detail"]))
        return planInvalid();
    const code = PLAN_NOTICE_CODES.find((known) => known === value["code"]);
    if (code === undefined) return planInvalid();
    const detail =
        value["detail"] === null ? null : boundedText(value["detail"], 256);
    return { code, detail };
}

/** Validate a shared Agent configuration payload; reject unknown or secret-bearing fields. */
export function targetFile(value: unknown): PlanTargetFile {
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

/** Validate a shared Agent configuration payload; reject unknown or secret-bearing fields. */
export function planSource(value: unknown): PlanSource {
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
/**
 * States a settled apply call can report. `planned` exists only in the write log:
 * an apply call always settles its operation before it answers.
 */
export type AppliedState = Exclude<ApplyState, "planned">;
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
    "agent_version_unverified",
    "protocol_unsupported",
    "credential_conflict",
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
    state: AppliedState;
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

/** Validate a shared Agent configuration payload; reject unknown or secret-bearing fields. */
export function appliedResult(value: unknown): ClaudeCodeApplied {
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

/** What a rollback did to one applied step, mirroring Rust's `RestoreState`. */
export const RESTORE_STATES = [
    "restored",
    "conflict",
    "unavailable",
    "failed",
    "unknown",
] as const;
export type RestoreState = (typeof RESTORE_STATES)[number];
/** What a restore would do to one file, mirroring Rust's `RestoreAction`. */
export const RESTORE_ACTIONS = [
    "restore",
    "remove",
    "conflict",
    "blocked",
    "skip",
] as const;
export type RestoreAction = (typeof RESTORE_ACTIONS)[number];
/**
 * Codes a restore preview attaches to a step it will not carry out: the file
 * moved on after the write, cannot be read safely, its stored copy is unusable,
 * or an older write did not record whether the file existed. A write that only
 * fails when the restore is attempted reports its own code in the result, not
 * here.
 */
export const RESTORE_REASON_CODES = [
    "target_changed",
    "target_unreadable",
    "target_too_large",
    "target_path_invalid",
    "symlink_refused",
    "backup_unavailable",
    "restore_history_unknown",
] as const;
export type RestoreReasonCode = (typeof RESTORE_REASON_CODES)[number];
/** Safe codes emitted by the restore commands. */
export const RESTORE_ERROR_CODES = [
    "operation_not_found",
    "plan_stale",
    "invalid_request",
    "storage_unavailable",
    "operation_failed",
] as const;
/** Restore failures plus errors detected at this IPC boundary. */
export type RestoreErrorCode =
    | (typeof RESTORE_ERROR_CODES)[number]
    | "desktop_required"
    | "invalid_response";
/** Safe restore failure; no file content travels with it. */
export class RestoreRequestError extends Error {
    readonly code: RestoreErrorCode;

    /** Construct a safe error without keeping its original payload or cause. */
    constructor(code: RestoreErrorCode) {
        super("The configuration restore failed.");
        this.name = "RestoreRequestError";
        this.code = code;
    }
}
/** One file of a recorded write, without backup paths or file content. */
export interface RecordedOperationStep {
    ordinal: number;
    targetPath: string;
    state: ApplyStepState;
    restoreState: RestoreState | null;
}
/** One recorded Claude Code settings write, as the restore view lists it. */
export interface RecordedOperation {
    operationId: string;
    state: ApplyState;
    providerId: string | null;
    providerLabel: string | null;
    createdAt: number;
    updatedAt: number;
    restorable: boolean;
    steps: RecordedOperationStep[];
}
/** One file of a restore preview. */
export interface RestorePlanStep {
    ordinal: number;
    targetPath: string;
    action: RestoreAction;
    reason: RestoreReasonCode | null;
    currentDigest: string | null;
    appliedDigest: string | null;
    restoreState: RestoreState | null;
    restoredProviderId: string | null;
    restoredProviderLabel: string | null;
}
/** A restore preview: what undoing one recorded write would do. */
export interface ClaudeCodeRestorePlan {
    operationId: string;
    state: ApplyState;
    createdAt: number;
    restorable: boolean;
    providerId: string | null;
    providerLabel: string | null;
    steps: RestorePlanStep[];
}
/** One reviewed step, exactly as the preview showed it. */
export interface ReviewedRestoreStep {
    ordinal: number;
    currentDigest: string | null;
}
/** What one step of a restore did. */
export interface RestoreStepOutcome {
    ordinal: number;
    targetPath: string;
    action: RestoreAction;
    state: RestoreState;
    errorCode: WriteErrorCode | null;
    leftoverPath: string | null;
}
/**
 * States a restore can settle into. A rollback never applies anything, so a
 * completed restore is undone, partly undone, unverified or refused — never
 * `applied`.
 */
export const RESTORE_OUTCOME_STATES = [
    "compensated",
    "partial",
    "unknown",
    "failed",
] as const;
export type RestoreOutcomeState = (typeof RESTORE_OUTCOME_STATES)[number];
/** Result of one confirmed restore, settled before it is answered. */
export interface RestoreOutcome {
    operationId: string;
    state: RestoreOutcomeState;
    steps: RestoreStepOutcome[];
    leftovers: string[];
}
/** Listing result; the browser preview has no history to show. */
export type OperationListResult =
    { kind: "preview" } | { kind: "desktop"; operations: RecordedOperation[] };
/** Preview result; the browser preview neither lists nor plans a restore. */
export type RestorePlanResult =
    { kind: "preview" } | { kind: "desktop"; plan: ClaudeCodeRestorePlan };

/** Reject a restore payload; the error type stays separate from the others. */
function restoreInvalid(): never {
    throw new RestoreRequestError("invalid_response");
}

/** A stored operation identity, in the shape every row uses. */
function operationId(value: unknown): string {
    if (typeof value !== "string" || !/^[0-9a-f]{32}$/u.test(value))
        return restoreInvalid();
    return value;
}

/** A bounded display name, or `null` when a reference has no resolved name. */
function optionalLabel(value: unknown): string | null {
    if (value === null) return null;
    if (typeof value !== "string" || value.length === 0 || value.length > 256)
        return restoreInvalid();
    return value;
}

/** A stored digest or `null`; the log stores nothing else in these fields. */
function optionalDigest(value: unknown): string | null {
    if (value === null) return null;
    if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value))
        return restoreInvalid();
    return value;
}

/** A millisecond timestamp the log can have written. */
function timestamp(value: unknown): number {
    if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0)
        return restoreInvalid();
    return value;
}

function restoreState(value: unknown): RestoreState | null {
    if (value === null) return null;
    const state = RESTORE_STATES.find((known) => known === value);
    return state ?? restoreInvalid();
}

function recordedOperationStep(
    value: unknown,
    ordinal: number,
): RecordedOperationStep {
    if (
        !isObject(value) ||
        !hasExactKeys(value, [
            "ordinal",
            "targetPath",
            "state",
            "restoreState",
        ]) ||
        value["ordinal"] !== ordinal
    )
        return restoreInvalid();
    const targetPath = value["targetPath"];
    if (!isSafePath(targetPath)) return restoreInvalid();
    const state = APPLY_STEP_STATES.find((known) => known === value["state"]);
    if (state === undefined) return restoreInvalid();
    const restore = restoreState(value["restoreState"]);
    // Only a step that wrote something can carry a rollback result.
    if (restore !== null && state !== "applied") return restoreInvalid();
    return { ordinal, targetPath, state, restoreState: restore };
}

function recordedOperation(value: unknown): RecordedOperation {
    if (
        !isObject(value) ||
        !hasExactKeys(value, [
            "operationId",
            "state",
            "providerId",
            "providerLabel",
            "createdAt",
            "updatedAt",
            "restorable",
            "steps",
        ]) ||
        typeof value["restorable"] !== "boolean"
    )
        return restoreInvalid();
    const id = operationId(value["operationId"]);
    const state = APPLY_STATES.find((known) => known === value["state"]);
    if (state === undefined) return restoreInvalid();
    const providerId = optionalLabel(value["providerId"]);
    const providerLabel = optionalLabel(value["providerLabel"]);
    const createdAt = timestamp(value["createdAt"]);
    const updatedAt = timestamp(value["updatedAt"]);
    // A name without the reference it names, or a write that claims to end
    // before it started, cannot come from the log.
    if (
        (providerLabel !== null && providerId === null) ||
        updatedAt < createdAt
    )
        return restoreInvalid();
    const rawSteps = value["steps"];
    if (!Array.isArray(rawSteps) || rawSteps.length === 0)
        return restoreInvalid();
    return {
        operationId: id,
        state,
        providerId,
        providerLabel,
        createdAt,
        updatedAt,
        restorable: value["restorable"],
        steps: rawSteps.map((entry: unknown, index: number) =>
            recordedOperationStep(entry, index),
        ),
    };
}

function restorePlanStep(value: unknown, ordinal: number): RestorePlanStep {
    if (
        !isObject(value) ||
        !hasExactKeys(value, [
            "ordinal",
            "targetPath",
            "action",
            "reason",
            "currentDigest",
            "appliedDigest",
            "restoreState",
            "restoredProviderId",
            "restoredProviderLabel",
        ]) ||
        value["ordinal"] !== ordinal
    )
        return restoreInvalid();
    const targetPath = value["targetPath"];
    if (!isSafePath(targetPath)) return restoreInvalid();
    const action = RESTORE_ACTIONS.find((known) => known === value["action"]);
    if (action === undefined) return restoreInvalid();
    const reason =
        value["reason"] === null
            ? null
            : RESTORE_REASON_CODES.find((known) => known === value["reason"]);
    if (reason === undefined) return restoreInvalid();
    const currentDigest = optionalDigest(value["currentDigest"]);
    const appliedDigest = optionalDigest(value["appliedDigest"]);
    const restore = restoreState(value["restoreState"]);
    const restoredProviderId = optionalLabel(value["restoredProviderId"]);
    const restoredProviderLabel = optionalLabel(value["restoredProviderLabel"]);
    // Cross-field checks: only a step the restore will not carry out carries a
    // reason, writing a copy back needs both digests, removing a created file
    // needs the digest it was left with, and a name needs its reference.
    if (
        (action === "conflict" || action === "blocked") !== (reason !== null) ||
        (action === "restore" &&
            (currentDigest === null || appliedDigest === null)) ||
        (action === "remove" && appliedDigest === null) ||
        (restoredProviderLabel !== null && restoredProviderId === null)
    )
        return restoreInvalid();
    return {
        ordinal,
        targetPath,
        action,
        reason,
        currentDigest,
        appliedDigest,
        restoreState: restore,
        restoredProviderId,
        restoredProviderLabel,
    };
}

function restorePlan(value: unknown): ClaudeCodeRestorePlan {
    if (
        !isObject(value) ||
        !hasExactKeys(value, [
            "operationId",
            "state",
            "createdAt",
            "restorable",
            "providerId",
            "providerLabel",
            "steps",
        ]) ||
        typeof value["restorable"] !== "boolean"
    )
        return restoreInvalid();
    const id = operationId(value["operationId"]);
    const state = APPLY_STATES.find((known) => known === value["state"]);
    if (state === undefined) return restoreInvalid();
    const providerId = optionalLabel(value["providerId"]);
    const providerLabel = optionalLabel(value["providerLabel"]);
    if (providerLabel !== null && providerId === null) return restoreInvalid();
    const rawSteps = value["steps"];
    if (!Array.isArray(rawSteps) || rawSteps.length === 0)
        return restoreInvalid();
    const steps = rawSteps.map((entry: unknown, index: number) =>
        restorePlanStep(entry, index),
    );
    // The flag the interface uses to offer a restore must match the steps it
    // would carry out.
    const changesFiles = steps.some(
        (step) => step.action === "restore" || step.action === "remove",
    );
    if (value["restorable"] !== changesFiles) return restoreInvalid();
    return {
        operationId: id,
        state,
        createdAt: timestamp(value["createdAt"]),
        restorable: value["restorable"],
        providerId,
        providerLabel,
        steps,
    };
}

function restoreStepOutcome(value: unknown): RestoreStepOutcome {
    if (
        !isObject(value) ||
        !hasExactKeys(value, [
            "ordinal",
            "targetPath",
            "action",
            "state",
            "errorCode",
            "leftoverPath",
        ])
    )
        return restoreInvalid();
    const ordinal = value["ordinal"];
    if (
        typeof ordinal !== "number" ||
        !Number.isSafeInteger(ordinal) ||
        ordinal < 0 ||
        ordinal > 0xffffffff
    )
        return restoreInvalid();
    const targetPath = value["targetPath"];
    if (!isSafePath(targetPath)) return restoreInvalid();
    const action = RESTORE_ACTIONS.find((known) => known === value["action"]);
    const state = RESTORE_STATES.find((known) => known === value["state"]);
    if (action === undefined || state === undefined) return restoreInvalid();
    const errorCode =
        value["errorCode"] === null
            ? null
            : WRITE_ERROR_CODES.find((known) => known === value["errorCode"]);
    if (errorCode === undefined) return restoreInvalid();
    // A step that reached its previous content explains nothing; every other
    // result carries the code of what stopped it.
    if ((state === "restored") !== (errorCode === null))
        return restoreInvalid();
    const leftoverPath = value["leftoverPath"];
    if (leftoverPath !== null && !isSafePath(leftoverPath))
        return restoreInvalid();
    return { ordinal, targetPath, action, state, errorCode, leftoverPath };
}

function restoreOutcome(
    value: unknown,
    reviewed: ReviewedRestoreStep[],
): RestoreOutcome {
    if (
        !isObject(value) ||
        !hasExactKeys(value, ["operationId", "state", "steps", "leftovers"])
    )
        return restoreInvalid();
    const id = operationId(value["operationId"]);
    const state = RESTORE_OUTCOME_STATES.find(
        (known) => known === value["state"],
    );
    if (state === undefined) return restoreInvalid();
    const rawSteps = value["steps"];
    if (!Array.isArray(rawSteps) || rawSteps.length === 0)
        return restoreInvalid();
    const steps = rawSteps.map((entry: unknown) => restoreStepOutcome(entry));
    // Native rollback runs in reverse order and can skip settled steps. Match
    // recorded ordinals, not array positions (a two-file result starts at 1).
    if (
        steps.length > reviewed.length ||
        steps.some(
            (step, index) =>
                !reviewed.some((token) => token.ordinal === step.ordinal) ||
                (index > 0 &&
                    (steps[index - 1]?.ordinal ?? -1) <= step.ordinal),
        )
    )
        return restoreInvalid();
    const rawLeftovers = value["leftovers"];
    if (!Array.isArray(rawLeftovers)) return restoreInvalid();
    const leftovers = rawLeftovers.map((entry: unknown) => {
        if (!isSafePath(entry)) return restoreInvalid();
        return entry;
    });
    // The list of copies left behind is exactly the paths the steps report, so a
    // reported leftover cannot be dropped from the summary.
    const reported = steps
        .map((step) => step.leftoverPath)
        .filter((entry): entry is string => entry !== null);
    if (
        reported.length !== leftovers.length ||
        reported.some((entry) => !leftovers.includes(entry))
    )
        return restoreInvalid();
    return { operationId: id, state, steps, leftovers };
}

/**
 * The commands each adapter exposes for its recorded writes and their recovery.
 *
 * An Agent with a preview but no write path is not listed here: the history and
 * restore screens only appear for adapters that can record an operation.
 */
const AGENT_CONFIG_COMMANDS = {
    claudeCode: {
        list: "claude_code_operations",
        plan: "plan_claude_code_restore",
        apply: "apply_claude_code_restore",
    },
    pi: {
        list: "pi_operations",
        plan: "plan_pi_restore",
        apply: "apply_pi_restore",
    },
    grokBuild: {
        list: "grok_operations",
        plan: "plan_grok_restore",
        apply: "apply_grok_restore",
    },
} as const;

/** An Agent whose configuration writes vibemate records and can undo. */
export type ConfigurableAgent = keyof typeof AGENT_CONFIG_COMMANDS;

/**
 * List the settings writes vibemate recorded for one Agent, newest first.
 *
 * Read-only: the listing names files, states and digests, never file content and
 * never where a copy of it sits. Browser preview says so instead of inventing a
 * history.
 */
export async function listAgentOperations(
    agent: ConfigurableAgent,
): Promise<OperationListResult> {
    if (!isTauri()) return { kind: "preview" };
    let value: unknown;
    try {
        value = await invoke<unknown>(AGENT_CONFIG_COMMANDS[agent].list);
    } catch (error: unknown) {
        throw new RestoreRequestError(
            RESTORE_ERROR_CODES.find((code) => code === error) ??
                "operation_failed",
        );
    }
    if (!Array.isArray(value)) return restoreInvalid();
    return {
        kind: "desktop",
        operations: value.map((entry: unknown) => recordedOperation(entry)),
    };
}

/**
 * Preview what undoing one recorded write would do.
 *
 * Rust reads the settings file for its change token and compares it with what
 * the write left, so the answer says whether each file would be written back,
 * removed, or left alone because it changed. No file content and no credential
 * value is read into it.
 */
export async function planAgentRestore(
    agent: ConfigurableAgent,
    operationId: string,
): Promise<RestorePlanResult> {
    if (!isTauri()) return { kind: "preview" };
    let value: unknown;
    try {
        value = await invoke<unknown>(AGENT_CONFIG_COMMANDS[agent].plan, {
            operationId,
        });
    } catch (error: unknown) {
        throw new RestoreRequestError(
            RESTORE_ERROR_CODES.find((code) => code === error) ??
                "operation_failed",
        );
    }
    return { kind: "desktop", plan: restorePlan(value) };
}

/**
 * Apply a restore the user confirmed.
 *
 * Rust rebuilds the preview and compares every reviewed digest, so a settings
 * edit made after the preview is refused with `plan_stale` instead of being
 * overwritten. Restoring reads no credential: the previous content comes from
 * the private copy, and the settings file ends up with whichever key it held
 * before the write being undone. A file-level conflict is a state in the result,
 * not a thrown error.
 */
export async function applyAgentRestore(
    agent: ConfigurableAgent,
    operationId: string,
    reviewed: ReviewedRestoreStep[],
): Promise<RestoreOutcome> {
    if (!isTauri()) throw new RestoreRequestError("desktop_required");
    let value: unknown;
    try {
        value = await invoke<unknown>(AGENT_CONFIG_COMMANDS[agent].apply, {
            input: { operationId, reviewed },
        });
    } catch (error: unknown) {
        throw new RestoreRequestError(
            RESTORE_ERROR_CODES.find((code) => code === error) ??
                "operation_failed",
        );
    }
    const result = restoreOutcome(value, reviewed);
    if (result.operationId !== operationId) return restoreInvalid();
    return result;
}
