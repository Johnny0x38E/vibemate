import { invoke, isTauri } from "@tauri-apps/api/core";
import * as z from "zod";
import {
    RESTORE_STATES,
    WRITE_ERROR_CODES,
    isSafePath,
    type ConfigurableAgent,
    type RestoreState,
} from "./agents";

const ERROR_CODES = [
    "operation_not_found",
    "plan_stale",
    "invalid_request",
    "storage_unavailable",
    "operation_failed",
    "desktop_required",
] as const;

/** Stable failure codes this boundary reports. */
export type VersionRestoreErrorCode =
    (typeof ERROR_CODES)[number] | "invalid_response";

/** Safe version-restore failure; no payload or validation diagnostics are kept. */
export class VersionRestoreRequestError extends Error {
    readonly code: VersionRestoreErrorCode;

    /** Discard original diagnostics; consumers translate only this code. */
    constructor(code: VersionRestoreRequestError["code"]) {
        super("The configuration version restore failed.");
        this.name = "VersionRestoreRequestError";
        this.code = code;
    }
}

const digest = z.string().regex(/^[0-9a-f]{64}$/u);
const revisionIdSchema = z.string().regex(/^[A-Za-z0-9_-]{1,128}$/u);
const operationId = z.string().regex(/^[0-9a-f]{32}$/u);
const entryKeySchema = z
    .string()
    .regex(/^(?:operation:[0-9a-f]{32}|batch:[A-Za-z0-9_-]{1,128})$/u);
const ownerRef = z.string().regex(/^[\x20-\x7e]{1,64}$/u);
const ordinal = z.int().min(0).max(15);
const action = z.enum(["restore", "remove", "skip", "blocked"]);
// Only reasons the version-restore planner emits. `target_changed` and
// `restore_history_unknown` belong to the legacy rollback preview.
const blockedReason = z.enum([
    "symlink_refused",
    "target_read_only",
    "target_path_invalid",
    "target_unreadable",
    "target_too_large",
    "backup_unavailable",
    "snapshot_unavailable",
]);
const intent = z.enum(["apply", "restore", "compensation", "version_restore"]);
const configFormat = z.enum(["json", "toml"]);
const summaryReason = z.enum([
    "symlink_refused",
    "target_read_only",
    "target_path_invalid",
    "target_unreadable",
    "target_too_large",
    "backup_unavailable",
    "snapshot_unavailable",
    "text_unavailable",
]);
const changeKind = z.enum(["added", "removed", "replaced"]);
const fieldChangeSchema = z
    .strictObject({
        path: z.string().min(1).max(4096),
        kind: changeKind,
        before: z.string().nullable(),
        after: z.string().nullable(),
        secret: z.boolean(),
    })
    .refine((change) =>
        change.kind === "added"
            ? change.before === null && change.after !== null
            : change.kind === "removed"
              ? change.before !== null && change.after === null
              : change.before !== null && change.after !== null,
    );
/** Strict field summary shared by native model and retained-version previews. */
export const summarySchema = z
    .strictObject({
        format: configFormat,
        changes: z.array(fieldChangeSchema).max(200),
        truncated: z.boolean(),
        reason: summaryReason.nullable(),
    })
    .refine((summary) =>
        summary.reason === null ? true : summary.changes.length === 0,
    );

const fileSchema = z
    .strictObject({
        revisionId: revisionIdSchema.nullable(),
        ordinal,
        targetPath: z.string().refine(isSafePath),
        action,
        reason: blockedReason.nullable(),
        currentDigest: digest.nullable(),
        targetDigest: digest.nullable(),
        restoredOwnerRef: ownerRef.nullable(),
        summary: summarySchema,
    })
    .refine((file) => {
        if (file.action === "blocked") return file.reason !== null;
        if (file.reason !== null) return false;
        if (file.action === "restore") {
            return (
                file.revisionId !== null &&
                file.targetDigest !== null &&
                file.currentDigest !== file.targetDigest
            );
        }
        if (file.action === "remove") {
            return (
                file.revisionId !== null &&
                file.targetDigest === null &&
                file.currentDigest !== null &&
                file.restoredOwnerRef === null
            );
        }
        // skip: the current bytes (or absence) already match the retained state.
        return file.currentDigest === file.targetDigest;
    });

const planSchema = z
    .strictObject({
        entryKey: entryKeySchema,
        operationId,
        intent,
        ownerRef: ownerRef.nullable(),
        createdAt: z.int().nonnegative(),
        files: z.array(fileSchema).max(16),
        restorable: z.boolean(),
        reason: z.enum(["history_unresolved"]).nullable(),
    })
    .superRefine((plan, context) => {
        const isOperation = plan.entryKey.startsWith("operation:");
        const expectedIntent = isOperation ? "apply" : null;
        if (
            (isOperation && plan.intent !== expectedIntent) ||
            (!isOperation && plan.intent === "apply") ||
            (isOperation &&
                plan.entryKey !== `operation:${plan.operationId}`) ||
            new Set(plan.files.map((file) => file.ordinal)).size !==
                plan.files.length ||
            new Set(
                plan.files.flatMap((file) =>
                    file.revisionId === null ? [] : [file.revisionId],
                ),
            ).size !==
                plan.files.filter((file) => file.revisionId !== null).length
        ) {
            context.addIssue({ code: "custom" });
            return;
        }
        if (plan.reason !== null) {
            if (plan.files.length !== 0 || plan.restorable) {
                context.addIssue({ code: "custom" });
            }
            return;
        }
        const changes = plan.files.some(
            (file) => file.action === "restore" || file.action === "remove",
        );
        const blocked = plan.files.some((file) => file.action === "blocked");
        if (plan.restorable !== (changes && !blocked)) {
            context.addIssue({ code: "custom" });
        }
    });

