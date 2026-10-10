import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
    addManualProviderModel,
    cancelProviderModelFetch,
    deleteManualProviderModel,
    fetchProviderModels,
    getProviderModelFetchStatus,
    listProviderModels,
    ModelRequestError,
    setProviderModelsSelected,
    type ProviderModel,
} from "./models";

vi.mock(import("@tauri-apps/api/core"));
const ID = "0123456789abcdef0123456789abcdef";
const model: ProviderModel = {
    providerId: ID,
    modelId: "mistral/mistral-large-4",
    source: "fetched",
    selected: false,
    alias: null,
    upstreamName: "Mistral Large",
    contextWindow: 128000,
    maxOutputTokens: null,
    inputModalities: ["text"],
    outputModalities: null,
    supportedEndpoints: ["/chat/completions"],
    routeSupport: "supported",
    upstreamState: "listed",
    lastSeenAtMs: 1000,
    missingSinceMs: null,
    createdAtMs: 1000,
    updatedAtMs: 1000,
};
const input = {
    providerId: ID,
    after: null,
    limit: 50,
    filter: "all",
} as const;
const page = { items: [model], nextCursor: null, totalMatches: null };
const summary = {
    complete: true,
    incompleteReason: null,
    listed: 1,
    added: 1,
    updated: 0,
    markedMissing: 0,
    pruned: 0,
    skippedInvalid: 0,
    fetchedAtMs: 1000,
};

beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(isTauri).mockReturnValue(true);
});

