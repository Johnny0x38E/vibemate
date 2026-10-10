import { invoke, isTauri } from "@tauri-apps/api/core";
import { hasExactKeys, isObject } from "./providers";

/** Safe domain codes; no arbitrary backend message may reach the UI. */
const MCP_CODES = [
    "storage_unavailable",
    "read_failed",
    "write_failed",
    "invalid_request",
    "display_name_invalid",
    "server_name_invalid",
    "server_name_taken",
    "command_invalid",
    "args_invalid",
    "cwd_invalid",
    "url_invalid",
    "field_name_invalid",
    "secret_required",
    "secret_invalid",
    "credential_store_unavailable",
    "credential_store_access_denied",
    "credential_store_failed",
    "not_found",
    "revision_conflict",
    "invalid_stored_definition",
    "outcome_unknown",
    "operation_failed",
] as const;
/** Sanitized domain, preview or response-validation failure. */
export type McpErrorCode =
    (typeof MCP_CODES)[number] | "desktop_required" | "invalid_response";
/** Safe errors deliberately discard all original payloads, including secrets. */
export class McpRequestError extends Error {
    readonly code: McpErrorCode;
    constructor(code: McpErrorCode) {
        super("MCP definition request failed.");
        this.name = "McpRequestError";
        this.code = code;
    }
}
/** Null retains an existing value; a string replaces it in the OS store. */
export interface McpSecretInput {
    name: string;
    value: string | null;
}
/** Literal central definitions only; these inputs never run a server. */
export type McpConnectionInput =
    | {
          type: "stdio";
          command: string;
          args: string[];
          cwd: string | null;
          env: McpSecretInput[];
      }
    | { type: "http"; url: string; headers: McpSecretInput[] };
/** Safe metadata contains names, never values or credential references. */
export type McpConnection =
    | {
          type: "stdio";
          command: string;
          args: string[];
          cwd: string | null;
          env: string[];
      }
    | { type: "http"; url: string; headers: string[] };
/** Stable identity and revision isolate updates from renamed definitions. */
export interface McpRecord {
    id: string;
    displayName: string;
    serverName: string;
    enabled: boolean;
    revision: number;
    connection: McpConnection;
    createdAtMs: number;
    updatedAtMs: number;
    cleanupPending: boolean;
}
/** Create with null identity/revision; edit at the last known revision. */
export interface SaveMcpInput {
    id: string | null;
    expectedRevision: number | null;
    displayName: string;
    serverName: string;
    enabled: boolean;
    connection: McpConnectionInput;
}
/** One bounded cursor page, or an honest browser preview. */
export type McpPageResult =
    | { kind: "preview" }
    | {
          kind: "desktop";
          page: { items: McpRecord[]; nextCursor: string | null };
      };
