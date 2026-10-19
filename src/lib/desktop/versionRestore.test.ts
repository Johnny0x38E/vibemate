import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";
import type { ConfigurableAgent } from "./agents";
import {
    VersionRestoreRequestError,
    applyVersionRestore,
    planVersionRestore,
    readVersionText,
    type ReviewedVersionFile,
    type VersionRestoreOutcome,
    type VersionRestorePlan,
    type VersionText,
} from "./versionRestore";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
const ID = "a".repeat(32);
const BEFORE = "b".repeat(64);
const AFTER = "c".repeat(64);
const REVISION = "100-1-0";
const PATH = "/Users/test/.claude/settings.json";

function file(
    overrides: Partial<VersionRestorePlan["files"][number]> = {},
): VersionRestorePlan["files"][number] {
    return {
        revisionId: REVISION,
        ordinal: 0,
        targetPath: PATH,
        action: "restore",
        reason: null,
        currentDigest: AFTER,
        targetDigest: BEFORE,
        restoredOwnerRef: null,
        summary: {
            format: "json",
            changes: [],
            truncated: false,
            reason: null,
        },
        ...overrides,
    };
}

/** A file with fields a strict schema must reject; the caller types it unknown. */
function rawFile(overrides: Record<string, unknown>): unknown {
    return { ...file(), ...overrides };
}

function plan(overrides: Partial<VersionRestorePlan> = {}): VersionRestorePlan {
    return {
        entryKey: `operation:${ID}`,
        operationId: ID,
        intent: "apply",
        ownerRef: ID,
        createdAt: 100,
        files: [file()],
        restorable: true,
        reason: null,
        ...overrides,
    };
}

function reviewed(): ReviewedVersionFile {
    return { revisionId: REVISION, currentDigest: AFTER };
}

function outcomeFile(): VersionRestoreOutcome["files"][number] {
    return {
        revisionId: REVISION,
        targetPath: PATH,
        action: "restore",
        state: "restored",
        errorCode: null,
    };
}

function outcome(
    overrides: Partial<VersionRestoreOutcome> = {},
): VersionRestoreOutcome {
    return {
        entryKey: `operation:${ID}`,
        files: [outcomeFile()],
        ...overrides,
    };
}

beforeEach(() => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(invoke).mockReset();
});

test.each<ConfigurableAgent>(["claudeCode", "pi", "grokBuild"])(
    "%s sends only the Agent and entry key for a preview",
    async (agent) => {
        const response = plan();
        vi.mocked(invoke).mockResolvedValue(response);
        expect(await planVersionRestore(agent, response.entryKey)).toEqual({
            kind: "desktop",
            plan: response,
        });
        expect(invoke).toHaveBeenCalledExactlyOnceWith(
            "plan_configuration_version_restore",
            { input: { agent, entryKey: response.entryKey } },
        );
    },
);

test("a batch plan keeps its non-apply intent", async () => {
    const response = plan({
        entryKey: "batch:direct-restore",
        intent: "version_restore",
    });
    vi.mocked(invoke).mockResolvedValue(response);
    expect(await planVersionRestore("pi", response.entryKey)).toEqual({
        kind: "desktop",
        plan: response,
    });
});

test("browser preview does not invent a version-restore plan", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(await planVersionRestore("pi", `operation:${ID}`)).toEqual({
        kind: "preview",
    });
    expect(invoke).not.toHaveBeenCalled();
});

test("applies exactly the reviewed revision and current digest", async () => {
    const response = outcome();
    vi.mocked(invoke).mockResolvedValue(response);
    expect(
        await applyVersionRestore("pi", `operation:${ID}`, [reviewed()]),
    ).toEqual(response);
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
        "apply_configuration_version_restore",
        {
            input: {
                agent: "pi",
                entryKey: `operation:${ID}`,
                reviewed: [reviewed()],
            },
        },
    );
});

test("browser apply requires the desktop app", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    await expect(
        applyVersionRestore("pi", `operation:${ID}`, [reviewed()]),
    ).rejects.toEqual(new VersionRestoreRequestError("desktop_required"));
    expect(invoke).not.toHaveBeenCalled();
});

