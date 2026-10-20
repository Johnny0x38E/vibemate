import { invoke, isTauri } from "@tauri-apps/api/core";
import * as z from "zod";
import { CLAUDE_EFFORT_LEVELS, type ClaudeEffortLevel } from "./agents";
import {
    validateModelSnapshot,
    type ModelIdentity,
    type ModelSnapshot,
} from "./models";

/** Payload-free native failures; unknown diagnostics are discarded. */
export const CLAUDE_PREFERENCE_ERROR_CODES = [
    "storage_unavailable",
    "read_failed",
    "write_failed",
    "invalid_request",
    "not_found",
    "invalid_stored_data",
    "model_changed",
    "preference_changed",
    "effort_not_mapped",
    "operation_failed",
    "desktop_required",
    "invalid_response",
] as const;
/** Safe starting-choice failure used for localized feedback. */
export type ClaudePreferenceErrorCode =
    (typeof CLAUDE_PREFERENCE_ERROR_CODES)[number];
/** No native diagnostics or untrusted response contents are retained. */
export class ClaudePreferenceError extends Error {
    readonly code: ClaudePreferenceErrorCode;
    constructor(code: ClaudePreferenceErrorCode) {
        super("The Claude Code effort preference operation failed.");
        this.name = "ClaudePreferenceError";
        this.code = code;
    }
}

const token = z.string().regex(/^[0-9a-f]{32}$/u);
const responseSchema = z.strictObject({
    snapshot: z.unknown(),
    revision: token.nullable(),
    selectedEffort: z.enum(CLAUDE_EFFORT_LEVELS).nullable(),
});
/** Model metadata and its reusable starting choice from one database snapshot. */
export interface ClaudeEffortPreference {
    snapshot: ModelSnapshot;
    revision: string | null;
    selectedEffort: ClaudeEffortLevel | null;
}
/** Both read tokens are required for an explicit remember action. */
export interface SaveClaudeEffortPreference extends ModelIdentity {
    expectedModelDigest: string;
    expectedRevision: string | null;
    selectedEffort: ClaudeEffortLevel | null;
}
function invalid(): never {
    throw new ClaudePreferenceError("invalid_response");
}
function preference(
    value: unknown,
    identity: ModelIdentity,
): ClaudeEffortPreference {
    const result = responseSchema.safeParse(value);
    if (
        !result.success ||
        (result.data.revision === null && result.data.selectedEffort !== null)
    )
        return invalid();
    try {
        return {
            snapshot: validateModelSnapshot(result.data.snapshot, identity),
            revision: result.data.revision,
            selectedEffort: result.data.selectedEffort,
        };
    } catch {
        return invalid();
    }
}
async function call(
    command: string,
    request: ModelIdentity | SaveClaudeEffortPreference,
): Promise<unknown> {
    try {
        return await invoke<unknown>(command, { request });
    } catch (error: unknown) {
        throw new ClaudePreferenceError(
            CLAUDE_PREFERENCE_ERROR_CODES.find((code) => code === error) ??
                "operation_failed",
        );
    }
}

/** Read only; browser preview does not invent a preference or write defaults. */
export async function getClaudeEffortPreference(
    identity: ModelIdentity,
): Promise<
    | { kind: "preview" }
    | { kind: "desktop"; preference: ClaudeEffortPreference }
> {
    if (!isTauri()) return { kind: "preview" };
    return {
        kind: "desktop",
        preference: preference(
            await call("get_claude_effort_preference", identity),
            identity,
        ),
    };
}

/** Remember a starting choice, not an Agent write. Verify the exact saved binding,
 * model token and effort before reporting success; native code owns CAS and rollback.
 */
export async function saveClaudeEffortPreference(
    input: SaveClaudeEffortPreference,
): Promise<ClaudeEffortPreference> {
    if (!isTauri()) throw new ClaudePreferenceError("desktop_required");
    const saved = preference(
        await call("save_claude_effort_preference", input),
        input,
    );
    if (
        saved.snapshot.contentDigest !== input.expectedModelDigest ||
        saved.selectedEffort !== input.selectedEffort
    )
        return invalid();
    return saved;
}
