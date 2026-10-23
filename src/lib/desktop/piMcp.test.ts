import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";
import {
    applyPiMcpChange,
    previewPiMcpChange,
    readPiMcpState,
    readPiMcpHistory,
    recoverPiMcpChange,
} from "./piMcp";
import {
    emptyPiMcpState,
    piMcpDefinition,
    piMcpDeployment,
    piMcpHistory,
    piMcpPlan,
    piMcpRecovery,
} from "../../test/piMcpFixtures";
vi.mock(import("@tauri-apps/api/core"));
const selection = {
    definitionId: piMcpDefinition.id,
    expectedRevision: 1,
    action: "deploy",
    historyId: null,
} as const;
beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(isTauri).mockReturnValue(true);
});

test("review, confirm, observe, history and recovery expose only scoped requests", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(piMcpPlan);
    expect(await previewPiMcpChange(selection)).toEqual(piMcpPlan);
    expect(invoke).toHaveBeenLastCalledWith("preview_pi_mcp_change", {
        selection,
    });
    const outcome = {
        changeId: "b".repeat(32),
        registered: true,
        reloadRequired: true,
    };
    vi.mocked(invoke).mockResolvedValueOnce(outcome);
    expect(await applyPiMcpChange(selection, piMcpPlan)).toEqual(outcome);
    expect(invoke).toHaveBeenLastCalledWith("apply_pi_mcp_change", {
        request: { selection, approval: piMcpPlan.approval },
    });
    vi.mocked(invoke).mockResolvedValueOnce(emptyPiMcpState);
    await readPiMcpState();
    expect(invoke).toHaveBeenLastCalledWith("pi_mcp_state", {
        request: { definitionId: null, after: null, limit: 20 },
    });
    vi.mocked(invoke).mockResolvedValueOnce(piMcpHistory);
    await readPiMcpHistory(selection.definitionId);
    expect(invoke).toHaveBeenLastCalledWith("pi_mcp_history", {
        request: {
            definitionId: selection.definitionId,
            after: null,
            limit: 20,
        },
    });
    vi.mocked(invoke).mockResolvedValueOnce({
        ...outcome,
        changeId: piMcpRecovery.id,
    });
    await recoverPiMcpChange(piMcpRecovery);
    expect(invoke).toHaveBeenLastCalledWith("recover_pi_mcp_change", {
        request: {
            changeId: piMcpRecovery.id,
            approval: piMcpRecovery.approval,
        },
    });
});

test("browser mode refuses every native operation without invoking IPC", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    for (const operation of [
        () => previewPiMcpChange(selection),
        () => applyPiMcpChange(selection, piMcpPlan),
        () => readPiMcpState(),
        () => readPiMcpHistory(selection.definitionId),
        () => recoverPiMcpChange(piMcpRecovery),
    ])
        await expect(operation()).rejects.toMatchObject({
            code: "desktop_required",
        });
    expect(invoke).not.toHaveBeenCalled();
});

test.each([
    { ...piMcpPlan, nativeDocument: { private: "discard-this" } },
    { ...piMcpPlan, definitionId: "e".repeat(32) },
    { ...piMcpPlan, definitionRevision: 2 },
    { ...piMcpPlan, action: "revoke" },
    {
        ...piMcpPlan,
        target: { ...piMcpPlan.target, path: "relative/mcp.json" },
    },
    { ...piMcpPlan, target: { ...piMcpPlan.target, symlink: true } },
    { ...piMcpPlan, target: { ...piMcpPlan.target, exists: true } },
    { ...piMcpPlan, approval: "not-an-approval" },
    {
        ...piMcpPlan,
        afterEntry: {
            ...piMcpPlan.afterEntry,
            env: { CUSTOM: "discard-this" },
        },
    },
    {
        ...piMcpPlan,
        beforeEntry: {
            url: "https://example.com/mcp",
            headers: { "x-custom": "discard-this" },
            enabled: true,
        },
    },
    {
        ...piMcpPlan,
        summary: {
            ...piMcpPlan.summary,
            changes: [
                {
                    path: "mcpServers.tools.env.CUSTOM",
                    kind: "added",
                    before: null,
                    after: "discard-this",
                    secret: false,
                },
            ],
        },
    },
])("malformed or unmasked review is discarded %#", async (response) => {
    vi.mocked(invoke).mockResolvedValue(response);
    await expect(previewPiMcpChange(selection)).rejects.toMatchObject({
        code: "invalid_response",
        message: "Pi MCP request failed.",
    });
});