function invalid(): never {
    throw new McpRequestError("invalid_response");
}
function object(
    value: unknown,
    keys: readonly string[],
): Record<string, unknown> {
    if (!isObject(value) || !hasExactKeys(value, keys)) invalid();
    return value;
}
function string(value: unknown): string {
    if (typeof value !== "string") invalid();
    return value;
}
function identity(value: unknown): string {
    const id = string(value);
    if (!/^[0-9a-f]{32}$/.test(id)) invalid();
    return id;
}
function integer(value: unknown): number {
    if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0)
        invalid();
    return value;
}
function boolean(value: unknown): boolean {
    if (typeof value !== "boolean") invalid();
    return value;
}
function strings(value: unknown): string[] {
    if (!Array.isArray(value)) invalid();
    return value.map((entry: unknown) => string(entry));
}
function names(value: unknown): string[] {
    const result = strings(value);
    if (
        result.length > 32 ||
        new Set(result).size !== result.length ||
        result.some((name) => name.length === 0)
    )
        invalid();
    return result;
}
function connection(value: unknown): McpConnection {
    if (!isObject(value)) invalid();
    if (value["type"] === "stdio") {
        const data = object(value, ["type", "command", "args", "cwd", "env"]);
        const args = strings(data["args"]);
        if (args.length > 64) invalid();
        return {
            type: "stdio",
            command: string(data["command"]),
            args,
            cwd: data["cwd"] === null ? null : string(data["cwd"]),
            env: names(data["env"]),
        };
    }
    const data = object(value, ["type", "url", "headers"]);
    if (data["type"] !== "http") invalid();
    return {
        type: "http",
        url: string(data["url"]),
        headers: names(data["headers"]),
    };
}
function record(value: unknown): McpRecord {
    const data = object(value, [
        "id",
        "displayName",
        "serverName",
        "enabled",
        "revision",
        "connection",
        "createdAtMs",
        "updatedAtMs",
        "cleanupPending",
    ]);
    const row = {
        id: identity(data["id"]),
        displayName: string(data["displayName"]),
        serverName: string(data["serverName"]),
        enabled: boolean(data["enabled"]),
        revision: integer(data["revision"]),
        connection: connection(data["connection"]),
        createdAtMs: integer(data["createdAtMs"]),
        updatedAtMs: integer(data["updatedAtMs"]),
        cleanupPending: boolean(data["cleanupPending"]),
    };
    if (
        row.revision < 1 ||
        row.updatedAtMs < row.createdAtMs ||
        row.displayName.trim() === "" ||
        !/^[A-Za-z0-9_-]{1,64}$/.test(row.serverName)
    )
        invalid();
    return row;
}
async function call(
    command: string,
    args: Record<string, unknown>,
): Promise<unknown> {
    try {
        return await invoke<unknown>(command, args);
    } catch (error: unknown) {
        throw new McpRequestError(
            MCP_CODES.find((code) => code === error) ?? "operation_failed",
        );
    }
}
function desktop(): void {
    if (!isTauri()) throw new McpRequestError("desktop_required");
}
/** Read bounded metadata; preview performs no IPC or credential access. */
export async function listMcpDefinitions(
    after: string | null,
    limit = 20,
): Promise<McpPageResult> {
    if (!isTauri()) return { kind: "preview" };
    const data = object(await call("list_mcp_definitions", { after, limit }), [
        "items",
        "nextCursor",
    ]);
    if (!Array.isArray(data["items"])) invalid();
    const items = data["items"].map((value: unknown) => record(value));
    const nextCursor =
        data["nextCursor"] === null ? null : identity(data["nextCursor"]);
    if (
        items.length > limit ||
        items.some(
            (row, index) =>
                (after !== null && row.id <= after) ||
                (index > 0 && row.id <= (items[index - 1]?.id ?? "")),
        ) ||
        (nextCursor !== null && nextCursor !== items[items.length - 1]?.id)
    )
        invalid();
    return { kind: "desktop", page: { items, nextCursor } };
}
/** Re-read non-secret metadata before editing or retrying an unknown save. */
export async function getMcpDefinition(id: string): Promise<McpRecord> {
    desktop();
    const row = record(await call("get_mcp_definition", { id }));
    if (row.id !== id) invalid();
    return row;
}
/** Save central metadata and protected values; no server execution is exposed. */
export async function saveMcpDefinition(
    request: SaveMcpInput,
): Promise<McpRecord> {
    desktop();
    const row = record(await call("save_mcp_definition", { request }));
    if (
        (request.id !== null && row.id !== request.id) ||
        row.revision !== (request.expectedRevision ?? 0) + 1 ||
        row.enabled !== request.enabled ||
        row.connection.type !== request.connection.type
    )
        invalid();
    return row;
}
/** Retry obsolete credential deletion, retaining the current definition. */
export async function cleanupMcpCredentials(id: string): Promise<McpRecord> {
    desktop();
    const row = record(await call("cleanup_mcp_credentials", { id }));
    if (row.id !== id) invalid();
    return row;
}
