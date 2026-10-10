import {
    act,
    cleanup,
    fireEvent,
    render,
    screen,
    waitFor,
} from "@testing-library/react";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import {
    listMcpDefinitions,
    getMcpDefinition,
    saveMcpDefinition,
    cleanupMcpCredentials,
    McpRequestError,
    type McpRecord,
} from "../../lib/desktop/mcp";
import { McpView } from "./McpView";
vi.mock(import("../../lib/desktop/mcp"), async (importOriginal) => ({
    ...(await importOriginal()),
    listMcpDefinitions: vi.fn(),
    getMcpDefinition: vi.fn(),
    saveMcpDefinition: vi.fn(),
    cleanupMcpCredentials: vi.fn(),
}));
const list = vi.mocked(listMcpDefinitions);
const get = vi.mocked(getMcpDefinition);
const save = vi.mocked(saveMcpDefinition);
const clean = vi.mocked(cleanupMcpCredentials);
const row: McpRecord = {
    id: "0123456789abcdef0123456789abcdef",
    displayName: "Tools",
    serverName: "tools",
    enabled: true,
    revision: 1,
    connection: {
        type: "stdio",
        command: "not-a-real-server",
        args: [],
        cwd: null,
        env: ["TOKEN"],
    },
    createdAtMs: 1,
    updatedAtMs: 1,
    cleanupPending: false,
};
function page(items: McpRecord[], nextCursor: string | null = null) {
    return { kind: "desktop" as const, page: { items, nextCursor } };
}
async function setup() {
    const i18n = await createAppI18n("en");
    return render(
        <StrictMode>
            <I18nextProvider i18n={i18n}>
                <McpView />
            </I18nextProvider>
        </StrictMode>,
    );
}
beforeEach(() => {
    list.mockReset().mockResolvedValue(page([row]));
    get.mockReset().mockResolvedValue(row);
    save.mockReset().mockResolvedValue({ ...row, revision: 2, enabled: false });
    clean.mockReset().mockResolvedValue(row);
});
afterEach(() => {
    cleanup();
});

test("shows central enabled state without claiming connection or deployment", async () => {
    await setup();
    await screen.findByText("Tools");
    expect(screen.getByText(/Definition enabled · Not deployed/)).toBeDefined();
    expect(save).not.toHaveBeenCalled();
    expect(clean).not.toHaveBeenCalled();
    expect(get).not.toHaveBeenCalled();
});

test("preview blocks creation without displaying fake definitions", async () => {
    list.mockResolvedValue({ kind: "preview" });
    await setup();
    await screen.findByText(/Browser preview cannot read or save/);
    fireEvent.click(screen.getByRole("button", { name: "New definition" }));
    expect(screen.queryByRole("form")).toBeNull();
    expect(save).not.toHaveBeenCalled();
});

test("list failures offer a read retry", async () => {
    list.mockRejectedValueOnce(new McpRequestError("read_failed"));
    await setup();
    await screen.findByRole("alert");
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    await screen.findByText("Tools");
});

test("new definition saves and returns to a re-read list", async () => {
    list.mockResolvedValue(page([]));
    save.mockResolvedValue(row);
    await setup();
    await screen.findByText("No MCP definitions saved yet.");
    fireEvent.click(screen.getByRole("button", { name: "New definition" }));
    fireEvent.change(screen.getByLabelText("Name"), {
        target: { value: "Tools" },
    });
    fireEvent.change(screen.getByLabelText(/^Server identifier/), {
        target: { value: "tools" },
    });
    fireEvent.change(screen.getByLabelText(/^Executable/), {
        target: { value: "not-a-real-server" },
    });
    list.mockResolvedValue(page([row]));
    fireEvent.click(screen.getByRole("button", { name: /^Save$/ }));
    await screen.findByText("Tools");
    expect(save).toHaveBeenCalledOnce();
    expect(screen.queryByRole("form")).toBeNull();
});

