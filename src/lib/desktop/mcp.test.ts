import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";
import {
    cleanupMcpCredentials,
    getMcpDefinition,
    listMcpDefinitions,
    saveMcpDefinition,
    McpRequestError,
    type McpRecord,
    type SaveMcpInput,
} from "./mcp";
vi.mock(import("@tauri-apps/api/core"));
const ID = "0123456789abcdef0123456789abcdef";
const row: McpRecord = {
    id: ID,
    displayName: "Tools",
    serverName: "tools",
    enabled: true,
    revision: 1,
    connection: {
        type: "stdio",
        command: "not-a-real-server",
        args: ["--stdio"],
        cwd: null,
        env: ["TOKEN"],
    },
    createdAtMs: 1,
    updatedAtMs: 1,
    cleanupPending: false,
};
const input: SaveMcpInput = {
    id: null,
    expectedRevision: null,
    displayName: "Tools",
    serverName: "tools",
    enabled: true,
    connection: {
        type: "stdio",
        command: "not-a-real-server",
        args: ["--stdio"],
        cwd: null,
        env: [{ name: "TOKEN", value: "synthetic-123" }],
    },
};
beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(isTauri).mockReturnValue(true);
});

test("preview reads no persisted definitions and rejects writes without IPC", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(await listMcpDefinitions(null)).toEqual({ kind: "preview" });
    await expect(saveMcpDefinition(input)).rejects.toMatchObject({
        code: "desktop_required",
    });
    await expect(cleanupMcpCredentials(ID)).rejects.toMatchObject({
        code: "desktop_required",
    });
    await expect(getMcpDefinition(ID)).rejects.toMatchObject({
        code: "desktop_required",
    });
    expect(invoke).not.toHaveBeenCalled();
});

test("list validates safe metadata and forwards bounded cursor arguments", async () => {
    vi.mocked(invoke).mockResolvedValue({ items: [row], nextCursor: null });
    expect(await listMcpDefinitions(null, 10)).toEqual({
        kind: "desktop",
        page: { items: [row], nextCursor: null },
    });
    expect(invoke).toHaveBeenCalledWith("list_mcp_definitions", {
        after: null,
        limit: 10,
    });
});

test.each([
    { ...row, id: "wrong" },
    { ...row, revision: 0 },
    { ...row, revision: 1.5 },
    { ...row, updatedAtMs: 0 },
    { ...row, enabled: "true" },
    { ...row, cleanupPending: null },
    { ...row, secret: "never return this" },
    {
        ...row,
        connection: {
            ...row.connection,
            env: [{ name: "TOKEN", value: "secret" }],
        },
    },
    {
        ...row,
        connection: { ...row.connection, credentialRef: "private-reference" },
    },
    {
        ...row,
        connection: {
            type: "sse",
            url: "https://example.com/mcp",
            headers: [],
        },
    },
    { ...row, connection: { ...row.connection, env: ["TOKEN", "TOKEN"] } },
])("rejects malformed or secret-bearing response %#", async (value) => {
    vi.mocked(invoke).mockResolvedValue({ items: [value], nextCursor: null });
    await expect(listMcpDefinitions(null)).rejects.toMatchObject({
        code: "invalid_response",
    });
});

test.each([
    { items: [row, row], nextCursor: null },
    { items: [row], nextCursor: "f".repeat(32) },
    { items: [row], nextCursor: 123 },
    { items: null, nextCursor: null },
])("rejects inconsistent page metadata %#", async (value) => {
    vi.mocked(invoke).mockResolvedValue(value);
    await expect(listMcpDefinitions(null)).rejects.toMatchObject({
        code: "invalid_response",
    });
});

test("validates HTTP headers as names only", async () => {
    const http = {
        ...row,
        connection: {
            type: "http",
            url: "https://example.com/mcp",
            headers: ["authorization"],
        },
    };
    vi.mocked(invoke).mockResolvedValue(http);
    expect(await getMcpDefinition(ID)).toEqual(http);
});

test("creates and edits with identity and revision acknowledgment checks", async () => {
    vi.mocked(invoke).mockResolvedValue(row);
    expect(await saveMcpDefinition(input)).toEqual(row);
    expect(invoke).toHaveBeenCalledWith("save_mcp_definition", {
        request: input,
    });
    vi.mocked(invoke).mockResolvedValue({ ...row, revision: 2 });
    expect(
        (await saveMcpDefinition({ ...input, id: ID, expectedRevision: 1 }))
            .revision,
    ).toBe(2);
    vi.mocked(invoke).mockResolvedValue(row);
    await expect(
        saveMcpDefinition({ ...input, id: ID, expectedRevision: 1 }),
    ).rejects.toMatchObject({ code: "invalid_response" });
});

test("get and cleanup refuse mismatched identities", async () => {
    vi.mocked(invoke).mockResolvedValue({ ...row, id: "f".repeat(32) });
    await expect(getMcpDefinition(ID)).rejects.toMatchObject({
        code: "invalid_response",
    });
    await expect(cleanupMcpCredentials(ID)).rejects.toMatchObject({
        code: "invalid_response",
    });
});

test("unknown runtime errors never retain their secret payload", async () => {
    const raw = "synthetic-error-secret-123";
    vi.mocked(invoke).mockRejectedValue(new Error(raw));
    try {
        await saveMcpDefinition(input);
        throw new Error("expected error");
    } catch (error: unknown) {
        expect(error).toBeInstanceOf(McpRequestError);
        expect(error).toMatchObject({ code: "operation_failed" });
        expect(String(error)).not.toContain(raw);
    }
    vi.mocked(invoke).mockRejectedValue("revision_conflict");
    await expect(saveMcpDefinition(input)).rejects.toMatchObject({
        code: "revision_conflict",
    });
});