const outcomeFileSchema = z
    .strictObject({
        revisionId: revisionIdSchema,
        targetPath: z.string().refine(isSafePath),
        action: z.enum(["restore", "remove"]),
        state: z.enum(RESTORE_STATES),
        errorCode: z
            .string()
            .regex(/^[a-z_]{1,64}$/u)
            .nullable(),
    })
    .refine((file) => {
        if (file.state === "restored") return file.errorCode === null;
        if (file.errorCode === null) return false;
        if (file.state === "conflict")
            return file.errorCode === "target_changed";
        if (file.state === "unavailable")
            return file.errorCode === "backup_unavailable";
        if (file.state === "unknown")
            return file.errorCode === "outcome_unknown";
        return (WRITE_ERROR_CODES as readonly string[]).includes(
            file.errorCode,
        );
    });

const outcomeSchema = z
    .strictObject({
        entryKey: entryKeySchema,
        files: z.array(outcomeFileSchema).min(1).max(16),
    })
    .refine(
        (outcome) =>
            new Set(outcome.files.map((file) => file.revisionId)).size ===
            outcome.files.length,
    );

const reviewedSchema = z.strictObject({
    revisionId: revisionIdSchema,
    currentDigest: digest.nullable(),
});

const textSideSchema = z.discriminatedUnion("state", [
    z.strictObject({ state: z.literal("absent") }),
    z.strictObject({
        state: z.literal("present"),
        text: z.string().max(1024 * 1024),
        bytes: z
            .int()
            .nonnegative()
            .max(1024 * 1024),
    }),
    z.strictObject({
        state: z.literal("unavailable"),
        reason: summaryReason,
    }),
]);
/** Complete sanitized text; the identity is scoped by the calling plan. */
export const textSchema = z.strictObject({
    revisionId: revisionIdSchema,
    targetPath: z.string().refine(isSafePath),
    format: configFormat,
    current: textSideSchema,
    target: textSideSchema,
});
/** Live sanitized text for one trusted Agent scope file, with no history anchor. */
export const currentTextSchema = z.strictObject({
    targetPath: z.string().refine(isSafePath),
    format: configFormat,
    current: textSideSchema,
});

/** One field-level current-to-retained change. */
export type VersionFieldChange = z.infer<typeof fieldChangeSchema>;
/** Sanitized field summary for one file. */
export type VersionFileSummary = z.infer<typeof summarySchema>;
/** One file of a version-restore preview. */
export type VersionRestoreFile = z.infer<typeof fileSchema>;
/** One side of the complete-text dialog. */
export type VersionTextSide = z.infer<typeof textSideSchema>;
/** Complete sanitized current and retained text for one file. */
export type VersionText = z.infer<typeof textSchema>;
/** Live sanitized text for one file in an Agent's trusted scope. */
export type CurrentConfigText = z.infer<typeof currentTextSchema>;
/** What restoring one history entry would do. */
export type VersionRestorePlan = z.infer<typeof planSchema>;
/** One file as the confirmation sends it back. */
export type ReviewedVersionFile = z.infer<typeof reviewedSchema>;
/** What one confirmed version restore did per file. */
export type VersionRestoreFileOutcome = z.infer<typeof outcomeFileSchema>;
/** Result of one confirmed version restore. */
export type VersionRestoreOutcome = z.infer<typeof outcomeSchema>;

/** A state code the restore result can carry for one file. */
export type VersionRestoreState = RestoreState;

function invalidResponse(): never {
    throw new VersionRestoreRequestError("invalid_response");
}

function requestError(error: unknown): VersionRestoreRequestError {
    return new VersionRestoreRequestError(
        ERROR_CODES.find((code) => code === error) ?? "operation_failed",
    );
}

/**
 * Read what restoring one history entry would change, without writing anything.
 *
 * Browser preview returns no invented plan and never invokes IPC. The request
 * carries only the Agent and the entry key; Rust selects the root, files,
 * snapshots and current-state tokens. A malformed response rejects as a whole
 * with `invalid_response`.
 */
export async function planVersionRestore(
    agent: ConfigurableAgent,
    entryKey: string,
): Promise<
    { kind: "preview" } | { kind: "desktop"; plan: VersionRestorePlan }
