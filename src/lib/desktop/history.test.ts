import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";
import type { ConfigurableAgent } from "./agents";
import {
    HistoryRequestError,
    readConfigurationHistory,
    type HistoryEntry,
    type HistoryPage,
    type HistoryRevision,
} from "./history";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
const ID = "a".repeat(32);
const BEFORE = "b".repeat(64);
const AFTER = "c".repeat(64);
const SCOPE = "d".repeat(64);

function revision(): HistoryRevision {
    return {
        revisionId: "100-1-0",
        ordinal: 0,
        previousDigest: BEFORE,
        intendedDigest: AFTER,
        backupRecorded: true,
        backupState: "available",
        state: "completed",
        resolution: null,
        errorCode: null,
        createdAt: 100,
        updatedAt: 101,
    };
}

function entry(): HistoryEntry {
    return {
        entryKey: `operation:${ID}`,
        operationId: ID,
        intent: "apply",
        ownerRef: ID,
        ownerLabel: "DeepSeek Work",
        requiresResolution: false,
        sourceState: "applied",
        createdAt: 90,
        updatedAt: 110,
        sourceSteps: [
            {
                ordinal: 0,
                targetPath: "/Users/test/.pi/agent/models.json",
                expectedDigest: BEFORE,
                appliedDigest: AFTER,
                previousExisted: true,
                backupRecorded: true,
                backupState: "available",
                state: "applied",
                errorCode: null,
                restoreState: null,
            },
        ],
        revisions: [revision()],
    };
}

function current(status: HistoryPage["current"]["status"] = "unrecorded") {
    return {
        status,
        matchedEntryKey: status === "matched" ? `operation:${ID}` : null,
        files: [
            {
                targetPath: "/Users/test/.pi/agent/models.json",
                currentDigest: AFTER,
                readable: true,
            },
        ],
    } satisfies HistoryPage["current"];
}

function page(row = entry(), status: HistoryPage["current"]["status"] = "unrecorded"): HistoryPage {
    return {
        current:
            status === "matched"
                ? current("matched")
                : status === "unreadable"
                  ? {
                        status: "unreadable",
                        matchedEntryKey: null,
                        files: [
                            {
                                targetPath: "/Users/test/.pi/agent/models.json",
                                currentDigest: null,
                                readable: false,
                            },
                        ],
                    }
                  : current(status),
        entries: [row],
        nextCursor: null,
    };
}

beforeEach(() => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(invoke).mockReset();
});

test.each<ConfigurableAgent>(["claudeCode", "pi", "grokBuild"])(
    "%s sends no path and returns validated metadata only",
    async (agent) => {
        const response = page();
        vi.mocked(invoke).mockResolvedValue(response);
        expect(await readConfigurationHistory(agent)).toEqual({
            kind: "desktop",
            page: response,
        });
        expect(invoke).toHaveBeenCalledExactlyOnceWith(
            "configuration_history_page",
            { input: { agent, after: null, limit: 20 } },
        );
    },
);

test("accepts a direct version-restore batch entry", async () => {
    const batch: HistoryEntry = {
        ...entry(),
        entryKey: "batch:direct-restore",
        intent: "version_restore",
    };
    vi.mocked(invoke).mockResolvedValue(page(batch));
    expect(await readConfigurationHistory("pi")).toEqual({
        kind: "desktop",
        page: page(batch),
    });
});

test("browser preview does not manufacture an empty desktop history", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(await readConfigurationHistory("pi")).toEqual({ kind: "preview" });
    expect(invoke).not.toHaveBeenCalled();
});

test("passes a validated seek cursor unchanged and accepts the empty final page", async () => {
    const first = page();
    first.nextCursor = {
        scopeDigest: SCOPE,
        createdAt: first.entries[0]?.createdAt ?? 0,
        entryKey: `operation:${ID}`,
    };
    vi.mocked(invoke)
        .mockResolvedValueOnce(first)
        .mockResolvedValueOnce({ current: current("unrecorded"), entries: [], nextCursor: null });
    const result = await readConfigurationHistory("pi", null, 1);
    expect(result).toEqual({ kind: "desktop", page: first });
    if (result.kind !== "desktop") throw new Error("expected desktop history");
    expect(
        await readConfigurationHistory("pi", result.page.nextCursor, 1),
    ).toEqual({
        kind: "desktop",
        page: { current: current("unrecorded"), entries: [], nextCursor: null },
    });
    expect(invoke).toHaveBeenLastCalledWith("configuration_history_page", {
        input: { agent: "pi", after: first.nextCursor, limit: 1 },
    });
});

