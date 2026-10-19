import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import {
    planAgentRestore,
    type ClaudeCodeRestorePlan,
    type RecordedOperation,
    type RestorePlanStep,
} from "../../lib/desktop/agents";
import {
    HistoryRequestError,
    readConfigurationHistory,
    type HistoryCursor,
    type HistoryEntry,
    type HistoryPage,
} from "../../lib/desktop/history";
import {
    VersionRestoreRequestError,
    applyVersionRestore,
    planVersionRestore,
    type VersionRestoreOutcome,
    type VersionRestorePlan,
} from "../../lib/desktop/versionRestore";
import {
    deleteHistoryEntry,
    HistoryDeleteRequestError,
} from "../../lib/desktop/historyDelete";
import { RestoreView } from "./RestoreView";

vi.mock(import("../../lib/desktop/agents"), async (original) => ({
    ...(await original()),
    applyAgentRestore: vi.fn(),
    planAgentRestore: vi.fn(),
}));
vi.mock(import("../../lib/desktop/history"), async (original) => ({
    ...(await original()),
    readConfigurationHistory: vi.fn(),
}));
vi.mock(import("../../lib/desktop/versionRestore"), async (original) => ({
    ...(await original()),
    applyVersionRestore: vi.fn(),
    planVersionRestore: vi.fn(),
    readVersionText: vi.fn(),
}));
vi.mock(import("../../lib/desktop/historyDelete"), async (original) => ({
    ...(await original()),
    deleteHistoryEntry: vi.fn(),
}));

const SETTINGS = "/Users/test/.claude/settings.json";
const BEFORE_DIGEST = "a".repeat(64);
const APPLIED_DIGEST = "b".repeat(64);
const CURRENT_DIGEST = "c".repeat(64);
const OPERATION_ID = "fedcba9876543210fedcba9876543210";
const PROVIDER_ID = "0123456789abcdef0123456789abcdef";
const REVISION = "100-1-0";
const SECRET = "••••••••";
const CURSOR: HistoryCursor = {
    scopeDigest: "d".repeat(64),
    createdAt: 1_700_000_000_000,
    entryKey: `operation:${OPERATION_ID}`,
};

const entry: HistoryEntry = {
    entryKey: `operation:${OPERATION_ID}`,
    operationId: OPERATION_ID,
    intent: "apply",
    ownerRef: PROVIDER_ID,
    ownerLabel: "DeepSeek Work",
    requiresResolution: false,
    sourceState: "applied",
    createdAt: 1_700_000_000_000,
    updatedAt: 1_700_000_000_500,
    sourceSteps: [
        {
            ordinal: 0,
            targetPath: SETTINGS,
            expectedDigest: BEFORE_DIGEST,
            appliedDigest: APPLIED_DIGEST,
            previousExisted: true,
            backupRecorded: true,
            backupState: "available",
            state: "applied",
            errorCode: null,
            restoreState: null,
        },
    ],
    revisions: [
        {
            revisionId: REVISION,
            ordinal: 0,
            previousDigest: BEFORE_DIGEST,
            intendedDigest: APPLIED_DIGEST,
            backupRecorded: true,
            backupState: "available",
            state: "completed",
            resolution: null,
            errorCode: null,
            createdAt: 1_700_000_000_000,
            updatedAt: 1_700_000_000_500,
        },
    ],
};

const versionFile: VersionRestorePlan["files"][number] = {
    revisionId: REVISION,
    ordinal: 0,
    targetPath: SETTINGS,
    action: "restore",
    reason: null,
    currentDigest: CURRENT_DIGEST,
    targetDigest: APPLIED_DIGEST,
    restoredOwnerRef: null,
    summary: {
        format: "json",
        changes: [
            {
                path: "env.ANTHROPIC_API_KEY",
                kind: "replaced",
                before: SECRET,
                after: SECRET,
                secret: true,
            },
            {
                path: "model",
                kind: "replaced",
                before: "current-model",
                after: "retained-model",
                secret: false,
            },
        ],
        truncated: false,
        reason: null,
    },
};

const versionPlan: VersionRestorePlan = {
    entryKey: entry.entryKey,
    operationId: OPERATION_ID,
    intent: "apply",
    ownerRef: PROVIDER_ID,
    createdAt: entry.createdAt,
    files: [versionFile],
    restorable: true,
    reason: null,
};

