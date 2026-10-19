import { invoke, isTauri } from "@tauri-apps/api/core";
import * as z from "zod";
import { isSafePath, type ConfigurableAgent } from "./agents";

const ERROR_CODES = [
    "operation_not_found",
    "plan_stale",
    "invalid_request",
    "storage_unavailable",
    "operation_failed",
    "history_active",
    "history_unresolved",
    "history_referenced",
    "desktop_required",
] as const;

/** Stable failure codes this boundary reports. */
export type HistoryDeleteErrorCode =
    (typeof ERROR_CODES)[number] | "invalid_response";

/** Safe history-delete failure; no payload or validation diagnostics are kept. */
export class HistoryDeleteRequestError extends Error {
    readonly code: HistoryDeleteErrorCode;

    /** Discard original diagnostics; consumers translate only this code. */
    constructor(code: HistoryDeleteRequestError["code"]) {
        super("The configuration history delete failed.");
        this.name = "HistoryDeleteRequestError";
        this.code = code;
    }
}

const entryKeySchema = z
    .string()
    .regex(/^(?:operation:[0-9a-f]{32}|batch:[A-Za-z0-9_-]{1,128})$/u);
const leftoverSchema = z
    .strictObject({
        backupId: z.string().min(1).max(128),
        path: z.string().refine(isSafePath),
        errorCode: z.enum(["backup_unavailable"]),
    })
    .refine((row) => row.path.includes(row.backupId));
const outcomeSchema = z
    .strictObject({
        entryKey: entryKeySchema,
        state: z.enum(["completed", "failed"]),
        leftovers: z.array(leftoverSchema).max(64),
    })
    .refine((outcome) =>
        outcome.state === "completed"
            ? outcome.leftovers.length === 0
            : outcome.leftovers.length > 0,
    );

/** One exclusive private copy that survived a delete attempt. */
export type HistoryDeleteLeftover = z.infer<typeof leftoverSchema>;
/** Result of deleting one history entry. */
export type HistoryDeleteOutcome = z.infer<typeof outcomeSchema>;

function invalidResponse(): never {
    throw new HistoryDeleteRequestError("invalid_response");
}

function requestError(error: unknown): HistoryDeleteRequestError {
    return new HistoryDeleteRequestError(
        ERROR_CODES.find((code) => code === error) ?? "operation_failed",
    );
}

/**
 * Delete one history entry and its exclusive private backups.
 *
 * Browser preview refuses with `desktop_required`. The request carries only the
 * Agent and entry key; Rust selects the configuration root and backup store.
 * Retryable leftovers return as an outcome with `state = failed`, not an error.
 * A malformed response rejects as a whole with `invalid_response`.
 */
export async function deleteHistoryEntry(
    agent: ConfigurableAgent,
    entryKey: string,
): Promise<HistoryDeleteOutcome> {
    if (!isTauri()) throw new HistoryDeleteRequestError("desktop_required");
    const request = z
        .strictObject({
            agent: z.enum(["claudeCode", "pi", "grokBuild"]),
            entryKey: entryKeySchema,
        })
        .safeParse({ agent, entryKey });
    if (!request.success)
        throw new HistoryDeleteRequestError("invalid_request");
    let value: unknown;
    try {
        value = await invoke<unknown>("delete_configuration_history_entry", {
            input: request.data,
        });
    } catch (error: unknown) {
        throw requestError(error);
    }
    const outcome = outcomeSchema.safeParse(value);
    if (!outcome.success) invalidResponse();
    return outcome.data;
}