test.each([0, 51, -1, 1.5, NaN, Infinity, Number.MAX_SAFE_INTEGER])(
    "refuses invalid page size %s before IPC",
    async (limit) => {
        await expect(
            readConfigurationHistory("pi", null, limit),
        ).rejects.toEqual(new HistoryRequestError("invalid_request"));
        expect(invoke).not.toHaveBeenCalled();
    },
);

test("rejects an extended or malformed outgoing cursor without IPC", async () => {
    for (const cursor of [
        {
            scopeDigest: SCOPE,
            createdAt: 90,
            entryKey: `operation:${ID}`,
            root: "/elsewhere",
        },
        { scopeDigest: "short", createdAt: 90, entryKey: `operation:${ID}` },
        { scopeDigest: SCOPE, createdAt: -1, entryKey: `operation:${ID}` },
        { scopeDigest: SCOPE, createdAt: 90, entryKey: "batch:../../outside" },
    ]) {
        await expect(
            readConfigurationHistory("pi", cursor),
        ).rejects.toMatchObject({ code: "invalid_request" });
    }
    expect(invoke).not.toHaveBeenCalled();
});

const base = entry();
const step = base.sourceSteps[0];
if (step === undefined) throw new Error("missing source step fixture");
const rev = revision();
const audit = {
    previousState: "unknown",
    observedDigest: AFTER,
    currentMatch: "intended",
    resolvedAt: 105,
};
const invalidEntries: unknown[] = [
    { ...base, extra: "synthetic-secret" },
    { ...base, entryKey: `operation:${"f".repeat(32)}` },
    { ...base, entryKey: "batch:100-1-0" },
    { ...base, operationId: "not-an-id" },
    { ...base, intent: "replay" },
    { ...base, intent: "restore" },
    { ...base, sourceState: "effective" },
    { ...base, ownerRef: "unbounded".repeat(20) },
    { ...base, ownerRef: "unsafe\nreference" },
    { ...base, ownerLabel: "" },
    { ...base, ownerLabel: "unsafe\nname" },
    { ...base, ownerLabel: "x".repeat(65) },
    { ...base, requiresResolution: "false" },
    { ...base, createdAt: -1 },
    { ...base, createdAt: 1.5 },
    { ...base, updatedAt: 89 },
    { ...base, updatedAt: Number.MAX_SAFE_INTEGER + 1 },
    { ...base, sourceSteps: [] },
    { ...base, sourceSteps: [{ ...step, ordinal: 1 }] },
    { ...base, sourceSteps: [{ ...step, targetPath: "relative" }] },
    {
        ...base,
        sourceSteps: [{ ...step, backupPath: "/private/synthetic-secret" }],
    },
    { ...base, sourceSteps: [{ ...step, previousExisted: "true" }] },
    { ...base, sourceSteps: [{ ...step, expectedDigest: "bad" }] },
    {
        ...base,
        sourceSteps: [{ ...step, state: "applied", appliedDigest: null }],
    },
    {
        ...base,
        sourceSteps: [{ ...step, state: "applied", errorCode: "write_failed" }],
    },
    {
        ...base,
        sourceSteps: [
            { ...step, state: "pending", appliedDigest: null, errorCode: null },
        ],
    },
    {
        ...base,
        sourceSteps: [
            { ...step, state: "failed", appliedDigest: null, errorCode: null },
        ],
    },
    { ...base, sourceSteps: [{ ...step, restoreState: "effective" }] },
    { ...base, revisions: [rev, rev] },
    { ...base, revisions: [{ ...rev, ordinal: 1 }] },
    { ...base, revisions: [{ ...rev, revisionId: "../../private" }] },
    { ...base, revisions: [{ ...rev, backupPath: "/private" }] },
    { ...base, revisions: [{ ...rev, previousDigest: null }] },
    { ...base, revisions: [{ ...rev, backupRecorded: false, backupState: "not_needed" }] },
    {
        ...base,
        revisions: [{ ...rev, state: "prepared", errorCode: "write_failed" }],
    },
    { ...base, revisions: [{ ...rev, state: "failed" }] },
    {
        ...base,
        revisions: [{ ...rev, state: "failed", errorCode: "outcome_unknown" }],
    },
    { ...base, revisions: [{ ...rev, state: "unknown" }] },
    {
        ...base,
        revisions: [{ ...rev, state: "completed", errorCode: "write_failed" }],
    },
    {
        ...base,
        revisions: [{ ...rev, errorCode: "private-runtime-diagnostic" }],
    },
    { ...base, revisions: [{ ...rev, updatedAt: 99 }] },
    { ...base, revisions: [{ ...rev, updatedAt: 111 }] },
    {
        ...base,
        revisions: [{ ...rev, resolution: { ...audit, extra: "secret" } }],
    },
    {
        ...base,
        revisions: [
            { ...rev, resolution: { ...audit, previousState: "failed" } },
        ],
    },
    {
        ...base,
        revisions: [
            { ...rev, resolution: { ...audit, observedDigest: BEFORE } },
        ],
    },
    {
        ...base,
        revisions: [{ ...rev, resolution: { ...audit, currentMatch: "both" } }],
    },
    {
        ...base,
        revisions: [
            { ...rev, resolution: { ...audit, currentMatch: "previous" } },
        ],
    },
    {
        ...base,
        revisions: [{ ...rev, resolution: { ...audit, resolvedAt: -1 } }],
    },
];

