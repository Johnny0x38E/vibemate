import { isTauri } from "@tauri-apps/api/core";
import {
    callProviderCommand,
    hasExactKeys,
    invalidResponse,
    isObject,
    ProviderRequestError,
} from "./providers";

/**
 * Whether a provider has an API key, as recorded in SQLite. `missing` only
 * occurs for configurations saved before keys existed (or after an unknown
 * create outcome); replacing the key then sets it.
 */
export type ProviderSecretState = "set" | "missing";

/** Key status from Rust. It never contains the key or any part of it. */
export interface ProviderSecretStatus {
    providerId: string;
    state: ProviderSecretState;
    /** Unix epoch milliseconds of the last key save; `null` when missing. */
    updatedAtMs: number | null;
}

/** A new key for an existing provider, identified by ID only. */
export interface ReplaceProviderSecretInput {
    providerId: string;
    /** Held only for this call; the caller clears its copy afterwards. */
    secret: string;
}

/** Preview has no Rust runtime and therefore no key status. */
export type ProviderSecretStatusResult =
    { kind: "preview" } | { kind: "desktop"; status: ProviderSecretStatus };

const STATUS_KEYS = ["providerId", "state", "updatedAtMs"] as const;

/**
 * Accept exactly `{ providerId, state, updatedAtMs }` for the requested ID:
 * `set` carries a non-negative integer time, `missing` carries `null`. Any
 * other shape, including an extra field, is `invalid_response`.
 */
function validateStatus(
    value: unknown,
    providerId: string,
): ProviderSecretStatus {
    if (!isObject(value) || !hasExactKeys(value, STATUS_KEYS)) {
        throw invalidResponse();
    }
    const { state, updatedAtMs } = value;
    if (value["providerId"] !== providerId) throw invalidResponse();
    if (
        state === "set" &&
        typeof updatedAtMs === "number" &&
        Number.isSafeInteger(updatedAtMs) &&
        updatedAtMs >= 0
    ) {
        return { providerId, state, updatedAtMs };
    }
    if (state === "missing" && updatedAtMs === null) {
        return { providerId, state, updatedAtMs: null };
    }
    throw invalidResponse();
}

/**
 * Read whether a provider has an API key. Rust reads SQLite only, so this never
 * opens the OS credential store or shows a system prompt. Preview returns
 * `preview` without invoking IPC.
 */
export async function getProviderSecretStatus(
    providerId: string,
): Promise<ProviderSecretStatusResult> {
    if (!isTauri()) return { kind: "preview" };
    const data = await callProviderCommand("get_provider_secret_status", {
        providerId,
    });
    return { kind: "desktop", status: validateStatus(data, providerId) };
}

/**
 * Replace a provider's API key, or set it when it is missing. Success requires
 * a valid `set` status for the same provider. The key is not checked here
 * (Rust is the only validator) and never appears in an error. After
 * `secret_outcome_unknown`, `operation_failed` or `invalid_response`, the
 * outcome is unknown: re-read the status before claiming anything. Preview
 * rejects with `desktop_required` without invoking IPC.
 */
export async function replaceProviderSecret(
    input: ReplaceProviderSecretInput,
): Promise<ProviderSecretStatus> {
    if (!isTauri()) throw new ProviderRequestError("desktop_required");
    const status = validateStatus(
        await callProviderCommand("replace_provider_secret", {
            request: { providerId: input.providerId, secret: input.secret },
        }),
        input.providerId,
    );
    if (status.state !== "set") throw invalidResponse();
    return status;
}
