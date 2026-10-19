import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import {
    RestoreRequestError,
    listAgentOperations,
    planAgentRestore,
    applyAgentRestore,
    type RecordedOperation,
} from "../../lib/desktop/agents";
import {
    reviewConfigurationResolution,
    resolveConfigurationOperation,
    type ResolutionReview,
    type ResolutionOutcome,
} from "../../lib/desktop/recovery";
import { RestoreView } from "./RestoreView";

vi.mock(import("../../lib/desktop/agents"), async (original) => ({
    ...(await original()),
    listAgentOperations: vi.fn(),
    planAgentRestore: vi.fn(),
    applyAgentRestore: vi.fn(),
}));
vi.mock(import("../../lib/desktop/recovery"), async (original) => ({
    ...(await original()),
    reviewConfigurationResolution: vi.fn(),
    resolveConfigurationOperation: vi.fn(),
}));
const ID = "a".repeat(32);
const PATH = "/Users/test/.pi/agent/models.json";
const operation: RecordedOperation = {
    operationId: ID,
    state: "unknown",
    providerId: null,
    providerLabel: null,
    createdAt: 1_700_000_000_000,
    updatedAt: 1_700_000_000_500,
    restorable: false,
    requiresResolution: true,
    steps: [
        { ordinal: 0, targetPath: PATH, state: "pending", restoreState: null },
    ],
};
const review: ResolutionReview = {
    operationId: ID,
    reviewedDigest: "d".repeat(64),
    resolvable: true,
    revisions: [
        {
            id: "100-1-0",
            batchId: ID,
            ordinal: 0,
            intent: "apply",
            state: "prepared",
            targetPath: PATH,
            previousDigest: "b".repeat(64),
            intendedDigest: "c".repeat(64),
            currentDigest: "c".repeat(64),
            currentMatch: "intended",
            errorCode: null,
            backupState: "available",
        },
    ],
};
const loadHistory = vi.mocked(listAgentOperations);
const inspectWrite = vi.mocked(reviewConfigurationResolution);
const resolveWrite = vi.mocked(resolveConfigurationOperation);

beforeEach(() => {
    loadHistory
        .mockReset()
        .mockResolvedValue({ kind: "desktop", operations: [operation] });
    inspectWrite.mockReset().mockResolvedValue({ kind: "desktop", review });
    resolveWrite
        .mockReset()
        .mockResolvedValue({ operationId: ID, state: "applied" });
    vi.mocked(planAgentRestore).mockReset();
    vi.mocked(applyAgentRestore).mockReset();
});
afterEach(cleanup);

async function mount(onBusyChange = vi.fn()): Promise<{
    i18n: Awaited<ReturnType<typeof createAppI18n>>;
    unmount: () => void;
}> {
    const i18n = await createAppI18n("en");
    const mounted = render(
        <StrictMode>
            <I18nextProvider i18n={i18n}>
                <RestoreView agent="pi" onBusyChange={onBusyChange} />
            </I18nextProvider>
        </StrictMode>,
    );
    return { i18n, unmount: mounted.unmount };
}
async function inspect(): Promise<void> {
    const user = userEvent.setup();
    await user.click(
        await screen.findByRole("button", {
            name: "Inspect interrupted write",
        }),
    );
    await screen.findByRole("button", { name: "Record these observations" });
}

test("inspection can be cancelled and repeated without repairing logs or restoring files", async () => {
    const user = userEvent.setup();
    await mount();
    expect(
        screen.queryByRole("button", { name: "Preview restore" }),
    ).toBeNull();
    await inspect();
    expect(inspectWrite).toHaveBeenCalledExactlyOnceWith("pi", ID);
    expect(screen.getByText(/does not prove who wrote them/u)).toBeDefined();
    await user.click(screen.getByRole("button", { name: "Close review" }));
    expect(
        screen.queryByRole("button", { name: "Record these observations" }),
    ).toBeNull();
    await inspect();
    expect(inspectWrite).toHaveBeenCalledTimes(2);
    expect(resolveWrite).not.toHaveBeenCalled();
    expect(applyAgentRestore).not.toHaveBeenCalled();
    expect(planAgentRestore).not.toHaveBeenCalled();
});