const versionOutcome: VersionRestoreOutcome = {
    entryKey: entry.entryKey,
    files: [
        {
            revisionId: REVISION,
            targetPath: SETTINGS,
            action: "restore",
            state: "restored",
            errorCode: null,
        },
    ],
};

const legacyOperation: RecordedOperation = {
    operationId: OPERATION_ID,
    state: "applied",
    providerId: PROVIDER_ID,
    providerLabel: "DeepSeek Work",
    createdAt: 1_700_000_000_000,
    updatedAt: 1_700_000_000_500,
    restorable: true,
    requiresResolution: false,
    steps: [
        {
            ordinal: 0,
            targetPath: SETTINGS,
            state: "applied",
            restoreState: null,
        },
    ],
};

const legacyStep: RestorePlanStep = {
    ordinal: 0,
    targetPath: SETTINGS,
    action: "restore",
    reason: null,
    currentDigest: APPLIED_DIGEST,
    appliedDigest: APPLIED_DIGEST,
    restoreState: null,
    restoredProviderId: PROVIDER_ID,
    restoredProviderLabel: "DeepSeek Personal",
};

const legacyPlan: ClaudeCodeRestorePlan = {
    operationId: OPERATION_ID,
    state: "applied",
    createdAt: legacyOperation.createdAt,
    restorable: true,
    providerId: PROVIDER_ID,
    providerLabel: "DeepSeek Work",
    steps: [legacyStep],
};

function currentConfig(
    status: HistoryPage["current"]["status"] = "matched",
    matchedEntryKey: string | null = entry.entryKey,
): HistoryPage["current"] {
    return {
        status,
        matchedEntryKey: status === "matched" ? matchedEntryKey : null,
        files: [
            {
                targetPath: SETTINGS,
                currentDigest: status === "unreadable" ? null : APPLIED_DIGEST,
                readable: status !== "unreadable",
            },
        ],
    };
}

function page(
    entries: HistoryEntry[] = [entry],
    nextCursor: HistoryCursor | null = null,
    current: HistoryPage["current"] = currentConfig(),
): HistoryPage {
    return { current, entries, nextCursor };
}

const loadHistory = vi.mocked(readConfigurationHistory);
const previewVersion = vi.mocked(planVersionRestore);
const restoreVersion = vi.mocked(applyVersionRestore);
const previewLegacy = vi.mocked(planAgentRestore);
const removeEntry = vi.mocked(deleteHistoryEntry);

beforeEach(() => {
    loadHistory
        .mockReset()
        .mockResolvedValue({ kind: "desktop", page: page() });
    previewVersion
        .mockReset()
        .mockResolvedValue({ kind: "desktop", plan: versionPlan });
    restoreVersion.mockReset().mockResolvedValue(versionOutcome);
    previewLegacy.mockReset().mockResolvedValue({
        kind: "desktop",
        plan: legacyPlan,
    });
    removeEntry.mockReset().mockResolvedValue({
        entryKey: entry.entryKey,
        state: "completed",
        leftovers: [],
    });
});
afterEach(cleanup);

async function mount(): Promise<void> {
    const i18n = await createAppI18n("en");
    render(
        <StrictMode>
            <I18nextProvider i18n={i18n}>
                <RestoreView />
            </I18nextProvider>
        </StrictMode>,
    );
}

async function openVersionPreview(): Promise<void> {
    await userEvent.click(
        await screen.findByRole("button", { name: "Review version" }),
    );
    await waitFor(() => {
        expect(previewVersion).toHaveBeenCalledWith(
            "claudeCode",
            entry.entryKey,
        );
    });
}

function rowSummary(
    text: string,
): (content: string, node: Element | null) => boolean {
    return (_content, node) =>
        node != null &&
        typeof node.className === "string" &&
        node.className.includes("state") &&
        node.textContent === text;
}

test("browser preview explains that the history cannot be read", async () => {
    loadHistory.mockResolvedValue({ kind: "preview" });
    await mount();
    expect(
        await screen.findByText(/browser preview cannot read configuration/u),
    ).toBeTruthy();
    expect(previewVersion).not.toHaveBeenCalled();
});

