import { invoke, isTauri } from "@tauri-apps/api/core";

/** Supported vendors; the values match Rust `ProviderKind` and the database. */
export const PROVIDER_KINDS = [
    "command-code",
    "deepseek",
    "openrouter",
] as const;
/** A supported vendor identity, never a display name. */
export type ProviderKind = (typeof PROVIDER_KINDS)[number];

/** Request protocols known to Rust; each template lists the ones it allows. */
export const PROVIDER_PROTOCOLS = [
    "chat_completions",
    "responses",
    "anthropic_messages",
] as const;
/** A protocol value accepted at the IPC boundary. */
export type ProviderProtocol = (typeof PROVIDER_PROTOCOLS)[number];

/** Largest page Rust returns; smaller requests are allowed, larger are rejected. */
export const MAX_PROVIDER_PAGE_SIZE = 100;

/** Defaults and limits for one vendor, owned by Rust. */
export interface ProviderTemplate {
    kind: ProviderKind;
    /** Brand name shown as-is in every language. */
    brandName: string;
    defaultBaseUrl: string;
    protocols: ProviderProtocol[];
    defaultProtocol: ProviderProtocol;
    /** Allowed extension keys; empty until evidence documents one. */
    extensionFields: string[];
}

/** One saved, non-secret provider instance. `id` is its only identity. */
export interface ProviderRecord {
    id: string;
    kind: ProviderKind;
    displayName: string;
    /** Normalized by Rust; may differ from the text the user typed. */
    baseUrl: string;
    protocol: ProviderProtocol;
    extensions: Record<string, string>;
    /** Starts at 1; pass it back as `expectedRevision` when editing. */
    revision: number;
    createdAtMs: number;
    updatedAtMs: number;
    /** Selected models for this configuration; Rust counts rows in SQLite. */
    selectedModelCount: number;
}

/** One bounded page ordered by creation time, then ID. */
export interface ProviderPage {
    items: ProviderRecord[];
    /** Opaque; pass it back as `after` for the next page. `null` on the last page. */
    nextCursor: string | null;
}

/** Fields for a new instance. Rust validates and normalizes every field. */
export interface CreateProviderInput {
    kind: ProviderKind;
    displayName: string;
    baseUrl: string;
    protocol: ProviderProtocol;
    extensions: Record<string, string>;
    /**
     * Required API key. Held only for this call and sent only to Rust, which
     * validates it and stores it in the OS credential store; the form clears it
     * afterwards. It never appears in a response, an error or a log.
     */
    secret: string;
}

/** Fields for an edit. The kind cannot change, so it is not part of the request. */
export interface UpdateProviderInput {
    /** Omit to preserve the stored key; supplied values replace it in the same save. */
    secret?: string;
    id: string;
    expectedRevision: number;
    displayName: string;
    baseUrl: string;
    protocol: ProviderProtocol;
    extensions: Record<string, string>;
}

/** Page request: `after` is `null` for the first page; `limit` is 1–100. */
export interface ListProvidersInput {
    after: string | null;
    limit: number;
}

/** Preview has no Rust runtime, so it has no templates or saved instances. */
export type ProviderTemplatesResult =
    { kind: "preview" } | { kind: "desktop"; templates: ProviderTemplate[] };

/** Preview must show that nothing is saved rather than an empty desktop list. */
export type ProviderPageResult =
    { kind: "preview" } | { kind: "desktop"; page: ProviderPage };

/** Codes Rust may return; each maps to one translated message. */
const RUST_PROVIDER_ERROR_CODES = [
    "storage_unavailable",
    "read_failed",
    "write_failed",
    "operation_failed",
    "invalid_stored_provider",
    "not_found",
    "revision_conflict",
    "invalid_request",
    "kind_not_supported",
    "protocol_not_supported",
    "display_name_invalid",
    "base_url_invalid",
    "base_url_not_https",
    "base_url_has_credentials",
    "extension_field_not_supported",
    "secret_invalid",
    "credential_store_unavailable",
    "credential_store_access_denied",
    "credential_store_failed",
    "create_outcome_unknown",
    "secret_outcome_unknown",
] as const;

/**
 * Safe Rust codes plus two detected here: `invalid_response` (malformed or
 * mismatched data) and `desktop_required` (browser preview cannot save).
 */
export type ProviderErrorCode =
    | (typeof RUST_PROVIDER_ERROR_CODES)[number]
    | "invalid_response"
    | "desktop_required";

