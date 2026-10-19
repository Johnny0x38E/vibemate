import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import {
    RestoreRequestError,
    applyAgentRestore,
    listAgentOperations,
    planAgentRestore,
    type ClaudeCodeRestorePlan,
    type RecordedOperation,
    type RestoreOutcome,
    type RestorePlanStep,
} from "../../lib/desktop/agents";
import { RestoreView } from "./RestoreView";

vi.mock(import("../../lib/desktop/agents"), async (original) => ({
    ...(await original()),
    applyAgentRestore: vi.fn(),
    listAgentOperations: vi.fn(),
    planAgentRestore: vi.fn(),
}));

const SETTINGS = "/Users/test/.claude/settings.json";
const BACKUP =
    "/Users/test/Library/Application Support/dev.vibemate.desktop/backups";
const APPLIED_DIGEST = "b".repeat(64);
const CURRENT_DIGEST = "c".repeat(64);
const OPERATION_ID = "fedcba9876543210fedcba9876543210";
const PROVIDER_ID = "0123456789abcdef0123456789abcdef";

const operation: RecordedOperation = {
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
    createdAt: operation.createdAt,
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

const loadHistory = vi.mocked(listAgentOperations);
const buildPreview = vi.mocked(planAgentRestore);
const runRestore = vi.mocked(applyAgentRestore);

beforeEach(() => {
    loadHistory
        .mockReset()
        .mockResolvedValue({ kind: "desktop", operations: [operation] });
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

/** Open the preview for the one recorded write and wait for its card. */
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
            ? Promise.reject(new RestoreRequestError("storage_unavailable"))
            : Promise.resolve({ kind: "desktop", operations: [operation] }),
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
    loadHistory.mockResolvedValue({ kind: "desktop", operations: [] });
    await mount();
    expect(
        await screen.findByText(/has not written this configuration yet/u),
    ).toBeTruthy();
    expect(
        screen.queryByRole("button", { name: "Preview restore" }),
    ).toBeNull();
});

test("a recorded write shows its state, time, provider and file", async () => {
    await mount();
    expect(await screen.findByText("Written")).toBeTruthy();
    expect(screen.getByText(SETTINGS)).toBeTruthy();
    expect(screen.getByText("Provider: DeepSeek Work")).toBeTruthy();
    // The timestamp is formatted, never the raw millisecond value.
    expect(screen.queryByText(String(operation.createdAt))).toBeNull();
});

test("a write with nothing to restore offers no preview", async () => {
    loadHistory.mockResolvedValue({
        kind: "desktop",
        operations: [{ ...operation, state: "compensated", restorable: false }],
    });
    await mount();
    expect(
        await screen.findByText(/nothing to restore in its current state/u),
    ).toBeTruthy();
    expect(
        screen.queryByRole("button", { name: "Preview restore" }),
    ).toBeNull();
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