test("a failed history read offers a retry that re-reads it", async () => {
    let failing = true;
    loadHistory.mockImplementation(() =>
        failing
            ? Promise.reject(new HistoryRequestError("storage_unavailable"))
            : Promise.resolve({ kind: "desktop", page: page() }),
    );
    await mount();
    expect(
        await screen.findByText(/Local configuration storage is unavailable/u),
    ).toBeTruthy();
    failing = false;
    const reads = loadHistory.mock.calls.length;
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(
        await screen.findByRole("button", { name: "Review version" }),
    ).toBeTruthy();
    expect(loadHistory).toHaveBeenCalledTimes(reads + 1);
});

test("an empty history says that nothing was written yet", async () => {
    loadHistory.mockResolvedValue({
        kind: "desktop",
        page: page([], null, currentConfig("unrecorded")),
    });
    await mount();
    expect(
        await screen.findByRole("region", { name: "Current configuration" }),
    ).toBeTruthy();
    expect(
        await screen.findByText(/has not written this configuration yet/u),
    ).toBeTruthy();
});

test("a recorded write shows its intent, state, time, provider and file", async () => {
    await mount();
    expect(
        await screen.findByText(
            rowSummary("Apply · Written · Current configuration"),
        ),
    ).toBeTruthy();
    expect(screen.getByText("DeepSeek Work")).toBeTruthy();
    expect(screen.getByText("Available")).toBeTruthy();
    expect(screen.getByRole("button", { name: "1 file" })).toBeTruthy();
    // The current-config pin always lists managed paths; long backup sentences
    // on the history row stay collapsed until expanded.
    expect(screen.getByText(SETTINGS)).toBeTruthy();
    expect(
        screen.queryByText(
            /Independent pre-mutation backup is available and matches its digest/u,
        ),
    ).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "1 file" }));
    expect(screen.getAllByText(SETTINGS).length).toBeGreaterThan(0);
    expect(
        screen.getByText(
            /Independent pre-mutation backup is available and matches its digest/u,
        ),
    ).toBeTruthy();
    expect(screen.queryByText(String(entry.createdAt))).toBeNull();
});

test("pins the live current configuration above the list and marks a matched entry", async () => {
    await mount();
    expect(
        await screen.findByRole("region", { name: "Current configuration" }),
    ).toBeTruthy();
    expect(screen.getByText(/Current configuration · Matched/u)).toBeTruthy();
    // Managed files stay visible; no expand toggle and no empty Provider slots.
    expect(screen.getByText("Readable")).toBeTruthy();
    expect(screen.getByText(SETTINGS)).toBeTruthy();
    expect(
        screen.queryByRole("button", { name: "Show managed files" }),
    ).toBeNull();
    expect(screen.queryByText(/Provider —/u)).toBeNull();
    expect(
        screen.getByText(rowSummary("Apply · Written · Current configuration")),
    ).toBeTruthy();
});

test("an unrecorded current pin stays fixed when older history loads", async () => {
    const older: HistoryEntry = {
        ...entry,
        entryKey: `operation:${"e".repeat(32)}`,
        operationId: "e".repeat(32),
        createdAt: entry.createdAt - 1_000,
        updatedAt: entry.updatedAt - 1_000,
    };
    const unrecorded = currentConfig("unrecorded");
    loadHistory.mockImplementation((_agent, after = null) =>
        after === null
            ? Promise.resolve({
                  kind: "desktop",
                  page: page([entry], CURSOR, unrecorded),
              })
            : Promise.resolve({
                  kind: "desktop",
                  page: page([older], null, unrecorded),
              }),
    );
    await mount();
    expect(
        await screen.findByText(/Current configuration · Unrecorded/u),
    ).toBeTruthy();
    expect(
        screen.queryByText(
            rowSummary("Apply · Written · Current configuration"),
        ),
    ).toBeNull();
    await userEvent.click(
        screen.getByRole("button", { name: "Load more history" }),
    );
    await waitFor(() => {
        expect(
            screen.getAllByRole("button", { name: "Review version" }),
        ).toHaveLength(2);
    });
    expect(
        screen.getByText(/Current configuration · Unrecorded/u),
    ).toBeTruthy();
    // The pin is one status region above the list, not a second history row.
    expect(
        screen.getAllByRole("region", { name: "Current configuration" }),
    ).toHaveLength(1);
    expect(
        screen.queryByText(
            rowSummary("Apply · Written · Current configuration"),
        ),
    ).toBeNull();
});