test("enablement changes preserve credential names without reading their values", async () => {
    await setup();
    fireEvent.click(
        await screen.findByRole("button", {
            name: "Disable “Tools” definition",
        }),
    );
    await screen.findByText(/Definition disabled/);
    expect(save).toHaveBeenCalledWith({
        id: row.id,
        expectedRevision: 1,
        displayName: "Tools",
        serverName: "tools",
        enabled: false,
        connection: {
            type: "stdio",
            command: "not-a-real-server",
            args: [],
            cwd: null,
            env: [{ name: "TOKEN", value: null }],
        },
    });
});

test("unknown toggle result blocks every mutation until a successful list reload", async () => {
    save.mockRejectedValue(new McpRequestError("outcome_unknown"));
    await setup();
    fireEvent.click(
        await screen.findByRole("button", {
            name: "Disable “Tools” definition",
        }),
    );
    await screen.findByRole("alert");
    fireEvent.click(screen.getByRole("button", { name: "New definition" }));
    fireEvent.click(screen.getByRole("button", { name: "Edit “Tools”" }));
    fireEvent.click(
        screen.getByRole("button", { name: "Disable “Tools” definition" }),
    );
    expect(get).not.toHaveBeenCalled();
    expect(save).toHaveBeenCalledOnce();
    fireEvent.click(
        screen.getByRole("button", { name: "Reload before retrying" }),
    );
    await waitFor(() => {
        expect(screen.queryByRole("alert")).toBeNull();
    });
    fireEvent.click(screen.getByRole("button", { name: "Edit “Tools”" }));
    await screen.findByRole("form", { name: "Edit MCP definition" });
});

test("failed edit reconciliation replaces stale editable rows with read error", async () => {
    get.mockRejectedValue(new McpRequestError("read_failed"));
    await setup();
    fireEvent.click(
        await screen.findByRole("button", { name: "Edit “Tools”" }),
    );
    await screen.findByRole("alert");
    expect(screen.queryByRole("button", { name: "Edit “Tools”" })).toBeNull();
    expect(
        screen
            .getByRole("button", { name: "New definition" })
            .getAttribute("aria-disabled"),
    ).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    await screen.findByText("Tools");
});

test("cleanup remains visible and retryable until old credentials are removed", async () => {
    list.mockResolvedValue(page([{ ...row, cleanupPending: true }]));
    clean
        .mockResolvedValueOnce({ ...row, cleanupPending: true })
        .mockResolvedValue(row);
    await setup();
    fireEvent.click(
        await screen.findByRole("button", { name: "Retry credential cleanup" }),
    );
    await waitFor(() => {
        expect(clean).toHaveBeenCalledOnce();
    });
    await waitFor(() => {
        expect(
            screen
                .getByRole("button", { name: "Retry credential cleanup" })
                .getAttribute("aria-disabled"),
        ).toBe("false");
    });
    fireEvent.click(
        screen.getByRole("button", { name: "Retry credential cleanup" }),
    );
    await waitFor(() => {
        expect(
            screen.queryByRole("button", { name: "Retry credential cleanup" }),
        ).toBeNull();
    });
    expect(clean).toHaveBeenCalledTimes(2);
});

test("cursor pages append without replacing existing definitions", async () => {
    list.mockResolvedValueOnce(page([row], row.id)).mockResolvedValue(
        page([
            {
                ...row,
                id: "f".repeat(32),
                displayName: "Second",
                serverName: "second",
            },
        ]),
    );
    await setup();
    fireEvent.click(await screen.findByRole("button", { name: "Load more" }));
    await screen.findByText("Second");
    expect(screen.getByText("Tools")).toBeDefined();
    expect(list).toHaveBeenLastCalledWith(row.id);
});

test("unmounted lists ignore late read replies", async () => {
    let resolve: (result: ReturnType<typeof page>) => void = () => {};
    list.mockImplementation(
        () =>
            new Promise((done) => {
                resolve = done;
            }),
    );
    const view = await setup();
    await waitFor(() => {
        expect(list).toHaveBeenCalled();
    });
    view.unmount();
    await act(async () => {
        resolve(page([row]));
        await Promise.resolve();
    });
    expect(screen.queryByText("Tools")).toBeNull();
});