test("confirmation is single-flight, refreshes history and never chains a restore", async () => {
    const user = userEvent.setup();
    let complete: ((value: ResolutionOutcome) => void) | undefined;
    resolveWrite.mockReturnValue(
        new Promise((resolve) => {
            complete = resolve;
        }),
    );
    const onBusyChange = vi.fn();
    await mount(onBusyChange);
    await inspect();
    const before = loadHistory.mock.calls.length;
    await user.click(
        screen.getByRole("button", { name: "Record these observations" }),
    );
    await user.click(
        screen.getByRole("button", { name: "Recording observations…" }),
    );
    await user.click(screen.getByRole("button", { name: "Recheck" }));
    await user.click(screen.getByRole("button", { name: "Close review" }));
    expect(resolveWrite).toHaveBeenCalledExactlyOnceWith("pi", review);
    expect(loadHistory).toHaveBeenCalledTimes(before);
    expect(onBusyChange).toHaveBeenLastCalledWith(true);
    loadHistory.mockResolvedValue({
        kind: "desktop",
        operations: [
            {
                ...operation,
                state: "applied",
                requiresResolution: false,
                restorable: true,
            },
        ],
    });
    await act(async () => {
        complete?.({ operationId: ID, state: "applied" });
        await Promise.resolve();
    });
    await screen.findByRole("button", { name: "Preview restore" });
    expect(screen.getByRole("status").textContent).toContain(
        "Configuration files were not changed",
    );
    expect(
        screen.queryByRole("button", { name: "Record these observations" }),
    ).toBeNull();
    expect(onBusyChange).toHaveBeenLastCalledWith(false);
    expect(planAgentRestore).not.toHaveBeenCalled();
    expect(applyAgentRestore).not.toHaveBeenCalled();
});

test.each(["diverged", "both", "unreadable", "backup", "superseded"])(
    "%s observations stay visible without a confirmation action",
    async (reason) => {
        const original = review.revisions[0];
        if (original === undefined) throw new Error("Missing test observation");
        inspectWrite.mockResolvedValue({
            kind: "desktop",
            review: {
                ...review,
                resolvable: false,
                revisions: [
                    {
                        ...original,
                        currentMatch:
                            reason === "diverged" ||
                            reason === "both" ||
                            reason === "unreadable"
                                ? reason
                                : "intended",
                        backupState:
                            reason === "backup" ? "unavailable" : "available",
                        errorCode:
                            reason === "unreadable"
                                ? "target_unreadable"
                                : null,
                    },
                ],
            },
        });
        await mount();
        await userEvent.setup().click(
            await screen.findByRole("button", {
                name: "Inspect interrupted write",
            }),
        );
        await screen.findByText(/cannot be settled safely/u);
        expect(
            screen.queryByRole("button", { name: "Record these observations" }),
        ).toBeNull();
        expect(resolveWrite).not.toHaveBeenCalled();
    },
);

test("stale confirmation discards approval and requires a fresh inspection", async () => {
    const user = userEvent.setup();
    resolveWrite.mockRejectedValue(new RestoreRequestError("plan_stale"));
    await mount();
    await inspect();
    const before = loadHistory.mock.calls.length;
    await user.click(
        screen.getByRole("button", { name: "Record these observations" }),
    );
    expect((await screen.findByRole("alert")).textContent).toContain(
        "Inspect again; this action does not restore configuration",
    );
    await waitFor(() => {
        expect(loadHistory.mock.calls.length).toBeGreaterThan(before);
    });
    expect(
        screen.queryByRole("button", { name: "Record these observations" }),
    ).toBeNull();
    await inspect();
    expect(inspectWrite).toHaveBeenCalledTimes(2);
    expect(resolveWrite).toHaveBeenCalledTimes(1);
});

test("inspection read failures can be retried without offering restore", async () => {
    const user = userEvent.setup();
    inspectWrite.mockRejectedValueOnce(
        new RestoreRequestError("storage_unavailable"),
    );
    await mount();
    await user.click(
        await screen.findByRole("button", {
            name: "Inspect interrupted write",
        }),
    );
    await screen.findByRole("alert");
    await inspect();
    expect(inspectWrite).toHaveBeenCalledTimes(2);
    expect(resolveWrite).not.toHaveBeenCalled();
});

test("switching language keeps the same observation approval", async () => {
    const user = userEvent.setup();
    const { i18n } = await mount();
    await inspect();
    await act(async () => {
        await i18n.changeLanguage("zh-CN");
    });
    await user.click(screen.getByRole("button", { name: "记录本次观察" }));
    await screen.findByRole("status");
    expect(resolveWrite).toHaveBeenCalledExactlyOnceWith("pi", review);
    expect(inspectWrite).toHaveBeenCalledTimes(1);
});

test("late inspection completion after unmount cannot offer a stale confirmation", async () => {
    let complete:
        | ((value: { kind: "desktop"; review: ResolutionReview }) => void)
        | undefined;
    inspectWrite.mockReturnValue(
        new Promise((resolve) => {
            complete = resolve;
        }),
    );
    const onBusyChange = vi.fn();
    const { unmount } = await mount(onBusyChange);
    await userEvent.setup().click(
        await screen.findByRole("button", {
            name: "Inspect interrupted write",
        }),
    );
    unmount();
    await act(async () => {
        complete?.({ kind: "desktop", review });
        await Promise.resolve();
    });
    expect(onBusyChange).toHaveBeenLastCalledWith(false);
    expect(resolveWrite).not.toHaveBeenCalled();
    expect(
        screen.queryByRole("button", { name: "Record these observations" }),
    ).toBeNull();
});