test("backup unavailable text is shown without inventing a restore action", async () => {
    const step = entry.sourceSteps[0];
    const revision = entry.revisions[0];
    if (step === undefined || revision === undefined) {
        throw new Error("fixture entry must include a step and revision");
    }
    const unavailable: HistoryEntry = {
        ...entry,
        sourceSteps: [{ ...step, backupState: "unavailable" }],
        revisions: [{ ...revision, backupState: "unavailable" }],
    };
    loadHistory.mockResolvedValue({
        kind: "desktop",
        page: page([unavailable], null, currentConfig("partial")),
    });
    await mount();
    expect(await screen.findByText("Unavailable")).toBeTruthy();
    expect(screen.getByText(/Current configuration · Partial/u)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "1 file" }));
    expect(
        screen.getByText(
            /Independent pre-mutation backup is missing, damaged or unreadable/u,
        ),
    ).toBeTruthy();
    expect(screen.getByRole("button", { name: "Review version" })).toBeTruthy();
});

test("the version preview lists sanitized field changes and confirms the reviewed files", async () => {
    previewVersion.mockResolvedValue({
        kind: "desktop",
        plan: versionPlan,
    });
    await mount();
    await openVersionPreview();
    expect(
        await screen.findByRole("dialog", {
            name: "Version restore preview",
        }),
    ).toBeTruthy();
    expect(screen.getByText("model")).toBeTruthy();
    expect(screen.getAllByText("Replaced").length).toBeGreaterThan(0);
    const modelChange = screen.getByText("model").closest("li");
    expect(modelChange?.textContent).toContain("current-model");
    expect(modelChange?.textContent).toContain("retained-model");
    const secretChange = screen
        .getByText("credential value hidden")
        .closest("li");
    expect(secretChange?.textContent).toContain(SECRET);
    expect(screen.getByText("credential value hidden")).toBeTruthy();
    expect(
        screen.getByText(/Start a new Agent session for the restored/u),
    ).toBeTruthy();

    const reads = loadHistory.mock.calls.length;
    await userEvent.click(
        screen.getByRole("button", { name: "Restore this version" }),
    );
    await waitFor(() => {
        expect(restoreVersion).toHaveBeenCalledWith(
            "claudeCode",
            entry.entryKey,
            [{ revisionId: REVISION, currentDigest: CURRENT_DIGEST }],
        );
    });
    expect(
        await screen.findByRole("region", {
            name: "Version restore result",
        }),
    ).toBeTruthy();
    expect(screen.getByText(`${SETTINGS} — restored`)).toBeTruthy();
    expect(loadHistory).toHaveBeenCalledTimes(reads + 1);
    expect(
        screen.queryByRole("button", { name: "Restore this version" }),
    ).toBeNull();
});

test("a blocked version cannot be confirmed and explains each file", async () => {
    previewVersion.mockResolvedValue({
        kind: "desktop",
        plan: {
            ...versionPlan,
            restorable: false,
            files: [
                {
                    ...versionFile,
                    action: "blocked",
                    reason: "backup_unavailable",
                    summary: {
                        format: "json",
                        changes: [],
                        truncated: false,
                        reason: "backup_unavailable",
                    },
                },
            ],
        },
    });
    await mount();
    await openVersionPreview();
    expect(
        (await screen.findAllByText(/retained copy is missing/u)).length,
    ).toBeGreaterThan(0);
    expect(
        screen.getByText(/This version cannot be restored safely/u),
    ).toBeTruthy();
    expect(
        screen.queryByRole("button", { name: "Restore this version" }),
    ).toBeNull();
});

test("an unresolved entry withholds the version confirmation", async () => {
    previewVersion.mockResolvedValue({
        kind: "desktop",
        plan: {
            ...versionPlan,
            restorable: false,
            files: [],
            reason: "history_unresolved",
        },
    });
    await mount();
    await openVersionPreview();
    expect(
        await screen.findByText(/interrupted writes. Inspect and record/u),
    ).toBeTruthy();
    expect(
        screen.queryByRole("button", { name: "Restore this version" }),
    ).toBeNull();
});