test.each(invalidEntries)(
    "rejects malformed or contradictory history entry %#",
    async (row) => {
        vi.mocked(invoke).mockResolvedValue({
            entries: [row],
            nextCursor: null,
        });
        await expect(readConfigurationHistory("pi")).rejects.toEqual(
            new HistoryRequestError("invalid_response"),
        );
    },
);

test.each([
    null,
    { entries: [], nextCursor: null, config: "synthetic-secret" },
    { entries: [] },
    {
        entries: [],
        nextCursor: {
            scopeDigest: SCOPE,
            createdAt: 90,
            entryKey: base.entryKey,
        },
    },
    { entries: [base, base], nextCursor: null },
    {
        entries: [base],
        nextCursor: {
            scopeDigest: "bad",
            createdAt: 90,
            entryKey: base.entryKey,
        },
    },
    {
        entries: [base],
        nextCursor: {
            scopeDigest: SCOPE,
            createdAt: 91,
            entryKey: base.entryKey,
        },
    },
    {
        entries: [base],
        nextCursor: {
            scopeDigest: SCOPE,
            createdAt: 90,
            entryKey: base.entryKey,
            backupPath: "secret",
        },
    },
])("rejects malformed pages and cursor anchors %#", async (response) => {
    vi.mocked(invoke).mockResolvedValue(response);
    await expect(readConfigurationHistory("pi", null, 1)).rejects.toMatchObject(
        { code: "invalid_response" },
    );
});

test("rejects oversized, misordered and repeated pages without hiding bad rows", async () => {
    const older = {
        ...entry(),
        entryKey: `operation:${"1".repeat(32)}`,
        operationId: "1".repeat(32),
        revisions: [],
    };
    const cursor = {
        scopeDigest: SCOPE,
        createdAt: 90,
        entryKey: base.entryKey,
    };
    for (const response of [
        { entries: [older, base], nextCursor: null },
        { entries: [base, older], nextCursor: null },
        { entries: [base], nextCursor: null },
        {
            entries: [older],
            nextCursor: {
                ...cursor,
                scopeDigest: "e".repeat(64),
                entryKey: older.entryKey,
            },
        },
    ]) {
        vi.mocked(invoke).mockResolvedValue(response);
        await expect(
            readConfigurationHistory("pi", cursor, 1),
        ).rejects.toMatchObject({ code: "invalid_response" });
    }
    vi.mocked(invoke).mockResolvedValue({
        entries: [base],
        nextCursor: cursor,
    });
    // A continuation cannot be claimed on a short page.
    await expect(readConfigurationHistory("pi", null, 2)).rejects.toMatchObject(
        { code: "invalid_response" },
    );
    vi.mocked(invoke).mockResolvedValue({
        current: current("unrecorded"),
        entries: [base, older],
        nextCursor: null,
    });
    expect(await readConfigurationHistory("pi", null, 2)).toEqual({
        kind: "desktop",
        page: {
            current: current("unrecorded"),
            entries: [base, older],
            nextCursor: null,
        },
    });
});

test.each<HistoryRevision>([
    { ...rev, previousDigest: null, backupRecorded: false, backupState: "not_needed" },
    { ...rev, intendedDigest: null },
    { ...rev, state: "prepared" },
    { ...rev, state: "failed", errorCode: "plan_stale" },
    { ...rev, state: "unknown", errorCode: "outcome_unknown" },
    {
        ...rev,
        resolution: {
            previousState: "completed",
            observedDigest: AFTER,
            currentMatch: "intended",
            resolvedAt: 105,
        },
    },
    {
        ...rev,
        state: "failed",
        errorCode: "write_failed",
        resolution: {
            previousState: "prepared",
            observedDigest: BEFORE,
            currentMatch: "previous",
            resolvedAt: 105,
        },
    },
])(
    "preserves absence, interrupted results and reconciliation without inventing eligibility %#",
    async (revision) => {
        const response = page({ ...entry(), revisions: [revision] });
        vi.mocked(invoke).mockResolvedValue(response);
        expect(await readConfigurationHistory("pi")).toEqual({
            kind: "desktop",
            page: response,
        });
    },
);

