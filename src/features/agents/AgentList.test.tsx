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
async function openPiDetail(): Promise<void> {
    fireEvent.click(
        await screen.findByRole("button", { name: "Open Pi details" }),
    );
    await screen.findByRole("heading", { name: /Pi/u });
}

test.each([
    {
        locale: "en",
        label: "Pi entry path",
        refresh: "Discover",
        rediscover: "Rediscover",
        found: "Found",
        openPi: "Open Pi details",
    },
    {
        locale: "zh-CN",
        label: "Pi 入口路径",
        refresh: "发现",
        rediscover: "重新发现",
        found: "已发现",
        openPi: "打开 Pi 详情",
    },
] as const)(
    "in $locale manual paths scan only on Rediscover and never show connected state",
    async ({ locale, label, rediscover, found, openPi }) => {
        await mount(locale);
        expect(
            await screen.findByRole("button", { name: openPi }),
        ).toBeDefined();
        fireEvent.click(screen.getByRole("button", { name: openPi }));
        expect(await screen.findByText(found)).toBeDefined();
        const calls = read.mock.calls.length;
        fireEvent.change(screen.getByRole("textbox", { name: label }), {
            target: { value: " /home/test user/manual-pi " },
        });
        expect(read).toHaveBeenCalledTimes(calls);
        fireEvent.click(screen.getByRole("button", { name: rediscover }));
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
    const button = screen.getByRole("button", { name: "Discover" });
    expect(button.getAttribute("aria-disabled")).toBe("true");
    const calls = read.mock.calls.length;
    fireEvent.click(button);
    expect(read).toHaveBeenCalledTimes(calls);
    expect(screen.queryByText(/Entry found/u)).toBeNull();
});
test("lists only found agents as cards and opens detail on activate", async () => {
    await mount();
    expect(
        await screen.findByRole("button", { name: "Open Pi details" }),
    ).toBeDefined();
    expect(
        screen.queryByRole("button", { name: "Open Grok Build details" }),
    ).toBeNull();
    expect(screen.getByText("1.1.0")).toBeDefined();
    await openPiDetail();
    expect(screen.getByText("User settings (settings.json)")).toBeDefined();
    expect(screen.getAllByText("Not found").length).toBeGreaterThan(0);
});
test("sanitizes errors and retries with preserved manual input", async () => {
    await mount();
    await openPiDetail();
    await screen.findByText("Found");
    const input = screen.getByRole("textbox", { name: "Pi entry path" });
    fireEvent.change(input, { target: { value: "relative/pi" } });
    read.mockRejectedValueOnce(new DiscoveryRequestError("invalid_path"));
    fireEvent.click(screen.getByRole("button", { name: "Rediscover" }));
    expect(await screen.findByRole("alert")).toHaveProperty(
        "textContent",
        "Enter an absolute entry path without control characters, or clear it to use automatic discovery, then select Discover.",
    );
    expect(input).toHaveProperty("value", "relative/pi");
    read.mockRejectedValueOnce(new Error("synthetic-private-path-and-secret"));
    fireEvent.click(screen.getByRole("button", { name: "Rediscover" }));
    await waitFor(() => {
        expect(screen.getByRole("alert").textContent).toBe(
            "Discovery failed. Select Discover to retry.",
        );
    });
    expect(document.body.textContent).not.toContain("synthetic-private");
    fireEvent.change(input, { target: { value: "" } });
    fireEvent.click(screen.getByRole("button", { name: "Rediscover" }));
    expect(await screen.findByText("Found")).toBeDefined();
    expect(read).toHaveBeenLastCalledWith({ piPath: null, grokPath: null });
});
test("blocks duplicate scans synchronously and keeps fields read-only until completion", async () => {
    await mount();
    await openPiDetail();
    await screen.findByText("Found");
    const next = deferred();
    read.mockReturnValueOnce(next.promise);
    const calls = read.mock.calls.length;
    const button = screen.getByRole("button", { name: "Rediscover" });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(read).toHaveBeenCalledTimes(calls + 1);
    expect(button.getAttribute("aria-disabled")).toBe("true");
    const discoveryStatus = screen.getByRole("status");
    expect(discoveryStatus.textContent).toBe("Discovering Agent entries…");
    expect(
        screen.getByRole("textbox", { name: "Pi entry path" }),
    ).toHaveProperty("readOnly", true);
    await act(async () => {
        next.resolve(result);
        await next.promise;
    });
    expect(button.getAttribute("aria-disabled")).toBe("false");
    expect(screen.queryByText("Discovering Agent entries…")).toBeNull();
});
test("ignores a stale StrictMode mount reply", async () => {
    const old = deferred();
    read.mockReturnValueOnce(old.promise).mockResolvedValueOnce(result);
    await mount();
    expect(
        await screen.findByRole("button", { name: "Open Pi details" }),
    ).toBeDefined();
    await act(async () => {
        old.resolve({ kind: "preview" });
        await old.promise;
    });
    expect(
        screen.getByRole("button", { name: "Open Pi details" }),
    ).toBeDefined();
    expect(
        screen.queryByText("Local Agent discovery requires the desktop app."),
    ).toBeNull();
});
test("late discovery completion after unmount does not restore UI or scan again", async () => {
    await mount();
    await screen.findByRole("button", { name: "Open Pi details" });
    const next = deferred();
    read.mockReturnValueOnce(next.promise);
    fireEvent.click(screen.getByRole("button", { name: "Discover" }));
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
            },
        ],
    });
    const { i18n } = await mount();
    fireEvent.click(
        await screen.findByRole("button", {
            name: "Open Grok Build details",
        }),
    );
    await screen.findByRole("heading", { name: /Grok Build/u });
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
    const calls = read.mock.calls.length;
    await act(async () => {
        await i18n.changeLanguage("zh-CN");
    });
    expect(
        screen.getByRole("textbox", { name: "Grok Build 入口路径" }),
    ).toHaveProperty("value", "/home/test/grok");
    expect(read).toHaveBeenCalledTimes(calls);
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
    fireEvent.click(
        await screen.findByRole("button", {
            name: "Open Grok Build details",
        }),
    );
    const versionBadge = await screen.findByText("1.0.50");
    expect(
        screen.queryByText("Filename hint only; version unverified"),
    ).toBeNull();
    const user = userEvent.setup();
    await user.hover(versionBadge);
    expect(
        await screen.findByText("Filename hint only; version unverified"),
    ).toBeDefined();
    await user.unhover(versionBadge);
    expect(screen.getByText("User config (config.toml)")).toBeDefined();
    expect(screen.getAllByText("Cannot inspect").length).toBe(3);
    expect(screen.queryByText("Not found")).toBeNull();
});
