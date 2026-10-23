import { invoke, isTauri } from "@tauri-apps/api/core";
import { hasExactKeys, isObject } from "./providers";

const CODES = [
    "invalid_request",
    "definition_unavailable",
    "revision_conflict",
    "target_unavailable",
    "invalid_config",
    "name_conflict",
    "operation_failed",
] as const;
/** Preview-specific safe errors; raw native errors never reach the interface. */
export type McpPreviewErrorCode =
    (typeof CODES)[number] | "desktop_required" | "invalid_response";
/** Discards original error payloads, including paths or credential values. */
export class McpPreviewError extends Error {
    readonly code: McpPreviewErrorCode;
    constructor(code: McpPreviewErrorCode) {
        super("MCP preview failed.");
        this.name = "McpPreviewError";
        this.code = code;
    }
}
/** Only literal placeholders, never resolved environment or header values. */
export const MCP_SECRET_PLACEHOLDER = "••••••••";
/** A native new-entry proposal, not the existing file or an application token. */
export type PiMcpEntry =
    | {
          command: string;
          args: string[];
          cwd?: string;
          env: Record<string, string>;
          enabled: boolean;
      }
    | { url: string; headers: Record<string, string>; enabled: boolean };
/** Rust-selected target and revision-checked, sanitized new-entry proposal. */
export interface PiMcpPreview {
    target: {
        path: string;
        exists: boolean;
        contentDigest: string | null;
        symlink: boolean;
    };
    proposal: {
        definitionId: string;
        revision: number;
        serverName: string;
        redactedEntry: PiMcpEntry;
        sessionRelativeCwd: boolean;
        expandsHome: boolean;
    };
}
function invalid(): never {
    throw new McpPreviewError("invalid_response");
}
function object(
    value: unknown,
    keys: readonly string[],
): Record<string, unknown> {
    if (!isObject(value) || !hasExactKeys(value, keys)) invalid();
    return value;
}
function text(value: unknown, limit = 4096): string {
    if (
        typeof value !== "string" ||
        value.length > limit ||
        Array.from(value).some(
            (character) =>
                character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127,
        )
    )
        invalid();
    return value;
}
function boolean(value: unknown): boolean {
    if (typeof value !== "boolean") invalid();
    return value;
}
function placeholders(
    value: unknown,
    kind: "env" | "headers",
): Record<string, string> {
    if (!isObject(value) || Object.keys(value).length > 32) invalid();
    const result: Record<string, string> = {};
    for (const [name, secret] of Object.entries(value)) {
        const pattern =
            kind === "env"
                ? /^[A-Za-z_][A-Za-z0-9_]*$/
                : /^[!#$%&'*+.^_`|~a-z0-9-]+$/;
        if (
            name.length > 128 ||
            !pattern.test(name) ||
            secret !== MCP_SECRET_PLACEHOLDER
        )
            invalid();
        Object.defineProperty(result, name, {
            value: MCP_SECRET_PLACEHOLDER,
            enumerable: true,
        });
    }
    return result;
}
function entry(value: unknown): PiMcpEntry {
    if (!isObject(value)) invalid();
    if ("command" in value) {
        const hasCwd = "cwd" in value;
        const data = object(value, [
            "command",
            "args",
            "env",
            "enabled",
            ...(hasCwd ? ["cwd"] : []),
        ]);
        if (!Array.isArray(data["args"]) || data["args"].length > 64) invalid();
        return {
            command: text(data["command"], 2048),
            args: data["args"].map((arg: unknown) => text(arg, 2048)),
            ...(hasCwd ? { cwd: text(data["cwd"], 2048) } : {}),
            env: placeholders(data["env"], "env"),
            enabled: boolean(data["enabled"]),
        };
    }
    const data = object(value, ["url", "headers", "enabled"]);
    return {
        url: text(data["url"], 2048),
        headers: placeholders(data["headers"], "headers"),
        enabled: boolean(data["enabled"]),
    };
}
/** Read only the fixed Pi user-level target. Browser preview never invokes IPC. */
export async function previewPiMcpRegistration(
    definitionId: string,
    expectedRevision: number,
): Promise<PiMcpPreview> {
    if (!isTauri()) throw new McpPreviewError("desktop_required");
    let response: unknown;
    try {
        response = await invoke<unknown>("preview_pi_mcp_registration", {
            request: { definitionId, expectedRevision },
        });
    } catch (error: unknown) {
        throw new McpPreviewError(
            CODES.find((code) => code === error) ?? "operation_failed",
        );
    }
    const data = object(response, ["target", "proposal"]);
    const target = object(data["target"], [
        "path",
        "exists",
        "contentDigest",
        "symlink",
    ]);
    const proposal = object(data["proposal"], [
        "definitionId",
        "revision",
        "serverName",
        "redactedEntry",
        "sessionRelativeCwd",
        "expandsHome",
    ]);
    const path = text(target["path"]);
    const exists = boolean(target["exists"]);
    const digest = target["contentDigest"];
    if (
        !/^(?:\/|[A-Za-z]:[\\/]|\\\\)/.test(path) ||
        (exists
            ? typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)
            : digest !== null) ||
        proposal["definitionId"] !== definitionId ||
        !/^[0-9a-f]{32}$/.test(definitionId) ||
        proposal["revision"] !== expectedRevision ||
        !Number.isSafeInteger(expectedRevision) ||
        expectedRevision < 1
    )
        invalid();
    const serverName = text(proposal["serverName"], 64);
    if (!/^[A-Za-z0-9_-]+$/.test(serverName)) invalid();
    return {
        target: {
            path,
            exists,
            contentDigest: exists ? text(digest) : null,
            symlink: boolean(target["symlink"]),
        },
        proposal: {
            definitionId,
            revision: expectedRevision,
            serverName,
            redactedEntry: entry(proposal["redactedEntry"]),
            sessionRelativeCwd: boolean(proposal["sessionRelativeCwd"]),
            expandsHome: boolean(proposal["expandsHome"]),
        },
    };
}
