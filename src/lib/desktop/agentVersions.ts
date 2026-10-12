import { invoke, isTauri } from "@tauri-apps/api/core";
import { AGENT_IDS, type AgentId } from "./agents";
import { hasExactKeys, isObject } from "./providers";

/** Explicitly approved fixed command; callers cannot supply arguments or environment. */
export interface VersionProbeInput {
    id: AgentId;
    entryPath: string;
    resolvedPath: string;
    confirmed: boolean;
}
/** CLI-reported version, not proof of authenticity, connection or configuration support. */
export interface AgentVersionResult {
    id: AgentId;
    entryPath: string;
    resolvedPath: string;
    version: string;
    evidence: "cliOutput";
}
const ERROR_CODES = [
    "confirmation_required",
    "busy",
    "entry_unavailable",
    "entry_changed",
    "unsupported_entry",
    "operation_failed",
    "timed_out",
    "output_too_large",
    "command_failed",
    "unrecognized_output",
    "desktop_required",
    "invalid_response",
] as const;
/** Safe failure categories; raw command output and IPC errors never enter UI state. */
export type VersionProbeErrorCode = (typeof ERROR_CODES)[number];
/** Sanitized version-probe failure translated by the feature UI. */
export class VersionProbeRequestError extends Error {
    readonly code: VersionProbeErrorCode;
    constructor(code: VersionProbeErrorCode) {
        super("Agent version probe failed.");
        this.name = "VersionProbeRequestError";
        this.code = code;
    }
}
function invalid(): never {
    throw new VersionProbeRequestError("invalid_response");
}
function isConfirmed(value: unknown): boolean {
    return value === true;
}
function safePath(value: unknown): value is string {
    return (
        typeof value === "string" &&
        new TextEncoder().encode(value).length <= 4096 &&
        !Array.from(value).some((char) => {
            const point = char.charCodeAt(0);
            return point < 32 || (point >= 127 && point <= 159);
        }) &&
        /^(\/|[A-Za-z]:[\\/]|\\\\[^\\]+\\)/u.test(value)
    );
}
/** Execute only a confirmed fixed version probe in desktop runtime.
 * Strictly validate the response against the approved identity and paths.
 */
export async function probeAgentVersion(
    input: VersionProbeInput,
): Promise<AgentVersionResult> {
    if (!isTauri()) throw new VersionProbeRequestError("desktop_required");
    if (
        !isObject(input) ||
        !hasExactKeys(input, [
            "id",
            "entryPath",
            "resolvedPath",
            "confirmed",
        ]) ||
        !AGENT_IDS.includes(input.id) ||
        !isConfirmed(input.confirmed) ||
        !safePath(input.entryPath) ||
        !safePath(input.resolvedPath)
    )
        return invalid();
    let value: unknown;
    try {
        value = await invoke<unknown>("probe_agent_version", { input });
    } catch (error: unknown) {
        for (const code of ERROR_CODES) {
            if (error === code) throw new VersionProbeRequestError(code);
        }
        throw new VersionProbeRequestError("operation_failed");
    }
    if (
        !isObject(value) ||
        !hasExactKeys(value, [
            "id",
            "entryPath",
            "resolvedPath",
            "version",
            "evidence",
        ]) ||
        value["id"] !== input.id ||
        value["entryPath"] !== input.entryPath ||
        value["resolvedPath"] !== input.resolvedPath ||
        value["evidence"] !== "cliOutput" ||
        typeof value["version"] !== "string" ||
        value["version"].length > 64 ||
        value["version"].trim() !== value["version"] ||
        !/^\d{1,10}\.\d{1,10}\.\d{1,10}(?:-[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*)?(?:\+[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*)?$/u.test(
            value["version"],
        )
    )
        return invalid();
    return {
        id: input.id,
        entryPath: input.entryPath,
        resolvedPath: input.resolvedPath,
        version: value["version"],
        evidence: "cliOutput",
    };
}