test("rejects malformed previews as a whole", async () => {
    const base = plan();
    const blocked = file({
        action: "blocked",
        reason: "backup_unavailable",
        targetDigest: BEFORE,
        currentDigest: AFTER,
    });
    const invalid: unknown[] = [
        { ...base, extra: "synthetic-secret" },
        { ...base, operationId: "f".repeat(32) },
        { ...base, intent: "restore" },
        {
            ...base,
            entryKey: "batch:direct",
            intent: "apply",
        },
        {
            ...base,
            files: [file(), file({ revisionId: "other-revision" })],
        },
        {
            ...base,
            files: [file(), file({ ordinal: 1, revisionId: REVISION })],
        },
        { ...base, files: [{ ...blocked, reason: null }] },
        { ...base, files: [{ ...file(), reason: "backup_unavailable" }] },
        {
            ...base,
            files: [{ ...file(), revisionId: null }],
        },
        {
            ...base,
            files: [{ ...file(), targetDigest: null }],
        },
        {
            ...base,
            files: [{ ...file(), currentDigest: BEFORE }],
        },
        {
            ...base,
            files: [{ ...file(), action: "remove" }],
        },
        {
            ...base,
            files: [
                file({
                    action: "remove",
                    targetDigest: null,
                    currentDigest: null,
                    restoredOwnerRef: null,
                }),
            ],
        },
        {
            ...base,
            files: [{ ...file(), action: "skip", currentDigest: BEFORE }],
        },
        {
            ...base,
            files: [{ ...file(), targetPath: "relative" }],
        },
        {
            ...base,
            files: [
                file({
                    summary: {
                        format: "json",
                        changes: [
                            {
                                path: "a",
                                kind: "added",
                                before: "x",
                                after: "y",
                                secret: false,
                            },
                        ],
                        truncated: false,
                        reason: null,
                    },
                }),
            ],
        },
        {
            ...base,
            files: [
                file({
                    summary: {
                        format: "json",
                        changes: [],
                        truncated: false,
                        reason: "backup_unavailable",
                    },
                }),
                file({
                    revisionId: "other-revision",
                    ordinal: 1,
                    summary: {
                        format: "json",
                        changes: [
                            {
                                path: "a",
                                kind: "removed",
                                before: "x",
                                after: "y",
                                secret: false,
                            },
                        ],
                        truncated: false,
                        reason: null,
                    },
                }),
            ],
        },
        {
            ...base,
            files: [
                file({
                    summary: {
                        format: "json",
                        changes: [
                            {
                                path: "a",
                                kind: "replaced",
                                before: null,
                                after: "y",
                                secret: false,
                            },
                        ],
                        truncated: false,
                        reason: null,
                    },
                }),
            ],
        },
        {
            ...base,
            files: [
                rawFile({
                    summary: {
                        format: "json",
                        changes: [],
                        truncated: false,
                        reason: "backup_unavailable",
                        extra: true,
                    },
                }),
            ],
        },
        {
            ...base,
            files: [
                rawFile({
                    summary: {
                        format: "yaml",
                        changes: [],
                        truncated: false,
                        reason: null,
                    },
                }),
            ],
        },
        { ...base, restorable: false },
        { ...base, restorable: true, files: [blocked] },
        {
            ...base,
            reason: "history_unresolved",
            restorable: false,
        },
        {
            ...base,
            reason: "history_unresolved",
            files: [],
            restorable: true,
        },
    ];
    for (const value of invalid) {
        vi.mocked(invoke).mockResolvedValue(value);
        await expect(
            planVersionRestore("pi", `operation:${ID}`),
        ).rejects.toEqual(new VersionRestoreRequestError("invalid_response"));
    }
});

test("a blocked plan without changes is not restorable", async () => {
    const response = plan({
        files: [
            file({
                action: "blocked",
                reason: "backup_unavailable",
            }),
        ],
        restorable: false,
    });
    vi.mocked(invoke).mockResolvedValue(response);
    expect(await planVersionRestore("pi", `operation:${ID}`)).toEqual({
        kind: "desktop",
        plan: response,
    });
});

test("an unresolved journal is a valid non-restorable plan", async () => {
    const response = plan({
        files: [],
        restorable: false,
        reason: "history_unresolved",
    });
    vi.mocked(invoke).mockResolvedValue(response);
    expect(await planVersionRestore("pi", `operation:${ID}`)).toEqual({
        kind: "desktop",
        plan: response,
    });
});

test("rejects malformed outcomes and mismatched reviewed sets", async () => {
    const invalid: unknown[] = [
        { ...outcome(), extra: "synthetic-secret" },
        { ...outcome(), entryKey: `operation:${"f".repeat(32)}` },
        { ...outcome(), files: [] },
        {
            ...outcome(),
            files: [
                {
                    ...outcomeFile(),
                    state: "restored",
                    errorCode: "write_failed",
                },
            ],
        },
        {
            ...outcome(),
            files: [
                {
                    ...outcomeFile(),
                    state: "conflict",
                    errorCode: "write_failed",
                },
            ],
        },
        {
            ...outcome(),
            files: [
                {
                    ...outcomeFile(),
                    state: "unavailable",
                    errorCode: "target_changed",
                },
            ],
        },
        {
            ...outcome(),
            files: [
                {
                    ...outcomeFile(),
                    state: "unknown",
                    errorCode: "write_failed",
                },
            ],
        },
        {
            ...outcome(),
            files: [
                {
                    ...outcomeFile(),
                    state: "failed",
                    errorCode: "private-diagnostic",
                },
            ],
        },
        {
            ...outcome(),
            files: [
                {
                    ...outcomeFile(),
                    action: "skip",
                },
            ],
        },
        {
            ...outcome(),
            files: [
                {
                    ...outcomeFile(),
                    revisionId: "other-revision",
                },
            ],
        },
    ];
    for (const value of invalid) {
        vi.mocked(invoke).mockResolvedValue(value);
        await expect(
            applyVersionRestore("pi", `operation:${ID}`, [reviewed()]),
        ).rejects.toEqual(new VersionRestoreRequestError("invalid_response"));
    }
});

