import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";
import { discoverAgents, type AgentObservation } from "./agents";
vi.mock(import("@tauri-apps/api/core"));
const pi: AgentObservation = {
    id: "pi",
    state: "found",
    source: "manual",
    entryPath: "/home/test/pi",
    resolvedPath: "/home/test/package/dist/bundle/cli.js",
    version: "1.1.0",
    versionEvidence: "packageMetadata",
    capabilityEvidence: "documented",
    configDirectory: "/home/test/.pi/agent",
    directorySource: "default",
    files: ["settings.json", "models.json", "mcp.json"].map((name) => ({
        name,
        state: "missing",
    })),
};
const grok: AgentObservation = {
    ...pi,
    id: "grokBuild",
    source: "none",
    state: "notFound",
    entryPath: null,
    resolvedPath: null,
    version: null,
    versionEvidence: "none",
    capabilityEvidence: "unverified",
    configDirectory: "/home/test/.grok",
    files: ["config.toml", "managed_config.toml", "requirements.toml"].map(
        (name) => ({ name, state: "missing" }),
    ),
};
const input = { piPath: "/home/test/pi", grokPath: null };
beforeEach(() => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(invoke).mockReset().mockResolvedValue([pi, grok]);
});
test("browser preview never scans or invents installations", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(await discoverAgents(input)).toEqual({ kind: "preview" });
    expect(invoke).not.toHaveBeenCalled();
});
test("sends only selected paths and validates both adapter observations", async () => {
    expect(await discoverAgents(input)).toEqual({
        kind: "desktop",
        items: [pi, grok],
    });
    expect(invoke).toHaveBeenCalledExactlyOnceWith("discover_agents", {
        input,
    });
});
test.each([
    "invalid_path",
    "location_unavailable",
    "operation_failed",
    "private-path-and-secret",
    new Error("private-runtime-error"),
])("sanitizes failure %s", async (error) => {
    vi.mocked(invoke).mockRejectedValue(error);
    await expect(discoverAgents(input)).rejects.toMatchObject({
        code:
            error === "invalid_path" || error === "location_unavailable"
                ? error
                : "operation_failed",
        message: "Agent discovery failed.",
    });
});
test.each([
    null,
    [],
    [pi],
    [pi, pi],
    [grok, pi],
    [pi, grok, grok],
    [{ ...pi, secret: "synthetic-never-display" }, grok],
    [{ ...pi, entryPath: "relative/pi" }, grok],
    [{ ...pi, resolvedPath: "/tmp/unsafe\npi" }, grok],
    [{ ...pi, configDirectory: "/" + "x".repeat(4096) }, grok],
    [{ ...pi, version: "synthetic-secret" }, grok],
    [{ ...pi, version: "1.1.0\n" }, grok],
    [{ ...pi, version: "1.2.0" }, grok],
    [{ ...pi, versionEvidence: "filename" }, grok],
    [{ ...pi, state: "connected" }, grok],
    [{ ...pi, source: "none" }, grok],
    [{ ...pi, state: "notFound" }, grok],
    [{ ...pi, resolvedPath: null }, grok],
    [{ ...pi, configDirectory: null }, grok],
    [{ ...pi, files: [] }, grok],
    [
        { ...pi, files: pi.files.map((f) => ({ ...f, content: "private" })) },
        grok,
    ],
    [
        {
            ...pi,
            files: [
                { name: "auth.json", state: "present" },
                ...pi.files.slice(1),
            ],
        },
        grok,
    ],
    [
        {
            ...pi,
            files: [
                { name: "settings.json", state: "invalid" },
                ...pi.files.slice(1),
            ],
        },
        grok,
    ],
    [pi, { ...grok, capabilityEvidence: "documented" }],
])(
    "rejects malformed or contradictory response %#",
    async (response: unknown) => {
        vi.mocked(invoke).mockResolvedValue(response);
        await expect(discoverAgents(input)).rejects.toMatchObject({
            code: "invalid_response",
        });
    },
);
test("preserves spaces, Unicode, Windows drive and UNC paths as display data", async () => {
    for (const entryPath of [
        "/home/test user/入口/pi",
        "C:\\Users\\test user\\pi.cmd",
        "\\\\server\\share\\pi.cmd",
    ]) {
        vi.mocked(invoke).mockResolvedValue([
            { ...pi, entryPath, resolvedPath: entryPath },
            grok,
        ]);
        const result = await discoverAgents(input);
        expect(
            result.kind === "desktop" ? result.items[0]?.entryPath : null,
        ).toBe(entryPath);
    }
});
test("accepts unavailable directory and unverified filename evidence without declaring support", async () => {
    const row: AgentObservation = {
        ...grok,
        state: "found",
        source: "path",
        entryPath: "/home/test/grok",
        resolvedPath: "/home/test/grok-1.0.50-linux-x86_64",
        version: "1.0.50",
        versionEvidence: "filename",
        configDirectory: null,
        directorySource: "unavailable",
        files: grok.files.map((f) => ({ ...f, state: "unavailable" })),
    };
    vi.mocked(invoke).mockResolvedValue([pi, row]);
    expect(await discoverAgents(input)).toEqual({
        kind: "desktop",
        items: [pi, row],
    });
});