test("a stale version confirmation is refused and the history is re-read", async () => {
    restoreVersion.mockRejectedValue(
        new VersionRestoreRequestError("plan_stale"),
    );
    await mount();
    await openVersionPreview();
    const reads = loadHistory.mock.calls.length;
    await userEvent.click(
        await screen.findByRole("button", { name: "Restore this version" }),
    );
    expect(
        await screen.findByText(
            /A file changed since the preview, so nothing was written/u,
        ),
    ).toBeTruthy();
    expect(
        screen.queryByRole("button", { name: "Restore this version" }),
    ).toBeNull();
    await waitFor(() => {
        expect(loadHistory.mock.calls.length).toBeGreaterThan(reads);
    });
});

test("a version preview read failure is reported and can be retried", async () => {
    previewVersion.mockRejectedValueOnce(
        new VersionRestoreRequestError("storage_unavailable"),
    );
    await mount();
    await userEvent.click(
        await screen.findByRole("button", { name: "Review version" }),
    );
    expect(
        await screen.findByText(/Local configuration storage is unavailable/u),
    ).toBeTruthy();
    await userEvent.click(
        screen.getByRole("button", { name: "Review version" }),
    );
    expect(
        await screen.findByRole("dialog", {
            name: "Version restore preview",
        }),
    ).toBeTruthy();
    expect(previewVersion).toHaveBeenCalledTimes(2);
});

test("restore and compensation batches offer the version preview", async () => {
    const batch: HistoryEntry = {
        ...entry,
        entryKey: "batch:restore-batch",
        intent: "restore",
        sourceState: "compensated",
    };
    loadHistory.mockResolvedValue({
        kind: "desktop",
        page: page([batch]),
    });
    previewVersion.mockResolvedValue({
        kind: "desktop",
        plan: {
            ...versionPlan,
            entryKey: batch.entryKey,
            intent: "version_restore",
        },
    });
    await mount();
    expect(
        await screen.findByText("Restore · File result recorded"),
    ).toBeTruthy();
    await userEvent.click(
        screen.getByRole("button", { name: "Review version" }),
    );
    await waitFor(() => {
        expect(previewVersion).toHaveBeenCalledWith(
            "claudeCode",
            batch.entryKey,
        );
    });
});

test("a legacy apply entry without retained bytes keeps the rollback entry", async () => {
    loadHistory.mockResolvedValue({
        kind: "desktop",
        page: page([{ ...entry, revisions: [] }]),
    });
    // The legacy listing shape is separate from the paginated entry; this test
    // only needs the action to reach planAgentRestore.
    previewLegacy.mockResolvedValue({ kind: "desktop", plan: legacyPlan });
    await mount();
    await userEvent.click(
        await screen.findByRole("button", { name: "Preview restore" }),
    );
    await waitFor(() => {
        expect(previewLegacy).toHaveBeenCalledWith("claudeCode", OPERATION_ID);
    });
    expect(
        await screen.findByRole("region", { name: "Restore preview" }),
    ).toBeTruthy();
});

test("loads more history without replacing the entries already read", async () => {
    const older: HistoryEntry = {
        ...entry,
        entryKey: `operation:${"e".repeat(32)}`,
        operationId: "e".repeat(32),
        createdAt: entry.createdAt - 1_000,
        updatedAt: entry.updatedAt - 1_000,
    };
    loadHistory.mockImplementation((_agent, after = null) =>
        after === null
            ? Promise.resolve({
                  kind: "desktop",
                  page: page([entry], CURSOR),
              })
            : Promise.resolve({ kind: "desktop", page: page([older]) }),
    );
    await mount();
    await screen.findByText(/Apply · Written/u);
    await userEvent.click(
        screen.getByRole("button", { name: "Load more history" }),
    );
    await waitFor(() => {
        expect(loadHistory).toHaveBeenLastCalledWith("claudeCode", CURSOR);
    });
    expect(
        screen.getAllByRole("button", { name: "Review version" }),
    ).toHaveLength(2);
    expect(
        screen.queryByRole("button", { name: "Load more history" }),
    ).toBeNull();
});

