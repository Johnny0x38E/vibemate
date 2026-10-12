import { beforeEach, expect, test, vi } from "vitest";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { AGENT_IDS } from "./agents";
import { probeAgentVersion, type VersionProbeInput } from "./agentVersions";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
const input: VersionProbeInput = {
    id: "pi",
    entryPath: "/home/test/entry with spaces ;",
    resolvedPath: "/home/test/cli",
    confirmed: true,
};
const response = {
    id: input.id,
    entryPath: input.entryPath,
    resolvedPath: input.resolvedPath,
    version: "1.1.0",
    evidence: "cliOutput",
};
beforeEach(() => {
    vi.mocked(isTauri).mockReset().mockReturnValue(true);
    vi.mocked(invoke).mockReset().mockResolvedValue(response);
});

test.each(AGENT_IDS)(
    "only sends approved identity and paths for %s",
    async (id) => {
        vi.mocked(invoke).mockResolvedValue({ ...response, id });
        expect(await probeAgentVersion({ ...input, id })).toEqual({
            ...response,
            id,
        });
        expect(invoke).toHaveBeenCalledExactlyOnceWith("probe_agent_version", {
            input: { ...input, id },
        });
    },
);
test("browser preview does not launch or invent a version", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    await expect(probeAgentVersion(input)).rejects.toMatchObject({
        code: "desktop_required",
    });
    expect(invoke).not.toHaveBeenCalled();
});
test.each([
    null,
    [],
    { ...response, id: "codex" },
    { ...response, entryPath: "/another/entry" },
    { ...response, resolvedPath: "/another/target" },
    { ...response, evidence: "packageMetadata" },
    { ...response, stdout: "synthetic-secret" },
    { ...response, stderr: "synthetic-secret" },
    { ...response, version: null },
    { ...response, version: "1.1.0 synthetic-token" },
    { ...response, version: "\u001b[31m1.1.0" },
    { ...response, version: "1.2.3+" },
    { ...response, version: `1.2.3-${"a".repeat(64)}` },
    { ...response, version: "1.1.0\n" },
])("rejects wrong or unsafe version response %#", async (value) => {
    vi.mocked(invoke).mockResolvedValue(value);
    await expect(probeAgentVersion(input)).rejects.toMatchObject({
        code: "invalid_response",
    });
});
test("keeps valid prerelease/build tokens without extra output", async () => {
    vi.mocked(invoke).mockResolvedValue({
        ...response,
        version: "1.2.3-dev.1+abc123",
    });
    expect((await probeAgentVersion(input)).version).toBe("1.2.3-dev.1+abc123");
});
test.each([
    "busy",
    "entry_changed",
    "unsupported_entry",
    "timed_out",
    "output_too_large",
    "unrecognized_output",
])("preserves safe failure %s", async (code) => {
    vi.mocked(invoke).mockRejectedValue(code);
    await expect(probeAgentVersion(input)).rejects.toMatchObject({ code });
});
test("raw IPC errors are never kept", async () => {
    vi.mocked(invoke).mockRejectedValue(new Error("synthetic-private-output"));
    await expect(probeAgentVersion(input)).rejects.toMatchObject({
        code: "operation_failed",
        message: "Agent version probe failed.",
    });
});
test.each([false, "true", 1, {}, null, undefined])(
    "requires literal boolean confirmation before IPC %#",
    async (confirmed) => {
        const request = { ...input };
        // Model malformed runtime data without weakening the production type.
        Reflect.set(request, "confirmed", confirmed);
        await expect(probeAgentVersion(request)).rejects.toMatchObject({
            code: "invalid_response",
        });
        expect(invoke).not.toHaveBeenCalled();
    },
);
test.each([
    { ...input, arguments: ["--help"] },
    { ...input, environment: { TEST_VALUE: "synthetic" } },
])("extra execution controls are rejected before IPC %#", async (request) => {
    await expect(probeAgentVersion(request)).rejects.toMatchObject({
        code: "invalid_response",
    });
    expect(invoke).not.toHaveBeenCalled();
});
test("unsafe input paths are rejected before IPC", async () => {
    await expect(
        probeAgentVersion({ ...input, entryPath: "relative" }),
    ).rejects.toMatchObject({ code: "invalid_response" });
    await expect(
        probeAgentVersion({ ...input, resolvedPath: "/path\u0000value" }),
    ).rejects.toMatchObject({ code: "invalid_response" });
    expect(invoke).not.toHaveBeenCalled();
});