test.each([
    { ...emptyPiMcpState, nativeContent: "discard-this" },
    { ...emptyPiMcpState, deployments: [piMcpDeployment, piMcpDeployment] },
    {
        ...emptyPiMcpState,
        deployments: [{ ...piMcpDeployment, configRoot: "/another/root" }],
    },
    {
        ...emptyPiMcpState,
        deployments: [{ ...piMcpDeployment, state: "connected" }],
    },
    { ...emptyPiMcpState, nextCursor: "e".repeat(32) },
    {
        ...emptyPiMcpState,
        recovery: { ...piMcpRecovery, observation: "conflict" },
    },
])("contradictory root observations are rejected %#", async (response) => {
    vi.mocked(invoke).mockResolvedValue(response);
    await expect(readPiMcpState()).rejects.toMatchObject({
        code: "invalid_response",
    });
});

test("definition reads allow multiple recorded roots but reject another identity", async () => {
    const other = { ...piMcpDeployment, configRoot: "/older/pi" };
    vi.mocked(invoke).mockResolvedValue({
        ...emptyPiMcpState,
        deployments: [piMcpDeployment, other],
    });
    expect(
        (await readPiMcpState(selection.definitionId)).deployments,
    ).toHaveLength(2);
    vi.mocked(invoke).mockResolvedValue({
        ...emptyPiMcpState,
        deployments: [{ ...other, definitionId: "f".repeat(32) }],
    });
    await expect(readPiMcpState(selection.definitionId)).rejects.toMatchObject({
        code: "invalid_response",
    });
});

test("history validates page order, cursors, timestamps and unique results", async () => {
    const item = piMcpHistory.items[0];
    if (!item) throw new Error("Missing fixture");
    for (const response of [
        { items: [item, item], nextCursor: null },
        {
            items: [
                item,
                { ...item, id: "e".repeat(32), createdAt: item.createdAt + 1 },
            ],
            nextCursor: null,
        },
        { items: [item], nextCursor: "f".repeat(32) },
        {
            items: [{ ...item, createdAt: Number.MAX_SAFE_INTEGER }],
            nextCursor: null,
        },
    ]) {
        vi.mocked(invoke).mockResolvedValue(response);
        await expect(
            readPiMcpHistory(selection.definitionId),
        ).rejects.toMatchObject({ code: "invalid_response" });
    }
});

test("uncertain writes and recovery never invent a result", async () => {
    vi.mocked(invoke).mockResolvedValue({
        changeId: "b".repeat(32),
        registered: false,
        reloadRequired: true,
    });
    await expect(applyPiMcpChange(selection, piMcpPlan)).rejects.toMatchObject({
        code: "invalid_response",
    });
    vi.mocked(invoke).mockResolvedValue({
        changeId: "b".repeat(32),
        registered: true,
        reloadRequired: true,
    });
    await expect(recoverPiMcpChange(piMcpRecovery)).rejects.toMatchObject({
        code: "invalid_response",
    });
    vi.mocked(invoke).mockClear();
    await expect(
        recoverPiMcpChange({
            ...piMcpRecovery,
            approval: null,
            observation: "conflict",
        }),
    ).rejects.toMatchObject({ code: "recovery_required" });
    expect(invoke).not.toHaveBeenCalled();
});

test.each([
    "credential_unavailable",
    "plan_stale",
    "recovery_required",
    "snapshot_unavailable",
])("known safe failures survive: %s", async (error) => {
    vi.mocked(invoke).mockRejectedValue(error);
    await expect(previewPiMcpChange(selection)).rejects.toMatchObject({
        code: error,
    });
});

test.each([
    new Error("discard-this"),
    { message: "discard-this" },
    "discard-this",
    null,
])("unknown diagnostics never become user text %#", async (error) => {
    vi.mocked(invoke).mockRejectedValue(error);
    try {
        await previewPiMcpChange(selection);
        throw new Error("Expected rejection");
    } catch (error: unknown) {
        expect(error).toMatchObject({
            code: "outcome_unknown",
            message: "Pi MCP request failed.",
        });
        expect(JSON.stringify(error)).not.toContain("discard-this");
    }
});