/**
 * A sanitized provider failure. UI code translates `code` and never sees raw
 * runtime diagnostics. After `operation_failed` or `invalid_response` from a
 * save, the outcome is unknown: reload before claiming success or retrying.
 * `create_outcome_unknown` (create) and `secret_outcome_unknown` (key
 * replacement) are Rust's own unknown-outcome codes and are handled the same way.
 */
export class ProviderRequestError extends Error {
    /** Stable error code. */
    readonly code: ProviderErrorCode;

    /** Create a safe failure without retaining the original runtime error. */
    constructor(code: ProviderErrorCode) {
        super("Provider request failed.");
        this.name = "ProviderRequestError";
        this.code = code;
    }
}

function sanitize(error: unknown): ProviderRequestError {
    const known = RUST_PROVIDER_ERROR_CODES.find((code) => code === error);
    return new ProviderRequestError(known ?? "operation_failed");
}

/** True for a plain JSON object (not `null` or an array). */
export function isObject(value: unknown): value is Record<string, unknown> {
    return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * True when `value` has exactly these own keys. Responses are checked with a
 * strict key set, so an unexpected field (for example a key echoed back by
 * mistake) is rejected as `invalid_response` instead of being passed along.
 */
export function hasExactKeys(
    value: Record<string, unknown>,
    keys: readonly string[],
): boolean {
    const own = Object.keys(value);
    return own.length === keys.length && keys.every((key) => own.includes(key));
}

const RECORD_KEYS = [
    "id",
    "kind",
    "displayName",
    "baseUrl",
    "protocol",
    "extensions",
    "revision",
    "createdAtMs",
    "updatedAtMs",
    "selectedModelCount",
] as const;

function isProviderKind(value: unknown): value is ProviderKind {
    return PROVIDER_KINDS.some((kind) => kind === value);
}

function isProviderProtocol(value: unknown): value is ProviderProtocol {
    return PROVIDER_PROTOCOLS.some((protocol) => protocol === value);
}

function isProviderId(value: unknown): value is string {
    return typeof value === "string" && /^[0-9a-f]{32}$/.test(value);
}

function isTimestamp(value: unknown): value is number {
    return (
        Number.isSafeInteger(value) && typeof value === "number" && value >= 0
    );
}

/** The error for malformed or mismatched data from Rust. */
export function invalidResponse(): ProviderRequestError {
    return new ProviderRequestError("invalid_response");
}

/** Copy a string-to-string map, rejecting any other value type. */
function validateExtensions(value: unknown): Record<string, string> {
    if (!isObject(value)) throw invalidResponse();
    const extensions: Record<string, string> = {};
    for (const [key, entry] of Object.entries(value)) {
        if (typeof entry !== "string") throw invalidResponse();
        extensions[key] = entry;
    }
    return extensions;
}

function validateRecord(value: unknown): ProviderRecord {
    if (
        !isObject(value) ||
        !hasExactKeys(value, RECORD_KEYS) ||
        !isProviderId(value["id"]) ||
        !isProviderKind(value["kind"]) ||
        typeof value["displayName"] !== "string" ||
        typeof value["baseUrl"] !== "string" ||
        !isProviderProtocol(value["protocol"]) ||
        !Number.isSafeInteger(value["revision"]) ||
        typeof value["revision"] !== "number" ||
        value["revision"] < 1 ||
        !isTimestamp(value["createdAtMs"]) ||
        !isTimestamp(value["updatedAtMs"]) ||
        value["updatedAtMs"] < value["createdAtMs"] ||
        !Number.isSafeInteger(value["selectedModelCount"]) ||
        typeof value["selectedModelCount"] !== "number" ||
        value["selectedModelCount"] < 0
    ) {
        throw invalidResponse();
    }
    return {
        id: value["id"],
        kind: value["kind"],
        displayName: value["displayName"],
        baseUrl: value["baseUrl"],
        protocol: value["protocol"],
        extensions: validateExtensions(value["extensions"]),
        revision: value["revision"],
        createdAtMs: value["createdAtMs"],
        updatedAtMs: value["updatedAtMs"],
        selectedModelCount: value["selectedModelCount"],
    };
}

function validateTemplate(value: unknown): ProviderTemplate {
    if (
        !isObject(value) ||
        !isProviderKind(value["kind"]) ||
        typeof value["brandName"] !== "string" ||
        typeof value["defaultBaseUrl"] !== "string" ||
        !Array.isArray(value["protocols"]) ||
        !isProviderProtocol(value["defaultProtocol"]) ||
        !Array.isArray(value["extensionFields"])
    ) {
        throw invalidResponse();
    }
    const protocols: ProviderProtocol[] = [];
    for (const protocol of value["protocols"]) {
        if (!isProviderProtocol(protocol)) throw invalidResponse();
        protocols.push(protocol);
    }
    const extensionFields: string[] = [];
    for (const field of value["extensionFields"]) {
        if (typeof field !== "string") throw invalidResponse();
        extensionFields.push(field);
    }
    if (!protocols.includes(value["defaultProtocol"])) throw invalidResponse();
    return {
        kind: value["kind"],
        brandName: value["brandName"],
        defaultBaseUrl: value["defaultBaseUrl"],
        protocols,
        defaultProtocol: value["defaultProtocol"],
        extensionFields,
    };
}

function validatePage(value: unknown, limit: number): ProviderPage {
    if (
        !isObject(value) ||
        !Array.isArray(value["items"]) ||
        value["items"].length > limit ||
        !(
            value["nextCursor"] === null ||
            typeof value["nextCursor"] === "string"
        )
    ) {
        throw invalidResponse();
    }
    return {
        items: value["items"].map(validateRecord),
        nextCursor: value["nextCursor"],
    };
}

/**
 * Call one command, converting any rejection into a safe error code. Shared
 * with `providerSecrets.ts`, so both modules keep the same code allowlist.
 */
export async function callProviderCommand(
    command: string,
    args?: Record<string, unknown>,
): Promise<unknown> {
    try {
        return await invoke<unknown>(command, args);
    } catch (error: unknown) {
        throw sanitize(error);
    }
}

/** Read the built-in templates, or identify preview without invoking IPC. */
export async function listProviderTemplates(): Promise<ProviderTemplatesResult> {
    if (!isTauri()) return { kind: "preview" };
    const data = await callProviderCommand("list_provider_templates");
    if (!Array.isArray(data)) throw invalidResponse();
    return { kind: "desktop", templates: data.map(validateTemplate) };
}

/**
 * Read one page through Rust, or identify preview without invoking IPC.
 * A page larger than the requested `limit` is rejected as `invalid_response`.
 */
export async function listProviders(
    input: ListProvidersInput,
): Promise<ProviderPageResult> {
    if (!isTauri()) return { kind: "preview" };
    const data = await callProviderCommand("list_providers", {
        request: { after: input.after, limit: input.limit },
    });
    return { kind: "desktop", page: validatePage(data, input.limit) };
}

/** Read one instance by ID. Preview has no instances, so it is rejected. */
export async function getProvider(id: string): Promise<ProviderRecord> {
    if (!isTauri()) throw new ProviderRequestError("desktop_required");
    const record = validateRecord(
        await callProviderCommand("get_provider", { id }),
    );
    if (record.id !== id) throw invalidResponse();
    return record;
}

/**
 * Save a new instance together with its required API key. Success is reported
 * only for a valid acknowledgment of revision 1 with the requested kind and
 * protocol and no extra fields. The display name and URL may be normalized by
 * Rust, so the returned values are the ones to show. The key is not checked
 * here (Rust is the only validator) and is never part of an error.
 * `create_outcome_unknown` means the list must be re-read before retrying.
 */
export async function createProvider(
    input: CreateProviderInput,
): Promise<ProviderRecord> {
    if (!isTauri()) throw new ProviderRequestError("desktop_required");
    const record = validateRecord(
        await callProviderCommand("create_provider", {
            request: {
                kind: input.kind,
                displayName: input.displayName,
                baseUrl: input.baseUrl,
                protocol: input.protocol,
                extensions: input.extensions,
                secret: input.secret,
            },
        }),
    );
    if (
        record.revision !== 1 ||
        record.kind !== input.kind ||
        record.protocol !== input.protocol
    ) {
        throw invalidResponse();
    }
    return record;
}

/**
 * Edit an instance identified by `id`, never by its name. Success requires the
 * same ID, the requested protocol, and exactly the next revision. A
 * `revision_conflict` means another save happened first: reload the instance.
 */
export async function updateProvider(
    input: UpdateProviderInput,
): Promise<ProviderRecord> {
    if (!isTauri()) throw new ProviderRequestError("desktop_required");
    const record = validateRecord(
        await callProviderCommand("update_provider", {
            request: {
                id: input.id,
                expectedRevision: input.expectedRevision,
                displayName: input.displayName,
                baseUrl: input.baseUrl,
                protocol: input.protocol,
                extensions: input.extensions,
            },
            secret: input.secret ?? null,
        }),
    );
    if (
        record.id !== input.id ||
        record.revision !== input.expectedRevision + 1 ||
        record.protocol !== input.protocol
    ) {
        throw invalidResponse();
    }
    return record;
}