test("retains legacy attempts and separate partial multi-file restore/compensation batches", async () => {
    const legacy = {
        ...entry(),
        revisions: [],
        ownerRef: null,
        sourceState: "failed" as const,
        sourceSteps: [
            {
                ordinal: 0,
                targetPath: "C:\\Users\\test\\.grok\\config.toml",
                expectedDigest: null,
                appliedDigest: null,
                previousExisted: null,
                backupRecorded: false,
                backupState: "not_needed",
                state: "failed" as const,
                errorCode: "write_failed" as const,
                restoreState: null,
            },
        ],
    };
    const legacyStep = legacy.sourceSteps[0];
    if (legacyStep === undefined)
        throw new Error("missing legacy step fixture");
    for (const intent of ["restore", "compensation"] as const) {
        const row: HistoryEntry = {
            ...entry(),
            entryKey: `batch:batch-${intent}`,
            intent,
            sourceState: "partial",
            sourceSteps: [{ ...step }, { ...legacyStep, ordinal: 1 }],
            // A batch may contain only a subset of the source files, and later
            // timestamps may go backwards; neither implies a missing revision.
            revisions:
                intent === "restore"
                    ? [{ ...revision(), ordinal: 1 }]
                    : [
                          revision(),
                          {
                              ...revision(),
                              revisionId: "100-1-1",
                              ordinal: 1,
                              createdAt: 95,
                              updatedAt: 99,
                          },
                      ],
        };
        const response = {
            current: current("unrecorded"),
            entries: [row, legacy],
            nextCursor: null,
        };
        // Batches have their own sort time rather than their source's time.
        row.createdAt = 100;
        vi.mocked(invoke).mockResolvedValue(response);
        expect(await readConfigurationHistory("pi")).toEqual({
            kind: "desktop",
            page: response,
        });
    }
});

test("rejects over-bound collections and missing nested fields", async () => {
    for (const response of [
        {
            entries: Array.from({ length: 51 }, () => entry()),
            nextCursor: null,
        },
        page({
            ...entry(),
            sourceSteps: Array.from({ length: 17 }, (_, ordinal) => ({
                ...step,
                ordinal,
            })),
        }),
        page({
            ...entry(),
            revisions: Array.from({ length: 17 }, (_, ordinal) => ({
                ...revision(),
                ordinal,
                revisionId: `100-1-${String(ordinal)}`,
            })),
        }),
        {
            entries: [
                {
                    ...entry(),
                    sourceSteps: [{ ...step, previousExisted: undefined }],
                },
            ],
            nextCursor: null,
        },
        {
            entries: [
                {
                    ...entry(),
                    revisions: [{ ...revision(), resolution: undefined }],
                },
            ],
            nextCursor: null,
        },
    ]) {
        vi.mocked(invoke).mockResolvedValue(response);
        await expect(
            readConfigurationHistory("pi", null, 50),
        ).rejects.toMatchObject({ code: "invalid_response" });
    }
});

test.each([
    "invalid_request",
    "storage_unavailable",
    "operation_failed",
] as const)("keeps documented %s errors", async (code) => {
    vi.mocked(invoke).mockRejectedValue(code);
    await expect(readConfigurationHistory("pi")).rejects.toEqual(
        new HistoryRequestError(code),
    );
});

test("discards transport and validation diagnostics including malicious field names", async () => {
    for (const error of [
        "synthetic-sensitive-diagnostic",
        { apiKey: "synthetic-sensitive-diagnostic" },
        new Error("synthetic-sensitive-diagnostic"),
    ]) {
        vi.mocked(invoke).mockRejectedValue(error);
        await expect(readConfigurationHistory("pi")).rejects.toEqual(
            new HistoryRequestError("operation_failed"),
        );
    }
    vi.mocked(invoke).mockResolvedValue({
        entries: [],
        nextCursor: null,
        "synthetic-sensitive-field": "synthetic-sensitive-value",
    });
    try {
        await readConfigurationHistory("pi");
        throw new Error("expected rejection");
    } catch (error: unknown) {
        expect(error).toEqual(new HistoryRequestError("invalid_response"));
        expect(String(error)).not.toContain("synthetic-sensitive");
        expect(JSON.stringify(error)).not.toContain("synthetic-sensitive");
        expect(error).not.toHaveProperty("cause");
        expect(error).not.toHaveProperty("issues");
    }
});
