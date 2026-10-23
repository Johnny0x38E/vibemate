import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";
import { MCP_SECRET_PLACEHOLDER, previewPiMcpRegistration } from "./mcpPreview";

vi.mock(import("@tauri-apps/api/core"));
const ID = "0123456789abcdef0123456789abcdef";
const proposal = {
    definitionId: ID,
    revision: 2,
    serverName: "tools",
    redactedEntry: {
        command: "not-a-server",
        args: ["literal space"],
        env: { TOKEN: MCP_SECRET_PLACEHOLDER },
        enabled: true,
    },
    sessionRelativeCwd: false,
    expandsHome: false,
};
const target = {
    path: "/tmp/isolated-agent/mcp.json",
    exists: false,
    contentDigest: null,
    symlink: false,
};
const response = { target, proposal };
beforeEach(() => {
    vi.mocked(invoke).mockReset().mockResolvedValue(response);
    vi.mocked(isTauri).mockReturnValue(true);
});

test("only identity/revision cross IPC and the redacted native shape is validated", async () => {
    expect(await previewPiMcpRegistration(ID, 2)).toEqual(response);
    expect(invoke).toHaveBeenCalledWith("preview_pi_mcp_registration", {
        request: { definitionId: ID, expectedRevision: 2 },
    });
});

test("browser preview refuses native reads without IPC", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    await expect(previewPiMcpRegistration(ID, 2)).rejects.toMatchObject({
        code: "desktop_required",
    });
    expect(invoke).not.toHaveBeenCalled();
});

test.each([
    { ...response, nativeDocument: { secret: "not-allowed" } },
    { ...response, proposal: { ...proposal, revision: 1 } },
    { ...response, proposal: { ...proposal, definitionId: "f".repeat(32) } },
    { ...response, proposal: { ...proposal, serverName: "bad name" } },
    { ...response, proposal: { ...proposal, expandsHome: "yes" } },
    { ...response, target: { ...target, path: "relative/mcp.json" } },
    { ...response, target: { ...target, exists: true } },
    { ...response, target: { ...target, contentDigest: "f".repeat(64) } },
    { ...response, target: { ...target, symlink: null } },
    {
        ...response,
        proposal: {
            ...proposal,
            redactedEntry: {
                ...proposal.redactedEntry,
                env: { TOKEN: "unmasked-secret" },
            },
        },
    },
    {
        ...response,
        proposal: {
            ...proposal,
            redactedEntry: { ...proposal.redactedEntry, headers: {} },
        },
    },
    {
        ...response,
        proposal: {
            ...proposal,
            redactedEntry: { ...proposal.redactedEntry, type: "sse" },
        },
    },
    {
        ...response,
        proposal: {
            ...proposal,
            redactedEntry: { ...proposal.redactedEntry, args: [1] },
        },
    },
    {
        ...response,
        proposal: {
            ...proposal,
            redactedEntry: {
                ...proposal.redactedEntry,
                env: { "BAD-NAME": MCP_SECRET_PLACEHOLDER },
            },
        },
    },
    {
        ...response,
        proposal: {
            ...proposal,
            redactedEntry: {
                url: "https://example.com/mcp",
                headers: { authorization: "unmasked-secret" },
                enabled: false,
            },
        },
    },
])(
    "discards malformed, mismatched or secret-bearing responses %#",
    async (value) => {
        vi.mocked(invoke).mockResolvedValue(value);
        await expect(previewPiMcpRegistration(ID, 2)).rejects.toMatchObject({
            code: "invalid_response",
        });
    },
);

test("accepts redacted HTTP and Windows target paths without mixing transports", async () => {
    const value = {
        target: {
            ...target,
            path: "C:\\Users\\test\\agent\\mcp.json",
            exists: true,
            contentDigest: "a".repeat(64),
        },
        proposal: {
            ...proposal,
            redactedEntry: {
                url: "https://example.com/mcp",
                headers: { authorization: MCP_SECRET_PLACEHOLDER },
                enabled: false,
            },
        },
    };
    vi.mocked(invoke).mockResolvedValue(value);
    expect(await previewPiMcpRegistration(ID, 2)).toEqual(value);
});

test("returns only allowlisted error codes, never raw native diagnostics", async () => {
    vi.mocked(invoke).mockRejectedValue("name_conflict");
    await expect(previewPiMcpRegistration(ID, 2)).rejects.toMatchObject({
        code: "name_conflict",
    });
    vi.mocked(invoke).mockRejectedValue({
        message: "private-token",
        path: "/private/native",
    });
    try {
        await previewPiMcpRegistration(ID, 2);
        throw new Error("expected failure");
    } catch (error: unknown) {
        expect(error).toMatchObject({ code: "operation_failed" });
        expect(JSON.stringify(error)).not.toContain("private-token");
    }
});