test("a failed load more keeps the read entries and offers the button again", async () => {
    const older: HistoryEntry = {
        ...entry,
        entryKey: `operation:${"f".repeat(32)}`,
        operationId: "f".repeat(32),
    };
    let failing = true;
    loadHistory.mockImplementation((_agent, after = null) => {
        if (after === null)
            return Promise.resolve({
                kind: "desktop",
                page: page([entry], CURSOR),
            });
        return failing
            ? Promise.reject(new HistoryRequestError("storage_unavailable"))
            : Promise.resolve({ kind: "desktop", page: page([older]) });
    });
    await mount();
    await screen.findByText(/Apply · Written/u);
    await userEvent.click(
        screen.getByRole("button", { name: "Load more history" }),
    );
    expect(
        await screen.findByText(/More history could not be read/u),
    ).toBeTruthy();
    expect(screen.getByText(/Apply · Written/u)).toBeTruthy();
    failing = false;
    await userEvent.click(
        screen.getByRole("button", { name: "Load more history" }),
    );
    await waitFor(() => {
        expect(
            screen.getAllByRole("button", { name: "Review version" }),
        ).toHaveLength(2);
    });
});

test("each Agent asks for its own history and version preview", async () => {
    const i18n = await createAppI18n("en");
    render(
        <I18nextProvider i18n={i18n}>
            <RestoreView agent="grokBuild" />
        </I18nextProvider>,
    );
    await waitFor(() => {
        expect(loadHistory).toHaveBeenCalledWith("grokBuild");
    });
    await userEvent.click(
        await screen.findByRole("button", { name: "Review version" }),
    );
    await waitFor(() => {
        expect(previewVersion).toHaveBeenCalledWith(
            "grokBuild",
            entry.entryKey,
        );
    });
});

test("deletes a history entry after confirmation and reloads the list", async () => {
    await mount();
    await screen.findByRole("button", { name: "Review version" });
    const reads = loadHistory.mock.calls.length;
    await userEvent.click(
        screen.getByRole("button", {
            name: "Delete this record and its backups",
        }),
    );
    expect(
        await screen.findByRole("heading", {
            name: "Delete configuration history entry?",
        }),
    ).toBeTruthy();
    expect(screen.getByText(/does not change the Agent/u)).toBeTruthy();
    await userEvent.click(
        screen.getByRole("button", { name: "Delete record" }),
    );
    await waitFor(() => {
        expect(removeEntry).toHaveBeenCalledWith("claudeCode", entry.entryKey);
    });
    await waitFor(() => {
        expect(loadHistory.mock.calls.length).toBeGreaterThan(reads);
    });
});

test("an unresolved entry disables delete with a reason", async () => {
    loadHistory.mockResolvedValue({
        kind: "desktop",
        page: page([{ ...entry, requiresResolution: true, revisions: [] }]),
    });
    await mount();
    const trash = await screen.findByRole("button", {
        name: /Interrupted writes still need review before deletion/u,
    });
    expect(trash.getAttribute("aria-disabled")).toBe("true");
    expect(removeEntry).not.toHaveBeenCalled();
});

test("failed leftovers stay visible with a retry", async () => {
    removeEntry
        .mockResolvedValueOnce({
            entryKey: entry.entryKey,
            state: "failed",
            leftovers: [
                {
                    backupId: "backup-1",
                    path: "/tmp/vibemate/backups/backup-1",
                    errorCode: "backup_unavailable",
                },
            ],
        })
        .mockResolvedValueOnce({
            entryKey: entry.entryKey,
            state: "completed",
            leftovers: [],
        });
    await mount();
    await userEvent.click(
        await screen.findByRole("button", {
            name: "Delete this record and its backups",
        }),
    );
    await userEvent.click(
        screen.getByRole("button", { name: "Delete record" }),
    );
    expect(
        await screen.findByText(/These private copies could not be removed/u),
    ).toBeTruthy();
    expect(screen.getByText("/tmp/vibemate/backups/backup-1")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Retry delete" }));
    await waitFor(() => {
        expect(removeEntry).toHaveBeenCalledTimes(2);
    });
});

test("a referenced refusal surfaces in the confirm dialog", async () => {
    removeEntry.mockRejectedValue(
        new HistoryDeleteRequestError("history_referenced"),
    );
    await mount();
    await userEvent.click(
        await screen.findByRole("button", {
            name: "Delete this record and its backups",
        }),
    );
    await userEvent.click(
        screen.getByRole("button", { name: "Delete record" }),
    );
    expect(
        await screen.findByText(/Other history still depends on this entry/u),
    ).toBeTruthy();
});
