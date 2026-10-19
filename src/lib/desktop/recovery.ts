import { invoke, isTauri } from "@tauri-apps/api/core";
import {
    APPLY_STATES,
    RESTORE_ERROR_CODES,
    RestoreRequestError,
    isSafePath,
    type ApplyState,
    type ConfigurableAgent,
} from "./agents";
import { hasExactKeys, isObject } from "./providers";

const READ_ERRORS = [
    "symlink_refused",
    "target_unreadable",
    "target_too_large",
    "target_path_invalid",
] as const;

/** One point-in-time observation; no stored configuration content crosses IPC. */
export interface RevisionObservation {
    id: string;
    batchId: string;
    ordinal: number;
    intent: "apply" | "restore" | "compensation";
    state: "prepared" | "unknown" | "completed";
    targetPath: string;
    previousDigest: string | null;
    intendedDigest: string | null;
    currentDigest: string | null;
    currentMatch: "previous" | "intended" | "both" | "diverged" | "unreadable";
    errorCode: (typeof READ_ERRORS)[number] | null;
    backupState: "not_needed" | "available" | "unavailable";
}

/** Backend-built review; its digest approves the full source/candidate set. */
export interface ResolutionReview {
    operationId: string;
    reviewedDigest: string;
    resolvable: boolean;
    revisions: RevisionObservation[];
}

/** Journal result only; does not claim files were restored or a session reloaded. */
export interface ResolutionOutcome {
    operationId: string;
    state: ApplyState;
}

function invalid(): never {
    throw new RestoreRequestError("invalid_response");
}
function digest(value: unknown): string {
    if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value))
        return invalid();
    return value;
}
function optionalDigest(value: unknown): string | null {
    return value === null ? null : digest(value);
}
function identity(value: unknown): string {
    if (typeof value !== "string" || !/^[A-Za-z0-9_-]{1,128}$/u.test(value))
        return invalid();
    return value;
}
function member<T extends string>(value: unknown, choices: readonly T[]): T {
    return choices.find((choice) => choice === value) ?? invalid();
}

function observation(value: unknown): RevisionObservation {
    if (
        !isObject(value) ||
        !hasExactKeys(value, [
            "id",
            "batchId",
            "ordinal",
            "intent",
            "state",
            "targetPath",
            "previousDigest",
            "intendedDigest",
            "currentDigest",
            "currentMatch",
            "errorCode",
            "backupState",
        ])
    )
        return invalid();
    const ordinal = value["ordinal"];
    const targetPath = value["targetPath"];
    if (
        typeof ordinal !== "number" ||
        !Number.isSafeInteger(ordinal) ||
        ordinal < 0 ||
        ordinal > 0xffff_ffff ||
        !isSafePath(targetPath)
    )
        return invalid();
    const currentMatch = member(value["currentMatch"], [
        "previous",
        "intended",
        "both",
        "diverged",
        "unreadable",
    ]);
    const previousDigest = optionalDigest(value["previousDigest"]);
    const intendedDigest = optionalDigest(value["intendedDigest"]);
    const currentDigest = optionalDigest(value["currentDigest"]);
    const backupState = member(value["backupState"], [
        "not_needed",
        "available",
        "unavailable",
    ]);
    const errorCode =
        value["errorCode"] === null
            ? null
            : member(value["errorCode"], READ_ERRORS);
    const matchesPrevious = currentDigest === previousDigest;
    const matchesIntended = currentDigest === intendedDigest;
    if (
        (currentMatch === "unreadable") !== (errorCode !== null) ||
        (currentMatch === "unreadable" && currentDigest !== null) ||
        (backupState === "not_needed") !== (previousDigest === null) ||
        (currentMatch === "previous" &&
            (!matchesPrevious || matchesIntended)) ||
        (currentMatch === "intended" &&
            (!matchesIntended || matchesPrevious)) ||
        (currentMatch === "both" && (!matchesPrevious || !matchesIntended)) ||
        (currentMatch === "diverged" && (matchesPrevious || matchesIntended))
    )
        return invalid();
    return {
        id: identity(value["id"]),
        batchId: identity(value["batchId"]),
        ordinal,
        intent: member(value["intent"], ["apply", "restore", "compensation"]),
        state: member(value["state"], ["prepared", "unknown", "completed"]),
        targetPath,
        previousDigest,
        intendedDigest,
        currentDigest,
        currentMatch,
        errorCode,
        backupState,
    };
}

function parseReview(value: unknown, operationId: string): ResolutionReview {
    if (
        !isObject(value) ||
        !hasExactKeys(value, [
            "operationId",
            "reviewedDigest",
            "resolvable",
            "revisions",
        ]) ||
        value["operationId"] !== operationId ||
        !/^[0-9a-f]{32}$/u.test(operationId) ||
        typeof value["resolvable"] !== "boolean" ||
        !Array.isArray(value["revisions"])
    )
        return invalid();
    const revisions = value["revisions"].map((entry: unknown) =>
        observation(entry),
    );
    if (
        new Set(revisions.map((revision) => revision.id)).size !==
        revisions.length
    )
        return invalid();
    if (
        value["resolvable"] &&
        (revisions.length === 0 ||
            new Set(revisions.map((revision) => revision.ordinal)).size !==
                revisions.length ||
            revisions.some(
                (revision) =>
                    !["previous", "intended"].includes(revision.currentMatch) ||
                    revision.backupState === "unavailable" ||
                    (revision.state === "completed" &&
                        revision.currentMatch !== "intended"),
            ))
    )
        return invalid();
    return {
        operationId,
        reviewedDigest: digest(value["reviewedDigest"]),
        resolvable: value["resolvable"],
        revisions,
    };
}

/** Request an Agent-scoped observation review; browser preview cannot read history. */
export async function reviewConfigurationResolution(
    agent: ConfigurableAgent,
    operationId: string,
): Promise<
    { kind: "preview" } | { kind: "desktop"; review: ResolutionReview }
> {
    if (!isTauri()) return { kind: "preview" };
    let value: unknown;
    try {
        value = await invoke<unknown>("review_configuration_resolution", {
            input: { agent, operationId },
        });
    } catch (error: unknown) {
        throw new RestoreRequestError(
            RESTORE_ERROR_CODES.find((code) => code === error) ??
                "operation_failed",
        );
    }
    return { kind: "desktop", review: parseReview(value, operationId) };
}

/** Record approved observations only; never invokes restore or reads credentials. */
export async function resolveConfigurationOperation(
    agent: ConfigurableAgent,
    review: ResolutionReview,
): Promise<ResolutionOutcome> {
    if (!isTauri()) throw new RestoreRequestError("desktop_required");
    let value: unknown;
    try {
        value = await invoke<unknown>("resolve_configuration_operation", {
            input: {
                agent,
                operationId: review.operationId,
                reviewedDigest: review.reviewedDigest,
            },
        });
    } catch (error: unknown) {
        throw new RestoreRequestError(
            RESTORE_ERROR_CODES.find((code) => code === error) ??
                "operation_failed",
        );
    }
    if (
        !isObject(value) ||
        !hasExactKeys(value, ["operationId", "state"]) ||
        value["operationId"] !== review.operationId
    )
        return invalid();
    return {
        operationId: review.operationId,
        state: member(value["state"], APPLY_STATES),
    };
}