describe("model IPC boundary", () => {
    it("makes no requests on import or in preview and refuses every mutation", async () => {
        expect(invoke).not.toHaveBeenCalled();
        vi.mocked(isTauri).mockReturnValue(false);
        expect(await listProviderModels(input)).toEqual({ kind: "preview" });
        expect(await getProviderModelFetchStatus(ID)).toEqual({
            kind: "preview",
        });
        expect(await cancelProviderModelFetch(ID)).toEqual({
            wasRunning: false,
        });
        for (const operation of [
            () => fetchProviderModels(ID),
            () =>
                setProviderModelsSelected({
                    providerId: ID,
                    modelIds: [model.modelId],
                    selected: true,
                }),
            () =>
                addManualProviderModel({
                    providerId: ID,
                    modelId: "manual",
                    alias: null,
                }),
            () =>
                deleteManualProviderModel({
                    providerId: ID,
                    modelId: "manual",
                }),
        ])
            await expect(operation()).rejects.toMatchObject({
                code: "desktop_required",
            });
        expect(invoke).not.toHaveBeenCalled();
    });

    it("forwards cursor/filter and optional search without transforming model IDs", async () => {
        vi.mocked(invoke).mockResolvedValueOnce({
            ...page,
            nextCursor: "6d69737472616c",
        });
        expect(await listProviderModels(input)).toEqual({
            kind: "desktop",
            page: { ...page, nextCursor: "6d69737472616c" },
        });
        expect(invoke).toHaveBeenLastCalledWith("list_provider_models", {
            request: input,
        });
        const search = { ...input, query: "mistral large" };
        vi.mocked(invoke).mockResolvedValueOnce({ ...page, totalMatches: 5 });
        expect(await listProviderModels(search)).toEqual({
            kind: "desktop",
            page: { ...page, totalMatches: 5 },
        });
        expect(invoke).toHaveBeenLastCalledWith("list_provider_models", {
            request: search,
        });
    });

    it("requires a matching selection acknowledgment and accepts Rust's deduplication", async () => {
        const request = {
            providerId: ID,
            modelIds: [model.modelId, model.modelId],
            selected: true,
        };
        vi.mocked(invoke).mockResolvedValueOnce([{ ...model, selected: true }]);
        expect(await setProviderModelsSelected(request)).toEqual([
            { ...model, selected: true },
        ]);
        expect(invoke).toHaveBeenLastCalledWith(
            "set_provider_models_selected",
            { request },
        );
        for (const response of [
            [],
            [model],
            [{ ...model, selected: true, modelId: "other" }],
            [{ ...model, selected: true, providerId: "f".repeat(32) }],
        ]) {
            vi.mocked(invoke).mockResolvedValueOnce(response);
            await expect(
                setProviderModelsSelected(request),
            ).rejects.toMatchObject({ code: "invalid_response" });
        }
    });

    it("accepts normalized manual IDs and validates add/delete acknowledgments", async () => {
        const request = {
            providerId: ID,
            modelId: "  manual/model  ",
            alias: " Mine ",
        };
        const manual = {
            ...model,
            modelId: "manual/model",
            source: "manual",
            selected: true,
            alias: "Mine",
        };
        vi.mocked(invoke).mockResolvedValueOnce(manual);
        expect(await addManualProviderModel(request)).toEqual(manual);
        expect(invoke).toHaveBeenLastCalledWith("add_manual_provider_model", {
            request,
        });
        for (const response of [
            model,
            { ...manual, modelId: "other" },
            { ...manual, selected: false },
        ]) {
            vi.mocked(invoke).mockResolvedValueOnce(response);
            await expect(addManualProviderModel(request)).rejects.toMatchObject(
                { code: "invalid_response" },
            );
        }
        const deletion = { providerId: ID, modelId: "manual/model" };
        vi.mocked(invoke).mockResolvedValueOnce(null);
        await expect(
            deleteManualProviderModel(deletion),
        ).resolves.toBeUndefined();
        expect(invoke).toHaveBeenLastCalledWith(
            "delete_manual_provider_model",
            { request: deletion },
        );
        vi.mocked(invoke).mockResolvedValueOnce({
            secret: "synthetic-private-value",
        });
        await expect(deleteManualProviderModel(deletion)).rejects.toMatchObject(
            { code: "invalid_response" },
        );
    });

    it("fetches only when called and validates complete/incomplete summaries", async () => {
        vi.mocked(invoke).mockResolvedValueOnce(summary);
        expect(await fetchProviderModels(ID)).toEqual(summary);
        expect(invoke).toHaveBeenLastCalledWith("fetch_provider_models", {
            providerId: ID,
        });
        for (const reason of ["model_limit", "page_limit", "empty_list"]) {
            const incomplete = {
                ...summary,
                complete: false,
                incompleteReason: reason,
            };
            vi.mocked(invoke).mockResolvedValueOnce(incomplete);
            expect(await fetchProviderModels(ID)).toEqual(incomplete);
        }
    });

    it("reads status and reports both cancellation results with exact argument names", async () => {
        for (const lastFetch of [
            null,
            { fetchedAtMs: 1000, complete: false, listedCount: 5 },
        ]) {
            vi.mocked(invoke).mockResolvedValueOnce({
                running: true,
                lastFetch,
            });
            expect(await getProviderModelFetchStatus(ID)).toEqual({
                kind: "desktop",
                status: { running: true, lastFetch },
            });
            expect(invoke).toHaveBeenLastCalledWith(
                "get_provider_model_fetch_status",
                { providerId: ID },
            );
        }
        for (const wasRunning of [true, false]) {
            vi.mocked(invoke).mockResolvedValueOnce({ wasRunning });
            expect(await cancelProviderModelFetch(ID)).toEqual({ wasRunning });
            expect(invoke).toHaveBeenLastCalledWith(
                "cancel_provider_model_fetch",
                { providerId: ID },
            );
        }
    });

    it.each([
        ["providerId", "invalid"],
        ["providerId", "f".repeat(32)],
        ["modelId", null],
        ["modelId", ""],
        ["source", "unknown"],
        ["selected", 1],
        ["alias", {}],
        ["upstreamName", []],
        ["contextWindow", -1],
        ["maxOutputTokens", 1.5],
        ["inputModalities", [1]],
        ["outputModalities", "text"],
        ["supportedEndpoints", [null]],
        ["routeSupport", "yes"],
        ["upstreamState", "available"],
        ["lastSeenAtMs", NaN],
        ["missingSinceMs", -1],
        ["createdAtMs", Number.MAX_SAFE_INTEGER + 1],
        ["updatedAtMs", 999],
    ])("rejects malformed model field %s", async (field, value) => {
        vi.mocked(invoke).mockResolvedValue({
            ...page,
            items: [{ ...model, [field]: value }],
        });
        await expect(listProviderModels(input)).rejects.toMatchObject({
            code: "invalid_response",
        });
    });

    it("rejects each missing model field and unknown nested/top-level fields", async () => {
        for (const key of Object.keys(model)) {
            const partial = Object.fromEntries(
                Object.entries(model).filter(([field]) => field !== key),
            );
            vi.mocked(invoke).mockResolvedValueOnce({
                ...page,
                items: [partial],
            });
            await expect(listProviderModels(input)).rejects.toMatchObject({
                code: "invalid_response",
            });
        }
        for (const response of [
            { ...page, secret: "synthetic-private-value" },
            {
                ...page,
                items: [{ ...model, secret: "synthetic-private-value" }],
            },
            { ...page, items: [model, model] },
            { ...page, nextCursor: 1 },
            { ...page, totalMatches: -1 },
            { ...page, totalMatches: 0 },
            { ...page, totalMatches: 1, nextCursor: "cursor" },
            null,
            {},
            { ...page, items: null },
        ]) {
            vi.mocked(invoke).mockResolvedValueOnce(response);
            await expect(listProviderModels(input)).rejects.toMatchObject({
                code: "invalid_response",
            });
        }
        vi.mocked(invoke).mockResolvedValueOnce(page);
        await expect(
            listProviderModels({ ...input, limit: 0 }),
        ).rejects.toMatchObject({ code: "invalid_response" });
        vi.mocked(invoke).mockResolvedValueOnce(page);
        await expect(
            listProviderModels({ ...input, filter: "selected" }),
        ).rejects.toMatchObject({ code: "invalid_response" });
    });

    it("accepts nullable metadata and each supported source/route/upstream enum", async () => {
        for (const routeSupport of [
            "supported",
            "unsupported",
            "unknown",
            "not_applicable",
        ]) {
            const row = {
                ...model,
                source: "manual",
                routeSupport,
                upstreamState: "never_listed",
                contextWindow: null,
                inputModalities: null,
                supportedEndpoints: null,
                lastSeenAtMs: null,
            };
            vi.mocked(invoke).mockResolvedValueOnce({ ...page, items: [row] });
            expect(await listProviderModels(input)).toEqual({
                kind: "desktop",
                page: { ...page, items: [row] },
            });
        }
        const missing = {
            ...model,
            upstreamState: "missing",
            missingSinceMs: 2000,
        };
        vi.mocked(invoke).mockResolvedValueOnce({ ...page, items: [missing] });
        expect(await listProviderModels(input)).toMatchObject({
            page: { items: [missing] },
        });
    });

    it.each([
        { ...summary, extra: null },
        { ...summary, complete: "true" },
        { ...summary, incompleteReason: "timeout" },
        { ...summary, incompleteReason: "page_limit" },
        { ...summary, complete: false },
        { ...summary, listed: 2 },
        ...[
            "listed",
            "added",
            "updated",
            "markedMissing",
            "pruned",
            "skippedInvalid",
            "fetchedAtMs",
        ].map((key) => ({ ...summary, [key]: -1 })),
        null,
        {},
    ])("rejects malformed fetch summaries %#", async (response) => {
        vi.mocked(invoke).mockResolvedValue(response);
        await expect(fetchProviderModels(ID)).rejects.toMatchObject({
            code: "invalid_response",
        });
    });

    it.each([
        null,
        {},
        { running: "yes", lastFetch: null },
        { running: true, lastFetch: {} },
        {
            running: true,
            lastFetch: { fetchedAtMs: 1, complete: true, listedCount: -1 },
        },
        {
            running: true,
            lastFetch: {
                fetchedAtMs: 1,
                complete: true,
                listedCount: 1,
                secret: "synthetic-private-value",
            },
        },
        { running: false, lastFetch: null, secret: "synthetic-private-value" },
    ])("rejects malformed status %#", async (response) => {
        vi.mocked(invoke).mockResolvedValue(response);
        await expect(getProviderModelFetchStatus(ID)).rejects.toMatchObject({
            code: "invalid_response",
        });
    });

    it.each([null, {}, { wasRunning: 1 }, { wasRunning: true, extra: null }])(
        "rejects malformed cancellation %#",
        async (response) => {
            vi.mocked(invoke).mockResolvedValue(response);
            await expect(cancelProviderModelFetch(ID)).rejects.toMatchObject({
                code: "invalid_response",
            });
        },
    );

    it.each([
        "storage_unavailable",
        "read_failed",
        "write_failed",
        "operation_failed",
        "invalid_request",
        "not_found",
        "invalid_stored_provider",
        "invalid_stored_model",
        "model_id_invalid",
        "model_alias_invalid",
        "model_already_exists",
        "model_not_found",
        "model_route_not_supported",
        "secret_missing",
        "secret_invalid",
        "base_url_invalid",
        "credential_store_unavailable",
        "credential_store_access_denied",
        "credential_store_failed",
        "model_fetch_in_progress",
        "model_fetch_cancelled",
        "model_fetch_stale",
        "connection_failed",
        "tls_failed",
        "request_timed_out",
        "auth_rejected",
        "insufficient_balance",
        "rate_limited",
        "upstream_unavailable",
        "upstream_response_invalid",
        "response_too_large",
    ])("retains only the allowlisted Rust code %s", async (code) => {
        vi.mocked(invoke).mockRejectedValue(code);
        await expect(fetchProviderModels(ID)).rejects.toMatchObject({ code });
        await expect(listProviderModels(input)).rejects.toMatchObject({ code });
    });

    it("discards raw errors, structured payloads and frontend-only codes", async () => {
        const secret = "synthetic-private-value";
        for (const rejection of [
            new Error(secret),
            `request failed: ${secret}`,
            { code: "auth_rejected", secret },
            "desktop_required",
            "invalid_response",
        ]) {
            vi.mocked(invoke).mockRejectedValueOnce(rejection);
            const error: unknown = await fetchProviderModels(ID).catch(
                (failure: unknown) => failure,
            );
            expect(error).toBeInstanceOf(ModelRequestError);
            expect(error).toMatchObject({
                code: "operation_failed",
                message: "Model request failed.",
            });
            expect(String(error)).not.toContain(secret);
            expect(JSON.stringify(error)).not.toContain(secret);
            if (error instanceof Error)
                expect(error.stack).not.toContain(secret);
        }
    });
});
