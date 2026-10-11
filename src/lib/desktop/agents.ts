import { invoke, isTauri } from "@tauri-apps/api/core";
import { hasExactKeys, isObject } from "./providers";

/** Stable adapters with separately documented configuration contracts. */
export type AgentId = "pi" | "grokBuild";
/** Optional current-window entry paths; Rust never executes either path. */
export interface DiscoveryInput {
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
/** Honest preview or two validated Agent observations in adapter order. */
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
function path(value: unknown): string | null {
    if (value === null) return null;
    if (
        typeof value !== "string" ||
        new TextEncoder().encode(value).length > 4096 ||
        Array.from(value).some((char) => {
            const point = char.codePointAt(0);
            return (
                point !== undefined &&
                (point < 32 || (point >= 127 && point <= 159))
            );
        }) ||
        !/^(\/|[A-Za-z]:[\\/]|\\\\[^\\]+\\)/u.test(value)
    )
        return invalid();
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
    const names =
        id === "pi"
            ? ["settings.json", "models.json", "mcp.json"]
            : ["config.toml", "managed_config.toml", "requirements.toml"];
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
    if (!Array.isArray(value) || value.length !== 2) return invalid();
    return {
        kind: "desktop",
        items: [
            observation(value[0], "pi"),
            observation(value[1], "grokBuild"),
        ],
    };
}
