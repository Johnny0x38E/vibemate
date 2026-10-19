import { invoke, isTauri } from "@tauri-apps/api/core";
import * as z from "zod";
import {
    APPLY_STATES,
    APPLY_STEP_STATES,
    RESTORE_STATES,
    WRITE_ERROR_CODES,
    isSafePath,
    type ConfigurableAgent,
} from "./agents";

const HISTORY_ERROR_CODES = [
    "invalid_request",
    "storage_unavailable",
    "operation_failed",
] as const;

/** History failures contain only safe codes, never a schema error or IPC payload. */
export class HistoryRequestError extends Error {
    readonly code: (typeof HISTORY_ERROR_CODES)[number] | "invalid_response";

    /** Discard original diagnostics; consumers translate only this stable code. */
    constructor(code: HistoryRequestError["code"]) {
        super("Reading configuration history failed.");
        this.name = "HistoryRequestError";
        this.code = code;
    }
}

/**
 * Reject terminal control characters in Provider display text.
 *
 * Provider validation already enforces this in Rust; repeating the check here
 * keeps a malformed journal/response from reaching the renderer.
 */
function isSafeLabel(value: string): boolean {
    // `for...of` walks code points, so emoji and other surrogate pairs stay
    // whole; only their control-character property matters here.
    for (const character of value) {
        const code = character.codePointAt(0) ?? 0;
        if (code < 0x20 || (code >= 0x7f && code <= 0x9f)) return false;
    }
    return true;
}

const digest = z.string().regex(/^[0-9a-f]{64}$/u);
const operationId = z.string().regex(/^[0-9a-f]{32}$/u);
const revisionId = z.string().regex(/^[A-Za-z0-9_-]{1,128}$/u);
const entryKey = z
    .string()
    .regex(/^(?:operation:[0-9a-f]{32}|batch:[A-Za-z0-9_-]{1,128})$/u);
const timestamp = z.int().nonnegative();
const ordinal = z.int().min(0).max(15);
// The history reader also accepts plan_stale from older/shared write journals.
const writeError = z.enum([...WRITE_ERROR_CODES, "plan_stale"]);
const revisionState = z.enum(["prepared", "completed", "failed", "unknown"]);
const backupState = z.enum(["not_needed", "available", "unavailable"]);
const cursorSchema = z.strictObject({
    scopeDigest: digest,
    createdAt: timestamp,
    entryKey,
});
const sourceStepSchema = z
    .strictObject({
        ordinal,
        targetPath: z.string().refine(isSafePath),
        expectedDigest: digest.nullable(),
        appliedDigest: digest.nullable(),
        previousExisted: z.boolean().nullable(),
        backupRecorded: z.boolean(),
        backupState,
        state: z.enum(APPLY_STEP_STATES),
        errorCode: writeError.nullable(),
        restoreState: z.enum(RESTORE_STATES).nullable(),
    })
    .refine(
        (step) =>
            (step.state === "failed") === (step.errorCode !== null) &&
            (step.state === "applied") === (step.appliedDigest !== null) &&
            (step.state === "applied" ||
                (!step.backupRecorded &&
                    step.previousExisted === null &&
                    step.restoreState === null)) &&
            (step.backupRecorded
                ? step.backupState !== "not_needed"
                : step.backupState === "not_needed"),
    );
const resolutionSchema = z.strictObject({
    previousState: z.enum(["prepared", "completed", "unknown"]),
    observedDigest: digest.nullable(),
    currentMatch: z.enum(["previous", "intended"]),
    resolvedAt: timestamp,
});
const revisionSchema = z
    .strictObject({
        revisionId,
        ordinal,
        previousDigest: digest.nullable(),
        intendedDigest: digest.nullable(),
        backupRecorded: z.boolean(),
        backupState,
        state: revisionState,
        resolution: resolutionSchema.nullable(),
        errorCode: writeError.nullable(),
        createdAt: timestamp,
        updatedAt: timestamp,
    })
    .refine((revision) => {
        const { state, errorCode, resolution } = revision;
        const validResult =
            state === "unknown"
                ? errorCode === "outcome_unknown"
                : state === "failed"
                  ? errorCode !== null && errorCode !== "outcome_unknown"
                  : errorCode === null;
        const validResolution =
            resolution === null ||
            (resolution.currentMatch === "previous"
                ? state === "failed" &&
                  errorCode === "write_failed" &&
                  resolution.observedDigest === revision.previousDigest
                : state === "completed" &&
                  resolution.observedDigest === revision.intendedDigest);
        return (
            validResult &&
            validResolution &&
            revision.backupRecorded === (revision.previousDigest !== null) &&
            (revision.backupRecorded
                ? revision.backupState !== "not_needed"
                : revision.backupState === "not_needed") &&
            revision.updatedAt >= revision.createdAt
        );
    });
