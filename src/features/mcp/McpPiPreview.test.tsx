import {
    act,
    cleanup,
    fireEvent,
    render,
    screen,
    waitFor,
    within,
} from "@testing-library/react";
import { StrictMode, useState, type JSX } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import {
    McpPreviewError,
    previewPiMcpRegistration,
    type PiMcpPreview,
} from "../../lib/desktop/mcpPreview";
import type { McpRecord } from "../../lib/desktop/mcp";
import { McpPiPreview } from "./McpPiPreview";
vi.mock(import("../../lib/desktop/mcpPreview"), async (original) => ({
    ...(await original()),
    previewPiMcpRegistration: vi.fn(),
}));
const record: McpRecord = {
    id: "0123456789abcdef0123456789abcdef",
    displayName: "Tools",
    serverName: "tools",
    enabled: true,
    revision: 2,
    connection: {
        type: "stdio",
        command: "not-a-server",
        args: [],
        cwd: null,
        env: ["TOKEN"],
    },
    createdAtMs: 1,
    updatedAtMs: 2,
    cleanupPending: false,
};
const preview: PiMcpPreview = {
    target: {
        path: "/isolated/pi-agent/mcp.json",
        exists: false,
        contentDigest: null,
        symlink: false,
    },
    proposal: {
        definitionId: record.id,
        revision: 2,
        serverName: "tools",
        redactedEntry: {
            command: "not-a-server",
            args: ["~/data"],
            cwd: "relative/work",
            env: { TOKEN: "••••••••" },
            enabled: true,
        },
        sessionRelativeCwd: true,
        expandsHome: true,
    },
};
const read = vi.mocked(previewPiMcpRegistration);
beforeEach(() => {
    read.mockReset().mockResolvedValue(preview);
});
afterEach(cleanup);
async function setup(): Promise<void> {
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
                    Review
                </button>
                {trigger !== null && (
                    <McpPiPreview
                        record={record}
                        returnFocus={trigger}
                        onClose={() => {
                            setTrigger(null);
                        }}
                        onReload={() => {
                            setTrigger(null);
                        }}
                    />
                )}
            </>
        );
    }
    render(
        <StrictMode>
            <I18nextProvider i18n={i18n}>
                <Harness />
            </I18nextProvider>
        </StrictMode>,
    );
    const trigger = screen.getByRole("button", { name: "Review" });
    trigger.focus();
    fireEvent.click(trigger);
}

test("reviews the fixed target, opens only masked entry details, and restores focus", async () => {
    await setup();
    expect(await screen.findByText(preview.target.path)).toBeDefined();
    expect(read).toHaveBeenCalledWith(record.id, record.revision);
    fireEvent.click(screen.getByRole("button", { name: "View entry details" }));
    const details = await screen.findByRole("dialog", {
        name: "Pi MCP entry details",
    });
    expect(within(details).getByText(/"TOKEN": "••••••••"/)).toBeDefined();
    expect(within(details).getByText(/"cwd": "relative\/work"/)).toBeDefined();
    fireEvent.click(within(details).getByRole("button", { name: "Close" }));
    await waitFor(() => {
        expect(
            screen.queryByRole("dialog", { name: "Pi MCP entry details" }),
        ).toBeNull();
    });
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() => {
        expect(screen.queryByRole("dialog")).toBeNull();
        expect(document.activeElement).toBe(
            screen.getByRole("button", { name: "Review" }),
        );
    });
});

test("failed preflight exposes retry but never details until a fresh read succeeds", async () => {
    read.mockRejectedValue(new McpPreviewError("name_conflict"));
    await setup();
    expect(await screen.findByRole("alert")).toBeDefined();
    expect(
        screen.queryByRole("button", { name: "View entry details" }),
    ).toBeNull();
    read.mockResolvedValue(preview);
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText(preview.target.path)).toBeDefined();
    expect(screen.queryByRole("alert")).toBeNull();
});

test("closing a pending preview ignores a late completion and allows another review", async () => {
    let finish: (value: PiMcpPreview) => void = () => {};
    read.mockReturnValue(
        new Promise((resolve) => {
            finish = resolve;
        }),
    );
    await setup();
    expect(screen.getByRole("status")).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await act(async () => {
        finish(preview);
        await Promise.resolve();
    });
    expect(screen.queryByText(preview.target.path)).toBeNull();
    read.mockResolvedValue(preview);
    fireEvent.click(screen.getByRole("button", { name: "Review" }));
    expect(await screen.findByText(preview.target.path)).toBeDefined();
});

test("reload definitions closes a stale-revision preview", async () => {
    read.mockRejectedValue(new McpPreviewError("revision_conflict"));
    await setup();
    await screen.findByRole("alert");
    fireEvent.click(screen.getByRole("button", { name: "Reload definitions" }));
    await waitFor(() => {
        expect(screen.queryByRole("dialog")).toBeNull();
    });
});