test("rejects invalid local requests before IPC", async () => {
    for (const [agent, entryKey] of [
        ["codex", `operation:${ID}`],
        ["pi", "not-an-entry"],
        ["pi", `operation:${"f".repeat(64)}`],
    ] as const) {
        await expect(
            planVersionRestore(agent as ConfigurableAgent, entryKey),
        ).rejects.toEqual(new VersionRestoreRequestError("invalid_request"));
    }
    for (const review of [
        [],
        [{ revisionId: "bad id", currentDigest: null }],
        [{ revisionId: REVISION, currentDigest: "short" }],
        [reviewed(), reviewed()],
    ]) {
        await expect(
            applyVersionRestore("pi", `operation:${ID}`, review),
        ).rejects.toEqual(new VersionRestoreRequestError("invalid_request"));
    }
    expect(invoke).not.toHaveBeenCalled();
});

function text(): VersionText {
    return {
        revisionId: REVISION,
        targetPath: PATH,
        format: "json",
        current: {
            state: "present",
            text: '{\n  "value": "after"\n}',
            bytes: 19,
        },
        target: {
            state: "present",
            text: '{\n  "value": "before"\n}',
            bytes: 20,
        },
    };
}

test("reads sanitized text with the preview digest as its token", async () => {
    const response = text();
    vi.mocked(invoke).mockResolvedValue(response);
    expect(
        await readVersionText("pi", `operation:${ID}`, REVISION, AFTER),
    ).toEqual({ kind: "desktop", text: response });
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
        "read_configuration_version_text",
        {
            input: {
                agent: "pi",
                entryKey: `operation:${ID}`,
                revisionId: REVISION,
                currentDigest: AFTER,
            },
        },
    );
});

test("browser text preview invents nothing", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(
        await readVersionText("pi", `operation:${ID}`, REVISION, AFTER),
    ).toEqual({ kind: "preview" });
    expect(invoke).not.toHaveBeenCalled();
});

test("rejects invalid text requests and responses", async () => {
    await expect(
        readVersionText("pi", `operation:${ID}`, "bad id", AFTER),
    ).rejects.toEqual(new VersionRestoreRequestError("invalid_request"));
    await expect(
        readVersionText("pi", `operation:${ID}`, REVISION, "short"),
    ).rejects.toEqual(new VersionRestoreRequestError("invalid_request"));
    await expect(
        readVersionText("pi", "not-entry", REVISION, AFTER),
    ).rejects.toEqual(new VersionRestoreRequestError("invalid_request"));

    const base = text();
    for (const value of [
        { ...base, extra: "synthetic-detail" },
        { ...base, revisionId: "other-revision" },
        { ...base, format: "yaml" },
        { ...base, targetPath: "relative" },
        {
            ...base,
            current: { state: "present", text: "x", bytes: -1 },
        },
        {
            ...base,
            current: { state: "unavailable", reason: "private-detail" },
        },
        {
            ...base,
            target: { state: "future-state" },
        },
    ]) {
        vi.mocked(invoke).mockResolvedValue(value);
        await expect(
            readVersionText("pi", `operation:${ID}`, REVISION, AFTER),
        ).rejects.toEqual(new VersionRestoreRequestError("invalid_response"));
    }
});

test("accepts absent and unavailable text sides", async () => {
    const response: VersionText = {
        ...text(),
        current: { state: "absent" },
        target: {
            state: "unavailable",
            reason: "backup_unavailable",
        },
    };
    vi.mocked(invoke).mockResolvedValue(response);
    expect(
        await readVersionText("pi", `operation:${ID}`, REVISION, null),
    ).toEqual({ kind: "desktop", text: response });
});

test("maps IPC failures to safe codes", async () => {
    for (const [error, code] of [
        ["plan_stale", "plan_stale"],
        ["storage_unavailable", "storage_unavailable"],
        ["unknown-runtime-diagnostic", "operation_failed"],
        [new Error("synthetic private detail"), "operation_failed"],
    ] as const) {
        vi.mocked(invoke).mockRejectedValue(error);
        await expect(
            planVersionRestore("pi", `operation:${ID}`),
        ).rejects.toEqual(new VersionRestoreRequestError(code));
    }
});