const entrySchema = z
    .strictObject({
        entryKey,
        operationId,
        intent: z.enum(["apply", "restore", "compensation", "version_restore"]),
        // This is a bounded journal reference, not necessarily a Provider ID.
        ownerRef: z
            .string()
            .regex(/^[\x20-\x7e]{1,64}$/u)
            .nullable(),
        // Display text resolved by Rust from a still-saved Provider.
        ownerLabel: z.string().min(1).max(64).refine(isSafeLabel).nullable(),
        // Journal metadata only; a true value withholds the legacy restore action.
        requiresResolution: z.boolean(),
        sourceState: z.enum(APPLY_STATES),
        createdAt: timestamp,
        updatedAt: timestamp,
        sourceSteps: z.array(sourceStepSchema).min(1).max(16),
        revisions: z.array(revisionSchema).max(16),
    })
    .refine((entry) => {
        const isApply = entry.intent === "apply";
        return (
            (isApply
                ? entry.entryKey === `operation:${entry.operationId}`
                : entry.entryKey.startsWith("batch:") &&
                  entry.revisions.length > 0) &&
            entry.updatedAt >= entry.createdAt &&
            entry.sourceSteps.every((step, index) => step.ordinal === index) &&
            new Set(entry.revisions.map((revision) => revision.revisionId))
                .size === entry.revisions.length &&
            entry.revisions.every(
                (revision, index, revisions) =>
                    revision.ordinal < entry.sourceSteps.length &&
                    (index === 0 ||
                        (revisions[index - 1]?.ordinal ?? -1) <
                            revision.ordinal) &&
                    revision.updatedAt <= entry.updatedAt,
            )
        );
    });
const currentFileSchema = z.strictObject({
    targetPath: z.string().refine(isSafePath),
    currentDigest: digest.nullable(),
    readable: z.boolean(),
});
const currentSchema = z
    .strictObject({
        status: z.enum(["matched", "unrecorded", "partial", "unreadable"]),
        matchedEntryKey: entryKey.nullable(),
        files: z.array(currentFileSchema).max(16),
        // Provider custom display name for list cards; null when unmatched/removed.
        providerLabel: z.string().min(1).max(64).refine(isSafeLabel).nullable(),
        // Live model id from matched config files; null when unknown.
        modelId: z
            .string()
            .min(1)
            .max(256)
            .refine((value) => isSafeLabel(value) && !/\s/u.test(value))
            .nullable(),
        // Alias when saved, else model id — list-card right side (`modelAliasOrId`).
        modelLabel: z.string().min(1).max(256).refine(isSafeLabel).nullable(),
    })
    .refine((current) => {
        const matched = current.status === "matched";
        if (matched !== (current.matchedEntryKey !== null)) return false;
        if (current.status === "unreadable") {
            return current.files.some((file) => !file.readable);
        }
        if (!current.files.every((file) => file.readable)) return false;
        // modelLabel requires modelId; neither is invented off a non-match.
        if (current.modelLabel !== null && current.modelId === null)
            return false;
        if (
            !matched &&
            (current.providerLabel !== null ||
                current.modelId !== null ||
                current.modelLabel !== null)
        ) {
            return false;
        }
        // Absence is represented by a null digest on a readable file.
        return true;
    });
const pageSchema = z.strictObject({
    current: currentSchema,
    entries: z.array(entrySchema).max(50),
    nextCursor: cursorSchema.nullable(),
});

