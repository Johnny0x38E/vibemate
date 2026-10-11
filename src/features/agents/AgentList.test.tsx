import {
    act,
    cleanup,
    fireEvent,
    render,
    screen,
    waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import {
    discoverAgents,
    DiscoveryRequestError,
    type AgentObservation,
    type DiscoveryResult,
} from "../../lib/desktop/agents";
import { AgentList } from "./AgentList";
vi.mock(import("../../lib/desktop/agents"), async (original) => ({
    ...(await original()),
    discoverAgents: vi.fn(),
}));
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
    state: "notFound",
    source: "none",
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
const result: DiscoveryResult = { kind: "desktop", items: [pi, grok] };
const read = vi.mocked(discoverAgents);
beforeEach(() => read.mockReset().mockResolvedValue(result));
afterEach(cleanup);
async function mount(locale: "en" | "zh-CN" = "en") {
    const i18n = await createAppI18n(locale);
    const view = render(
        <StrictMode>
            <I18nextProvider i18n={i18n}>
                <AgentList />
            </I18nextProvider>
        </StrictMode>,
    );
    return { ...view, i18n };
}
function deferred() {
    let resolve: (value: DiscoveryResult) => void = () => {
        throw Error("not initialized");
    };
    const promise = new Promise<DiscoveryResult>((res) => {
        resolve = res;
    });
    return { promise, resolve };
}
test.each([
    {
        locale: "en",
        label: "Pi entry path",
        refresh: "Refresh discovery",
        found: "Entry found · Manual path",
    },
    {
        locale: "zh-CN",
        label: "Pi 入口路径",
        refresh: "刷新发现",
        found: "已发现入口 · 手动路径",
    },
] as const)(
    "in $locale manual paths scan only on Refresh and never show connected state",
    async ({ locale, label, refresh, found }) => {
        await mount(locale);
        expect(await screen.findByText(found)).toBeDefined();
        const calls = read.mock.calls.length;
        fireEvent.change(screen.getByRole("textbox", { name: label }), {
            target: { value: " /home/test user/manual-pi " },
        });
        expect(read).toHaveBeenCalledTimes(calls);
        fireEvent.click(screen.getByRole("button", { name: refresh }));
        await waitFor(() => {
            expect(read).toHaveBeenCalledTimes(calls + 1);
        });
        expect(read).toHaveBeenLastCalledWith({
            piPath: "/home/test user/manual-pi",
            grokPath: null,
        });
        expect(await screen.findByText(found)).toBeDefined();
        expect(document.body.textContent).not.toMatch(/\bconnected\b|已连接/u);
    },
);
test("shows browser preview without fabricated entries or a refresh call", async () => {
    read.mockResolvedValue({ kind: "preview" });
    await mount();
    expect(
        await screen.findByText(
            "Local Agent discovery requires the desktop app.",
        ),
    ).toBeDefined();
    const button = screen.getByRole("button", { name: "Refresh discovery" });
    expect(button.getAttribute("aria-disabled")).toBe("true");
    const calls = read.mock.calls.length;
    fireEvent.click(button);
    expect(read).toHaveBeenCalledTimes(calls);
    expect(screen.queryByText(/Entry found/u)).toBeNull();
});
test("sanitizes errors and retries with preserved manual input", async () => {
    await mount();
    await screen.findByText("Entry found · Manual path");
    const input = screen.getByRole("textbox", { name: "Pi entry path" });
    fireEvent.change(input, { target: { value: "relative/pi" } });
    read.mockRejectedValueOnce(new DiscoveryRequestError("invalid_path"));
    fireEvent.click(screen.getByRole("button", { name: "Refresh discovery" }));
    expect(await screen.findByRole("alert")).toHaveProperty(
        "textContent",
        "Enter an absolute entry path without control characters, or clear it to use automatic discovery, then refresh.",
    );
    expect(input).toHaveProperty("value", "relative/pi");
    read.mockRejectedValueOnce(new Error("synthetic-private-path-and-secret"));
    fireEvent.click(screen.getByRole("button", { name: "Refresh discovery" }));
    await waitFor(() => {
        expect(screen.getByRole("alert").textContent).toBe(
            "Discovery failed. Refresh to retry.",
        );
    });
    expect(document.body.textContent).not.toContain("synthetic-private");
    fireEvent.change(input, { target: { value: "" } });
    fireEvent.click(screen.getByRole("button", { name: "Refresh discovery" }));
    expect(await screen.findByText("Entry found · Manual path")).toBeDefined();
    expect(read).toHaveBeenLastCalledWith({ piPath: null, grokPath: null });
});
test("blocks duplicate scans synchronously and keeps fields read-only until completion", async () => {
    await mount();
    await screen.findByText("Entry found · Manual path");
    const next = deferred();
    read.mockReturnValueOnce(next.promise);
    const calls = read.mock.calls.length;
    const button = screen.getByRole("button", { name: "Refresh discovery" });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(read).toHaveBeenCalledTimes(calls + 1);
    expect(button.getAttribute("aria-disabled")).toBe("true");
    expect(
        screen.getByRole("textbox", { name: "Pi entry path" }),
    ).toHaveProperty("readOnly", true);
    await act(async () => {
        next.resolve(result);
        await next.promise;
    });
    expect(button.getAttribute("aria-disabled")).toBe("false");
});
test("ignores a stale StrictMode mount reply", async () => {
    const old = deferred();
    read.mockReturnValueOnce(old.promise).mockResolvedValueOnce(result);
    await mount();
    await screen.findByText("Entry found · Manual path");
    await act(async () => {
        old.resolve({ kind: "preview" });
        await old.promise;
    });
    expect(screen.getByText("Entry found · Manual path")).toBeDefined();
    expect(
        screen.queryByText("Local Agent discovery requires the desktop app."),
    ).toBeNull();
});
test("late discovery completion after unmount does not restore UI or scan again", async () => {
    await mount();
    await screen.findByText("Entry found · Manual path");
    const next = deferred();
    read.mockReturnValueOnce(next.promise);
    fireEvent.click(screen.getByRole("button", { name: "Refresh discovery" }));
    const calls = read.mock.calls.length;
    cleanup();
    await act(async () => {
        next.resolve(result);
        await next.promise;
    });
    expect(screen.queryByRole("heading")).toBeNull();
    expect(read).toHaveBeenCalledTimes(calls);
});
test("keyboard Enter submits both paths and locale switching preserves them without scanning", async () => {
    const { i18n } = await mount();
    await screen.findByText("Entry found · Manual path");
    const user = userEvent.setup();
    await user.click(
        screen.getByRole("textbox", { name: "Grok Build entry path" }),
    );
    await user.type(
        screen.getByRole("textbox", { name: "Grok Build entry path" }),
        "/home/test/grok{Enter}",
    );
    await waitFor(() => {
        expect(read).toHaveBeenLastCalledWith({
            piPath: null,
            grokPath: "/home/test/grok",
        });
    });
    await screen.findByText("Entry found · Manual path");
    const calls = read.mock.calls.length;
    await act(async () => {
        await i18n.changeLanguage("zh-CN");
    });
    expect(
        screen.getByRole("textbox", { name: "Grok Build 入口路径" }),
    ).toHaveProperty("value", "/home/test/grok");
    expect(read).toHaveBeenCalledTimes(calls);
    expect(screen.getByText("已发现入口 · 手动路径")).toBeDefined();
});
test("filename version remains unverified and unavailable config roots do not show missing files", async () => {
    read.mockResolvedValue({
        kind: "desktop",
        items: [
            pi,
            {
                ...grok,
                state: "found",
                source: "path",
                entryPath: "/home/test/grok",
                resolvedPath: "/home/test/grok-1.0.50-linux-x86_64",
                version: "1.0.50",
                versionEvidence: "filename",
                directorySource: "unavailable",
                configDirectory: null,
                files: grok.files.map((file) => ({
                    ...file,
                    state: "unavailable",
                })),
            },
        ],
    });
    await mount();
    expect(
        await screen.findByText(
            "Version: 1.0.50 · Filename hint only; version unverified",
        ),
    ).toBeDefined();
    expect(screen.getByText("config.toml · Cannot inspect")).toBeDefined();
    expect(screen.queryByText("config.toml · Not found")).toBeNull();
    expect(
        screen.getByText(
            "Version-specific configuration capabilities are unverified; no support or incompatibility is inferred.",
        ),
    ).toBeDefined();
});