> {
    if (!isTauri()) return { kind: "preview" };
    const request = z
        .strictObject({
            agent: z.enum(["claudeCode", "pi", "grokBuild"]),
            entryKey: entryKeySchema,
        })
        .safeParse({ agent, entryKey });
    if (!request.success)
        throw new VersionRestoreRequestError("invalid_request");
    let value: unknown;
    try {
        value = await invoke<unknown>("plan_configuration_version_restore", {
            input: request.data,
        });
    } catch (error: unknown) {
        throw requestError(error);
    }
    const plan = planSchema.safeParse(value);
    if (!plan.success) invalidResponse();
    return { kind: "desktop", plan: plan.data };
}

/**
 * Confirm a reviewed version restore.
 *
 * Exactly the files the preview said it would change are sent back with the
 * current digest it showed. Rust rebuilds the plan under the configuration
 * lock and refuses any changed file, snapshot or journal with `plan_stale`.
 * The outcome must describe exactly those revisions; otherwise it is rejected
 * as an unexpected response.
 */
export async function applyVersionRestore(
    agent: ConfigurableAgent,
    entryKey: string,
    reviewed: readonly ReviewedVersionFile[],
): Promise<VersionRestoreOutcome> {
    if (!isTauri()) throw new VersionRestoreRequestError("desktop_required");
    const request = z
        .strictObject({
            agent: z.enum(["claudeCode", "pi", "grokBuild"]),
            entryKey: entryKeySchema,
            reviewed: z.array(reviewedSchema).min(1).max(16),
        })
        .safeParse({ agent, entryKey, reviewed });
    if (!request.success)
        throw new VersionRestoreRequestError("invalid_request");
    if (
        new Set(request.data.reviewed.map((file) => file.revisionId)).size !==
        request.data.reviewed.length
    )
        throw new VersionRestoreRequestError("invalid_request");
    let value: unknown;
    try {
        value = await invoke<unknown>("apply_configuration_version_restore", {
            input: request.data,
        });
    } catch (error: unknown) {
        throw requestError(error);
    }
    const outcome = outcomeSchema.safeParse(value);
    if (
        !outcome.success ||
        outcome.data.entryKey !== request.data.entryKey ||
        outcome.data.files.length !== request.data.reviewed.length ||
        new Set(outcome.data.files.map((file) => file.revisionId)).size !==
            outcome.data.files.length ||
        outcome.data.files.some(
            (file) =>
                !request.data.reviewed.some(
                    (reviewedFile) =>
                        reviewedFile.revisionId === file.revisionId,
                ),
        )
    )
        invalidResponse();
    return outcome.data;
}

/**
 * Read complete sanitized current and retained text for one revision.
 *
 * The current digest from the preview is repeated; Rust refuses with
 * `plan_stale` when the file or entry changed. Text is sanitized before it
 * crosses IPC, and an unparseable side is reported as `unavailable` instead of
 * returning raw content. Browser preview returns no invented text.
 */
export async function readVersionText(
    agent: ConfigurableAgent,
    entryKey: string,
    revisionId: string,
    currentDigest: string | null,
): Promise<{ kind: "preview" } | { kind: "desktop"; text: VersionText }> {
    if (!isTauri()) return { kind: "preview" };
    const request = z
        .strictObject({
            agent: z.enum(["claudeCode", "pi", "grokBuild"]),
            entryKey: entryKeySchema,
            revisionId: revisionIdSchema,
            currentDigest: digest.nullable(),
        })
        .safeParse({ agent, entryKey, revisionId, currentDigest });
    if (!request.success)
        throw new VersionRestoreRequestError("invalid_request");
    let value: unknown;
    try {
        value = await invoke<unknown>("read_configuration_version_text", {
            input: request.data,
        });
    } catch (error: unknown) {
        throw requestError(error);
    }
    const parsed = textSchema.safeParse(value);
    if (!parsed.success || parsed.data.revisionId !== revisionId)
        invalidResponse();
    return { kind: "desktop", text: parsed.data };
}

/**
 * Read the live, sanitized text of one trusted Agent scope file.
 *
 * No history entry or digest is involved: the call still works after an
 * external edit stopped the file matching a retained version. The target path
 * is one the history response already reported; Rust revalidates it against
 * the adapter's fixed scope. Browser preview returns no invented text.
 */
export async function readCurrentConfigurationText(
    agent: ConfigurableAgent,
    targetPath: string,
): Promise<{ kind: "preview" } | { kind: "desktop"; text: CurrentConfigText }> {
    if (!isTauri()) return { kind: "preview" };
    const request = z
        .strictObject({
            agent: z.enum(["claudeCode", "pi", "grokBuild"]),
            targetPath: z.string().refine(isSafePath),
        })
        .safeParse({ agent, targetPath });
    if (!request.success)
        throw new VersionRestoreRequestError("invalid_request");
    let value: unknown;
    try {
        value = await invoke<unknown>("read_current_configuration_text", {
            input: request.data,
        });
    } catch (error: unknown) {
        throw requestError(error);
    }
    const parsed = currentTextSchema.safeParse(value);
    if (!parsed.success || parsed.data.targetPath !== targetPath)
        invalidResponse();
    return { kind: "desktop", text: parsed.data };
}