/** Seek position only; Rust rechecks the Agent/root scope on every read. */
export type HistoryCursor = z.infer<typeof cursorSchema>;
/** Original apply bookkeeping, not the result of a later restore batch. */
export type HistorySourceStep = z.infer<typeof sourceStepSchema>;
/** Recorded reconciliation observation, not proof of who changed a file. */
export type HistoryResolution = z.infer<typeof resolutionSchema>;
/** One file's recorded mutation result; `completed` may still need review. */
export type HistoryRevisionState = z.infer<typeof revisionState>;
/** Pre-mutation metadata; backupState is a real private-store check. */
export type HistoryRevision = z.infer<typeof revisionSchema>;
/**
 * One original operation or an independent restore/compensation batch, with
 * adapter-level Provider display text and interrupted-write status.
 */
export type HistoryEntry = z.infer<typeof entrySchema>;
/**
 * Live configuration pin returned with every history page.
 *
 * When matched, may carry `providerLabel` (Provider custom display name) and
 * `modelId` / `modelLabel` (alias, or id when no alias) for list cards as
 * `providerLabel | modelLabel`. Fields stay separate; null when unknown.
 */
export type HistoryCurrentConfig = z.infer<typeof currentSchema>;
/** Complete bounded metadata page, including the fixed current-config pin. */
export type HistoryPage = z.infer<typeof pageSchema>;

function isOlder(
    entry: { createdAt: number; entryKey: string },
    previous: { createdAt: number; entryKey: string },
): boolean {
    // Keys are ASCII, matching SQLite's binary tie-breaker (not locale ordering).
    return (
        entry.createdAt < previous.createdAt ||
        (entry.createdAt === previous.createdAt &&
            entry.entryKey < previous.entryKey)
    );
}

function parsePage(
    value: unknown,
    after: HistoryCursor | null,
    limit: number,
): HistoryPage {
    const result = pageSchema.safeParse(value);
    // Never throw a ZodError: its issue paths/keys can contain untrusted text.
    if (!result.success) throw new HistoryRequestError("invalid_response");
    const page = result.data;
    const last = page.entries[page.entries.length - 1];
    if (
        new Set(page.current.files.map((file) => file.targetPath)).size !==
            page.current.files.length ||
        page.entries.length > limit ||
        page.entries.some((entry, index, entries) => {
            const previous = index === 0 ? after : entries[index - 1];
            return previous != null && !isOlder(entry, previous);
        }) ||
        new Set(page.entries.map((entry) => entry.entryKey)).size !==
            page.entries.length ||
        new Set(
            page.entries.flatMap((entry) =>
                entry.revisions.map((revision) => revision.revisionId),
            ),
        ).size !==
            page.entries.reduce(
                (count, entry) => count + entry.revisions.length,
                0,
            ) ||
        (page.nextCursor !== null &&
            (last === undefined ||
                page.entries.length !== limit ||
                page.nextCursor.createdAt !== last.createdAt ||
                page.nextCursor.entryKey !== last.entryKey ||
                (after !== null &&
                    page.nextCursor.scopeDigest !== after.scopeDigest)))
    )
        throw new HistoryRequestError("invalid_response");
    return page;
}

/**
 * Read complete Agent/root-scoped history metadata, without touching credentials.
 *
 * Browser preview returns no invented records and never invokes IPC. Only the
 * Agent, cursor and bounded page size are sent; Rust selects the active root.
 * Rejects malformed/contradictory pages with a sanitized HistoryRequestError.
 * Pages are live journal snapshots, not a frozen export or recovery approval.
 */
export async function readConfigurationHistory(
    agent: ConfigurableAgent,
    after: HistoryCursor | null = null,
    limit = 20,
): Promise<{ kind: "preview" } | { kind: "desktop"; page: HistoryPage }> {
    if (!isTauri()) return { kind: "preview" };
    const request = z
        .strictObject({
            agent: z.enum(["claudeCode", "pi", "grokBuild"]),
            after: cursorSchema.nullable(),
            limit: z.int().min(1).max(50),
        })
        .safeParse({ agent, after, limit });
    if (!request.success) throw new HistoryRequestError("invalid_request");
    let value: unknown;
    try {
        value = await invoke<unknown>("configuration_history_page", {
            input: request.data,
        });
    } catch (error: unknown) {
        throw new HistoryRequestError(
            HISTORY_ERROR_CODES.find((code) => code === error) ??
                "operation_failed",
        );
    }
    return {
        kind: "desktop",
        page: parsePage(value, request.data.after, request.data.limit),
    };
}
