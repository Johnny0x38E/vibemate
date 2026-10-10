import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ProviderRequestError } from "./providers";
import {
    getProviderSecretStatus,
    replaceProviderSecret,
    type ProviderSecretStatus,
} from "./providerSecrets";

// Auto-mocking keeps invoke's generic signature without unsafe casts.
vi.mock(import("@tauri-apps/api/core"));

const ID = "0123456789abcdef0123456789abcdef";
// Synthetic; never a real key.
const SECRET = "sk-synthetic-replace-1111";

const set: ProviderSecretStatus = {
    providerId: ID,
    state: "set",
    updatedAtMs: 5000,
};
const missing: ProviderSecretStatus = {
    providerId: ID,
    state: "missing",
    updatedAtMs: null,
};

beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(isTauri).mockReturnValue(true);
});

async function failure(promise: Promise<unknown>): Promise<unknown> {
    return promise.then(
        () => new Error("Expected a failure"),
        (error: unknown) => error,
    );
}

describe("provider key desktop boundary", () => {
    it("preview reports preview for the status and refuses to replace, without IPC", async () => {
        vi.mocked(isTauri).mockReturnValue(false);
        expect(await getProviderSecretStatus(ID)).toEqual({ kind: "preview" });
        await expect(
            replaceProviderSecret({ providerId: ID, secret: SECRET }),
        ).rejects.toMatchObject({ code: "desktop_required" });
        expect(invoke).not.toHaveBeenCalled();
    });

    it("reads the status with the documented arguments", async () => {
        vi.mocked(invoke).mockResolvedValueOnce(set);
        expect(await getProviderSecretStatus(ID)).toEqual({
            kind: "desktop",
            status: set,
        });
        expect(invoke).toHaveBeenLastCalledWith("get_provider_secret_status", {
            providerId: ID,
        });
        vi.mocked(invoke).mockResolvedValueOnce(missing);
        expect(await getProviderSecretStatus(ID)).toEqual({
            kind: "desktop",
            status: missing,
        });
    });

    it("replaces with exactly providerId and secret and returns the new status", async () => {
        vi.mocked(invoke).mockResolvedValueOnce(set);
        expect(
            await replaceProviderSecret({ providerId: ID, secret: SECRET }),
        ).toEqual(set);
        expect(invoke).toHaveBeenLastCalledWith("replace_provider_secret", {
            request: { providerId: ID, secret: SECRET },
        });
    });

    it.each([
        null,
        [],
        "set",
        { ...set, providerId: "f".repeat(32) },
        { ...set, state: "unknown" },
        { ...set, updatedAtMs: null },
        { ...set, updatedAtMs: -1 },
        { ...set, updatedAtMs: 1.5 },
        { ...set, updatedAtMs: "5000" },
        { ...missing, updatedAtMs: 5000 },
        { providerId: ID, state: "set" },
        // A strict key set: an extra field, such as an echoed key, is rejected.
        { ...set, secret: SECRET },
        { ...missing, extra: true },
    ])("rejects a malformed status %j", async (response) => {
        vi.mocked(invoke).mockResolvedValue(response);
        await expect(getProviderSecretStatus(ID)).rejects.toMatchObject({
            code: "invalid_response",
        });
    });

    it("does not confirm a replacement acknowledged as missing or for another provider", async () => {
        for (const response of [
            missing,
            { ...set, providerId: "f".repeat(32) },
            { ...set, secret: SECRET },
        ]) {
            vi.mocked(invoke).mockResolvedValueOnce(response);
            await expect(
                replaceProviderSecret({ providerId: ID, secret: SECRET }),
            ).rejects.toMatchObject({ code: "invalid_response" });
        }
    });

    it.each([
        "invalid_request",
        "not_found",
        "secret_invalid",
        "storage_unavailable",
        "read_failed",
        "write_failed",
        "credential_store_unavailable",
        "credential_store_access_denied",
        "credential_store_failed",
        "secret_outcome_unknown",
        "operation_failed",
    ])("retains the known safe error code %s", async (code) => {
        vi.mocked(invoke).mockRejectedValue(code);
        await expect(getProviderSecretStatus(ID)).rejects.toMatchObject({
            code,
        });
        await expect(
            replaceProviderSecret({ providerId: ID, secret: SECRET }),
        ).rejects.toMatchObject({ code });
    });

    it("never puts the key into an error", async () => {
        for (const rejection of [
            "secret_invalid",
            `invalid args for replace_provider_secret: ${SECRET}`,
            new Error(SECRET),
        ]) {
            vi.mocked(invoke).mockRejectedValueOnce(rejection);
            const error = await failure(
                replaceProviderSecret({ providerId: ID, secret: SECRET }),
            );
            expect(error).toBeInstanceOf(ProviderRequestError);
            expect(String(error)).not.toContain(SECRET);
            expect(JSON.stringify(error)).not.toContain(SECRET);
            if (error instanceof Error)
                expect(error.stack ?? "").not.toContain(SECRET);
        }
    });
});
