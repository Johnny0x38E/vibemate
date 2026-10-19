import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import {
    RestoreRequestError,
    applyAgentRestore,
    planAgentRestore,
    type ClaudeCodeRestorePlan,
    type RestoreOutcome,
    type RestorePlanStep,
} from "../../lib/desktop/agents";
import {
    HistoryRequestError,
    readConfigurationHistory,
    type HistoryCursor,
    type HistoryEntry,
    type HistoryPage,
} from "../../lib/desktop/history";
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

const SETTINGS = "/Users/test/.claude/settings.json";
const BACKUP =
    "/Users/test/Library/Application Support/dev.vibemate.desktop/backups";
const BEFORE_DIGEST = "a".repeat(64);
const APPLIED_DIGEST = "b".repeat(64);
const CURRENT_DIGEST = "c".repeat(64);
const OPERATION_ID = "fedcba9876543210fedcba9876543210";
const PROVIDER_ID = "0123456789abcdef0123456789abcdef";
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
            state: "applied",
            errorCode: null,
            restoreState: null,
        },
    ],
    revisions: [
        {
            revisionId: "100-1-0",
            ordinal: 0,
            previousDigest: BEFORE_DIGEST,
            intendedDigest: APPLIED_DIGEST,
            backupRecorded: true,
            state: "completed",
            resolution: null,
            errorCode: null,
            createdAt: 1_700_000_000_000,
            updatedAt: 1_700_000_000_500,
        },
    ],
};

