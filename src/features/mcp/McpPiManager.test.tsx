import {
    act,
    cleanup,
    fireEvent,
    render,
    screen,
    waitFor,
    within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode, useState, type JSX } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import {
    applyPiMcpChange,
    PiMcpError,
    previewPiMcpChange,
    readPiMcpHistory,
    readPiMcpState,
    recoverPiMcpChange,
    type PiMcpApplied,
    type PiMcpPlan,
} from "../../lib/desktop/piMcp";
import {
    emptyPiMcpState,
    piMcpDefinition,
    piMcpDeployment,
    piMcpHistory,
    piMcpPlan,
    piMcpRecovery,
} from "../../test/piMcpFixtures";
import { McpPiManager } from "./McpPiManager";
vi.mock(import("../../lib/desktop/piMcp"), async (original) => ({
    ...(await original()),
    readPiMcpState: vi.fn(),
    readPiMcpHistory: vi.fn(),
    previewPiMcpChange: vi.fn(),
    applyPiMcpChange: vi.fn(),
    recoverPiMcpChange: vi.fn(),
}));
const busy = vi.fn(),
    changed = vi.fn(),
    reload = vi.fn();
beforeEach(() => {
    busy.mockReset();
    changed.mockReset();
    reload.mockReset();
    vi.mocked(readPiMcpState).mockReset().mockResolvedValue(emptyPiMcpState);
    vi.mocked(readPiMcpHistory)
        .mockReset()
        .mockResolvedValue({ items: [], nextCursor: null });
    vi.mocked(previewPiMcpChange)
        .mockReset()
        .mockImplementation((selection) =>
            Promise.resolve({
                ...piMcpPlan,
                action: selection.action,
                beforeEntry:
                    selection.action === "deploy" ? null : piMcpPlan.afterEntry,
                afterEntry:
                    selection.action === "revoke"
                        ? null
                        : {
                              command: "never-start-test-server",
                              args: [],
                              env: {},
                              enabled: selection.action !== "disable",
                          },
            }),
        );
    vi.mocked(applyPiMcpChange)
        .mockReset()
        .mockResolvedValue({
            changeId: "b".repeat(32),
            registered: true,
            reloadRequired: true,
        });
    vi.mocked(recoverPiMcpChange).mockReset().mockResolvedValue({
        changeId: piMcpRecovery.id,
        registered: true,
        reloadRequired: true,
    });
});
afterEach(cleanup);
async function setup() {
    const i18n = await createAppI18n("en");
    function Harness(): JSX.Element {
        const [trigger, setTrigger] = useState<HTMLElement | null>(null);
        return (
            <>
                <button
                    type="button"
                    onClick={(event) => {
                        setTrigger(event.currentTarget);
                    }}
                >
                    Manage test Pi
                </button>
                {trigger !== null && (
                    <McpPiManager
                        record={piMcpDefinition}
                        returnFocus={trigger}
                        onClose={() => {
                            setTrigger(null);
                        }}
                        onReload={reload}
                        onBusyChange={busy}
                        onChanged={changed}
                    />
                )}
            </>
        );
    }
    const result = render(
        <StrictMode>
            <I18nextProvider i18n={i18n}>
                <Harness />
            </I18nextProvider>
        </StrictMode>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Manage test Pi" }));
    return result;
}

test("deployment requires a sanitized review and explicit confirmation; reads do not apply", async () => {
    await setup();
    fireEvent.click(
        await screen.findByRole("button", { name: "Review deployment" }),
    );
    await screen.findByRole("region", { name: "Reviewed entry change" });
    expect(previewPiMcpChange).toHaveBeenCalledWith({
        definitionId: piMcpDefinition.id,
        expectedRevision: 1,
        action: "deploy",
        historyId: null,
    });
    expect(applyPiMcpChange).not.toHaveBeenCalled();
    fireEvent.click(
        screen.getByRole("button", { name: "Confirm file change" }),
    );
    await screen.findByText(/Change written\. Run \/reload/);
    expect(applyPiMcpChange).toHaveBeenCalledWith(
        {
            definitionId: piMcpDefinition.id,
            expectedRevision: 1,
            action: "deploy",
            historyId: null,
        },
        expect.objectContaining({ approval: piMcpPlan.approval }),
    );
    expect(busy.mock.calls).toEqual([[true], [false]]);
    expect(changed).toHaveBeenCalledTimes(1);
});

test("cancelling a review does not apply and returns to current observations", async () => {
    await setup();
    fireEvent.click(
        await screen.findByRole("button", { name: "Review deployment" }),
    );
    await screen.findByRole("button", { name: "Confirm file change" });
    fireEvent.click(
        screen.getByRole("button", { name: "Back to registration" }),
    );
    await screen.findByRole("button", { name: "Review deployment" });
    expect(applyPiMcpChange).not.toHaveBeenCalled();
});

test.each([
    ["Disable in Pi", "disable"],
    ["Revoke from Pi", "revoke"],
] as const)(
    "managed action %s uses the same review/confirm boundary",
    async (label, action) => {
        vi.mocked(readPiMcpState).mockResolvedValue({
            ...emptyPiMcpState,
            deployments: [piMcpDeployment],
        });
        await setup();
        fireEvent.click(await screen.findByRole("button", { name: label }));
        await screen.findByRole("button", { name: "Confirm file change" });
        expect(previewPiMcpChange).toHaveBeenCalledWith(
            expect.objectContaining({ action }),
        );
        expect(applyPiMcpChange).not.toHaveBeenCalled();
        fireEvent.click(
            screen.getByRole("button", { name: "Confirm file change" }),
        );
        await waitFor(() => {
            expect(applyPiMcpChange).toHaveBeenCalledTimes(1);
        });
    },
);

test("enable is offered for a disabled native entry, independent of the central toggle", async () => {
    vi.mocked(readPiMcpState).mockResolvedValue({
        ...emptyPiMcpState,
        deployments: [
            {
                ...piMcpDeployment,
                ownership: { ...piMcpDeployment.ownership, enabled: false },
            },
        ],
    });
    await setup();
    fireEvent.click(
        await screen.findByRole("button", { name: "Enable in Pi" }),
    );
    await screen.findByRole("button", { name: "Confirm file change" });
    expect(previewPiMcpChange).toHaveBeenCalledWith(
        expect.objectContaining({ action: "enable" }),
    );
});

test("a pending write cannot be double-confirmed or dismissed with Escape", async () => {
    let resolve: (value: PiMcpApplied) => void = () => {};
    vi.mocked(applyPiMcpChange).mockReturnValue(
        new Promise((done) => {
            resolve = done;
        }),
    );
    await setup();
    fireEvent.click(
        await screen.findByRole("button", { name: "Review deployment" }),
    );
    const confirm = await screen.findByRole("button", {
        name: "Confirm file change",
    });
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    await userEvent.setup().keyboard("{Escape}");
    expect(screen.getByRole("dialog")).toBeDefined();
    expect(applyPiMcpChange).toHaveBeenCalledTimes(1);
    expect(
        screen.getByRole("button", { name: "Close" }).hasAttribute("disabled"),
    ).toBe(true);
    await act(async () => {
        resolve({
            changeId: "b".repeat(32),
            registered: true,
            reloadRequired: true,
        });
        await Promise.resolve();
    });
    await screen.findByText(/Change written/);
});

test("a write that outlives its view still releases the parent's guard and requests reconciliation", async () => {
    let resolve: (value: PiMcpApplied) => void = () => {};
    vi.mocked(applyPiMcpChange).mockReturnValue(
        new Promise((done) => {
            resolve = done;
        }),
    );
    const view = await setup();
    fireEvent.click(
        await screen.findByRole("button", { name: "Review deployment" }),
    );
    fireEvent.click(
        await screen.findByRole("button", { name: "Confirm file change" }),
    );
    view.unmount();
    await act(async () => {
        resolve({
            changeId: "b".repeat(32),
            registered: true,
            reloadRequired: true,
        });
        await Promise.resolve();
    });
    expect(busy.mock.calls).toEqual([[true], [false]]);
    expect(changed).toHaveBeenCalledTimes(1);
});

test("unknown write outcome discards approval and rereads recovery instead of blind retry", async () => {
    vi.mocked(applyPiMcpChange).mockRejectedValue(
        new PiMcpError("outcome_unknown"),
    );
    await setup();
    fireEvent.click(
        await screen.findByRole("button", { name: "Review deployment" }),
    );
    await screen.findByRole("button", { name: "Confirm file change" });
    const reads = vi.mocked(readPiMcpState).mock.calls.length;
    vi.mocked(readPiMcpState).mockResolvedValue({
        ...emptyPiMcpState,
        recovery: piMcpRecovery,
    });
    fireEvent.click(
        screen.getByRole("button", { name: "Confirm file change" }),
    );
    await screen.findByRole("alert");
    await screen.findByRole("button", { name: "Review interruption" });
    expect(vi.mocked(readPiMcpState).mock.calls.length).toBeGreaterThan(reads);
    expect(
        screen.queryByRole("button", { name: "Confirm file change" }),
    ).toBeNull();
    expect(applyPiMcpChange).toHaveBeenCalledTimes(1);
    expect(
        screen
            .getByRole("button", { name: "Review deployment" })
            .hasAttribute("disabled"),
    ).toBe(true);
});

test("recovery requires its own observation review and never invokes the file-change command", async () => {
    vi.mocked(readPiMcpState).mockResolvedValue({
        ...emptyPiMcpState,
        recovery: piMcpRecovery,
    });
    await setup();
    fireEvent.click(
        await screen.findByRole("button", { name: "Review interruption" }),
    );
    await screen.findByText(/This only reconciles/);
    expect(recoverPiMcpChange).not.toHaveBeenCalled();
    fireEvent.click(
        screen.getByRole("button", { name: "Confirm observed state" }),
    );
    await waitFor(() => {
        expect(recoverPiMcpChange).toHaveBeenCalledWith(piMcpRecovery);
    });
    expect(applyPiMcpChange).not.toHaveBeenCalled();
});

test("divergent recovery provides no approval or mutation button", async () => {
    vi.mocked(readPiMcpState).mockResolvedValue({
        ...emptyPiMcpState,
        recovery: { ...piMcpRecovery, observation: "conflict", approval: null },
    });
    await setup();
    await screen.findByText(/matches neither recorded state/);
    expect(
        screen.queryByRole("button", { name: "Review interruption" }),
    ).toBeNull();
    expect(
        screen
            .getByRole("button", { name: "Review deployment" })
            .hasAttribute("disabled"),
    ).toBe(true);
    expect(recoverPiMcpChange).not.toHaveBeenCalled();
});

test("history pages are appended and a selected restore reviews its exact source", async () => {
    vi.mocked(readPiMcpHistory)
        .mockResolvedValueOnce(piMcpHistory)
        .mockResolvedValueOnce({
            ...piMcpHistory,
            nextCursor: piMcpHistory.items[0]?.id ?? null,
        })
        .mockResolvedValue({
            items: [
                {
                    id: "1".repeat(32),
                    action: "disable",
                    createdAt: 5,
                    enabled: false,
                    serverName: "tools",
                },
            ],
            nextCursor: null,
        });
    await setup();
    fireEvent.click(await screen.findByRole("button", { name: "Load more" }));
    await screen.findByRole("button", { name: "Review entry result 11111111" });
    expect(readPiMcpHistory).toHaveBeenLastCalledWith(
        piMcpDefinition.id,
        "b".repeat(32),
    );
    fireEvent.click(
        screen.getByRole("button", { name: "Review entry result 11111111" }),
    );
    await screen.findByText(/old plaintext credentials/);
    expect(previewPiMcpChange).toHaveBeenCalledWith(
        expect.objectContaining({
            action: "restore",
            historyId: "1".repeat(32),
        }),
    );
    expect(applyPiMcpChange).not.toHaveBeenCalled();
});

test("nested masked details close before the manager and return focus", async () => {
    const user = userEvent.setup();
    await setup();
    fireEvent.click(
        await screen.findByRole("button", { name: "Review deployment" }),
    );
    const details = await screen.findByRole("button", {
        name: "View entry details",
    });
    details.focus();
    await user.keyboard("{Enter}");
    const dialogs = screen.getAllByRole("dialog", { hidden: true });
    expect(dialogs).toHaveLength(2);
    const nested = dialogs.find(
        (node) =>
            within(node).queryByText("Before and after for this entry only.", {
                exact: false,
            }) !== null,
    );
    expect(nested).toBeDefined();
    await user.keyboard("{Escape}");
    await waitFor(() => {
        expect(screen.getAllByRole("dialog")).toHaveLength(1);
        expect(document.activeElement).toBe(details);
    });
    await user.keyboard("{Escape}");
    await waitFor(() => {
        expect(screen.queryByRole("dialog")).toBeNull();
        expect(document.activeElement).toBe(
            screen.getByRole("button", { name: "Manage test Pi" }),
        );
    });
});

test("a closed preview ignores late failures and a retry uses only its new review", async () => {
    let reject: (error: Error) => void = () => {};
    vi.mocked(previewPiMcpChange).mockReturnValueOnce(
        new Promise<PiMcpPlan>((_, fail) => {
            reject = fail;
        }),
    );
    await setup();
    fireEvent.click(
        await screen.findByRole("button", { name: "Review deployment" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await act(async () => {
        reject(new PiMcpError("name_conflict"));
        await Promise.resolve();
    });
    expect(screen.queryByRole("alert")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Manage test Pi" }));
    fireEvent.click(
        await screen.findByRole("button", { name: "Review deployment" }),
    );
    await screen.findByRole("button", { name: "Confirm file change" });
    expect(applyPiMcpChange).not.toHaveBeenCalled();
});