const previewStep: RestorePlanStep = {
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

const plan: ClaudeCodeRestorePlan = {
    operationId: OPERATION_ID,
    state: "applied",
    createdAt: entry.createdAt,
    restorable: true,
    providerId: PROVIDER_ID,
    providerLabel: "DeepSeek Work",
    steps: [previewStep],
};

const restoredStep: RestoreOutcome["steps"][number] = {
    ordinal: 0,
    targetPath: SETTINGS,
    action: "restore",
    state: "restored",
    errorCode: null,
    leftoverPath: null,
};
const outcome: RestoreOutcome = {
    operationId: OPERATION_ID,
    state: "compensated",
    steps: [restoredStep],
    leftovers: [],
};

function page(
    entries: HistoryEntry[] = [entry],
    nextCursor: HistoryCursor | null = null,
): HistoryPage {
    return { entries, nextCursor };
}

const loadHistory = vi.mocked(readConfigurationHistory);
const buildPreview = vi.mocked(planAgentRestore);
const runRestore = vi.mocked(applyAgentRestore);

beforeEach(() => {
    loadHistory
        .mockReset()
        .mockResolvedValue({ kind: "desktop", page: page() });
    buildPreview.mockReset().mockResolvedValue({ kind: "desktop", plan });
    runRestore.mockReset().mockResolvedValue(outcome);
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

/** Open the preview for the one apply entry and wait for its card. */
async function openPreview(): Promise<void> {
    await userEvent.click(
        await screen.findByRole("button", { name: "Preview restore" }),
    );
    await waitFor(() => {
        expect(buildPreview).toHaveBeenCalledWith("claudeCode", OPERATION_ID);
    });
}

test("browser preview explains that the history cannot be read", async () => {
    loadHistory.mockResolvedValue({ kind: "preview" });
    await mount();
    expect(
        await screen.findByText(/browser preview cannot read configuration/u),
    ).toBeTruthy();
    expect(buildPreview).not.toHaveBeenCalled();
});

test("a failed history read offers a retry that re-reads it", async () => {
    // The read keeps failing until the retry is clicked, so StrictMode's second
    // mount cannot silently turn that first failure into a success.
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
        await screen.findByRole("button", { name: "Preview restore" }),
    ).toBeTruthy();
    expect(loadHistory).toHaveBeenCalledTimes(reads + 1);
});

test("an empty history says that nothing was written yet", async () => {
    loadHistory.mockResolvedValue({ kind: "desktop", page: page([]) });
    await mount();
    expect(
        await screen.findByText(/has not written this configuration yet/u),
    ).toBeTruthy();
    expect(
        screen.queryByRole("button", { name: "Preview restore" }),
    ).toBeNull();
});

test("a recorded write shows its intent, state, time, provider and file", async () => {
    await mount();
    expect(await screen.findByText("Apply · Written")).toBeTruthy();
    expect(screen.getByText(SETTINGS)).toBeTruthy();
    expect(screen.getByText("Provider: DeepSeek Work")).toBeTruthy();
    // The timestamp is formatted, never the raw millisecond value.
    expect(screen.queryByText(String(entry.createdAt))).toBeNull();
});

test("restore and compensation batches are listed without a legacy restore action", async () => {
    const revision = entry.revisions[0];
    if (revision === undefined) throw new Error("missing revision fixture");
    const batch: HistoryEntry = {
        ...entry,
        entryKey: "batch:restore-batch",
        intent: "restore",
        sourceState: "compensated",
        revisions: [{ ...revision, state: "completed" }],
    };
    const compensation: HistoryEntry = {
        ...batch,
        entryKey: "batch:compensation-batch",
        intent: "compensation",
        revisions: [{ ...revision, revisionId: "101-1-0" }],
    };
    loadHistory.mockResolvedValue({
        kind: "desktop",
        page: page([compensation, batch]),
    });
    await mount();
    expect(
        await screen.findByText("Restore · File result recorded"),
    ).toBeTruthy();
    expect(
        screen.getByText("Automatic compensation · File result recorded"),
    ).toBeTruthy();
    expect(screen.getAllByText(SETTINGS)).toHaveLength(2);
    expect(
        screen.queryByRole("button", { name: "Preview restore" }),
    ).toBeNull();
});

test("an apply entry stays previewable; the preview reports nothing left to undo", async () => {
    buildPreview.mockResolvedValue({
        kind: "desktop",
        plan: {
            ...plan,
            restorable: false,
            steps: [
                {
                    ...previewStep,
                    action: "skip",
                    currentDigest: null,
                    restoredProviderId: null,
                    restoredProviderLabel: null,
                },
            ],
        },
    });
    await mount();
    await openPreview();
    expect(await screen.findByText("Nothing to do")).toBeTruthy();
    expect(
        screen.queryByRole("button", { name: "Restore the previous content" }),
    ).toBeNull();
});

test("loads more history without replacing the entries already read", async () => {
    const older: HistoryEntry = {
        ...entry,
        entryKey: `operation:${"e".repeat(32)}`,
        operationId: "e".repeat(32),
        createdAt: entry.createdAt - 1_000,
        updatedAt: entry.updatedAt - 1_000,
    };
    // Keyed by cursor rather than call order: StrictMode performs two initial
    // reads, and only the second one is allowed to update the view.
    loadHistory.mockImplementation((_agent, after = null) =>
        after === null
            ? Promise.resolve({
                  kind: "desktop",
                  page: page([entry], CURSOR),
              })
            : Promise.resolve({ kind: "desktop", page: page([older]) }),
    );
    await mount();
    await screen.findByText("Apply · Written");
    await userEvent.click(
        screen.getByRole("button", { name: "Load more history" }),
    );
    await waitFor(() => {
        expect(loadHistory).toHaveBeenLastCalledWith("claudeCode", CURSOR);
    });
    expect(
        screen.getAllByRole("button", { name: "Preview restore" }),
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
    await screen.findByText("Apply · Written");
    await userEvent.click(
        screen.getByRole("button", { name: "Load more history" }),
    );
    expect(
        await screen.findByText(/More history could not be read/u),
    ).toBeTruthy();
    expect(screen.getByText("Apply · Written")).toBeTruthy();
    failing = false;
    await userEvent.click(
        screen.getByRole("button", { name: "Load more history" }),
    );
    await waitFor(() => {
        expect(
            screen.getAllByRole("button", { name: "Preview restore" }),
        ).toHaveLength(2);
    });
    expect(screen.queryByRole("alert")).toBeNull();
});

test("previews the files, the actions and the owner of the content", async () => {
    await mount();
    await openPreview();
    expect(
        await screen.findByRole("region", { name: "Restore preview" }),
    ).toBeTruthy();
    expect(screen.getByText("Write the previous content back")).toBeTruthy();
    expect(
        screen.getByText(
            "The content that comes back was written by the Provider DeepSeek Personal.",
        ),
    ).toBeTruthy();
    expect(
        screen.getByText(/The restored file gets back whatever it held/u),
    ).toBeTruthy();
});

test("confirms with the digests the preview showed and reports the result", async () => {
    await mount();
    await openPreview();
    const reads = loadHistory.mock.calls.length;
    await userEvent.click(
        await screen.findByRole("button", {
            name: "Restore the previous content",
        }),
    );
    await waitFor(() => {
        expect(runRestore).toHaveBeenCalledWith("claudeCode", OPERATION_ID, [
            { ordinal: 0, currentDigest: APPLIED_DIGEST },
        ]);
    });
    expect(await screen.findByText(/The write is undone/u)).toBeTruthy();
    expect(
        screen.getByText(`${SETTINGS} — previous content restored`),
    ).toBeTruthy();
    // The settled write is read again, so the list describes the files as they
    // are now instead of keeping the pre-restore flags.
    expect(loadHistory).toHaveBeenCalledTimes(reads + 1);
    // The confirmation is consumed with the plan it belonged to.
    expect(
        screen.queryByRole("button", { name: "Restore the previous content" }),
    ).toBeNull();
});

test("a conflict is explained instead of confirmed", async () => {
    buildPreview.mockResolvedValue({
        kind: "desktop",
        plan: {
            ...plan,
            restorable: false,
            steps: [
                {
                    ...previewStep,
                    action: "conflict",
                    reason: "target_changed",
                    currentDigest: CURRENT_DIGEST,
                    restoredProviderId: null,
                    restoredProviderLabel: null,
                },
            ],
        },
    });
    await mount();
    await openPreview();
    expect(
        await screen.findByText(
            "Left alone: the file changed after this write",
        ),
    ).toBeTruthy();
    expect(screen.getByText("the file changed after this write")).toBeTruthy();
    expect(
        screen.getByText(/Open the file and decide what to keep/u),
    ).toBeTruthy();
    expect(
        screen.queryByRole("button", { name: "Restore the previous content" }),
    ).toBeNull();
});

test("a stale preview is refused and its plan is dropped", async () => {
    runRestore.mockRejectedValue(new RestoreRequestError("plan_stale"));
    await mount();
    await openPreview();
    await userEvent.click(
        await screen.findByRole("button", {
            name: "Restore the previous content",
        }),
    );
    expect(
        await screen.findByText(
            /A file changed since the preview, so nothing was written/u,
        ),
    ).toBeTruthy();
    expect(
        screen.queryByRole("button", { name: "Restore the previous content" }),
    ).toBeNull();
});

test("a second confirmation while the first runs is ignored", async () => {
    let settle: (value: RestoreOutcome) => void = () => {};
    runRestore.mockImplementation(
        () =>
            new Promise<RestoreOutcome>((resolve) => {
                settle = resolve;
            }),
    );
    await mount();
    await openPreview();
    const confirm = await screen.findByRole("button", {
        name: "Restore the previous content",
    });
    await userEvent.click(confirm);
    await userEvent.click(confirm);
    expect(runRestore).toHaveBeenCalledTimes(1);
    // Resolving the pending call is a state update, so it runs inside `act`.
    act(() => {
        settle(outcome);
    });
    expect(await screen.findByText(/The write is undone/u)).toBeTruthy();
});

test("the confirmation works from the keyboard", async () => {
    await mount();
    await openPreview();
    const confirm = await screen.findByRole("button", {
        name: "Restore the previous content",
    });
    // Native buttons are reached by keyboard; this keeps the flow usable without
    // a pointer instead of only asserting that a button exists.
    confirm.focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => {
        expect(runRestore).toHaveBeenCalledTimes(1);
    });
});

test("reports copies the restore could not remove", async () => {
    const leftover = `${BACKUP}/1700000000000-1-0/settings.json`;
    runRestore.mockResolvedValue({
        ...outcome,
        steps: [{ ...restoredStep, leftoverPath: leftover }],
        leftovers: [leftover],
    });
    await mount();
    await openPreview();
    await userEvent.click(
        await screen.findByRole("button", {
            name: "Restore the previous content",
        }),
    );
    expect(
        await screen.findByText(/These private copies could not be removed/u),
    ).toBeTruthy();
    expect(screen.getByText(leftover)).toBeTruthy();
    expect(
        screen.getByText(/Only your user account can read them/u),
    ).toBeTruthy();
});

test("a partially restored write lets the user recheck and preview again", async () => {
    runRestore.mockResolvedValue({
        ...outcome,
        state: "partial",
        steps: [
            restoredStep,
            {
                ordinal: 1,
                targetPath: "/Users/test/.claude/other.json",
                action: "restore",
                state: "conflict",
                errorCode: "target_changed",
                leftoverPath: null,
            },
        ],
    });
    await mount();
    await openPreview();
    await userEvent.click(
        await screen.findByRole("button", {
            name: "Restore the previous content",
        }),
    );
    expect(await screen.findByText(/Some files were left alone/u)).toBeTruthy();
    expect(
        screen.getByText(
            "/Users/test/.claude/other.json — the file changed after this write",
        ),
    ).toBeTruthy();
    // The list is read again, so the user can recheck and preview the remaining
    // work with fresh digests.
    const reads = loadHistory.mock.calls.length;
    await userEvent.click(screen.getByRole("button", { name: "Recheck" }));
    expect(
        await screen.findByRole("button", { name: "Preview restore" }),
    ).toBeTruthy();
    expect(loadHistory).toHaveBeenCalledTimes(reads + 1);
});

test("each Agent asks for its own recorded writes", async () => {
    // Rust keeps the histories apart, but the card must still name the Agent it
    // belongs to: a Grok page that read the Claude Code list would offer to undo
    // another adapter's file.
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
        await screen.findByRole("button", { name: "Preview restore" }),
    );
    await waitFor(() => {
        expect(buildPreview).toHaveBeenCalledWith("grokBuild", OPERATION_ID);
    });
});
