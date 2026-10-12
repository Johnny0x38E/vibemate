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
import {
    probeAgentVersion,
    type AgentVersionResult,
} from "../../lib/desktop/agentVersions";
import { agentReport, automaticAgentPaths } from "../../test/agentDiscovery";
vi.mock(import("../../lib/desktop/agents"), async (original) => ({
    ...(await original()),
    discoverAgents: vi.fn(),
}));
vi.mock(import("../../lib/desktop/agentVersions"), async (original) => ({
    ...(await original()),
    probeAgentVersion: vi.fn(),
}));
const probe = vi.mocked(probeAgentVersion);
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
const result: DiscoveryResult = {
    kind: "desktop",
    items: agentReport([pi, grok]),
};
const read = vi.mocked(discoverAgents);
beforeEach(() => {
    read.mockReset().mockResolvedValue(result);
    probe.mockReset();
});
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
            ...automaticAgentPaths,
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
    const button = screen.getByRole("button", { name: "Discover" });
    await waitFor(() => {
        expect(button.getAttribute("aria-disabled")).toBe("true");
    });
    expect(
        screen.queryByText("Local Agent discovery requires the desktop app."),
    ).toBeNull();
    const calls = read.mock.calls.length;
    fireEvent.click(button);
    expect(read).toHaveBeenCalledTimes(calls);
    expect(screen.queryByText(/Entry found/u)).toBeNull();
    expect(screen.getByRole("list", { name: "Agents" }).children).toHaveLength(
        6,
    );
    fireEvent.click(screen.getByRole("button", { name: "Open Codex details" }));
    expect(
        await screen.findByRole("textbox", { name: "Codex entry path" }),
    ).toHaveProperty("readOnly", true);
    expect(read).toHaveBeenCalledTimes(calls);
});
test("keeps absent entries visible and allows manual paths from their detail", async () => {
    await mount();
    const button = await screen.findByRole("button", {
        name: "Open Grok Build details",
    });
    await waitFor(() => {
        expect(button.getAttribute("aria-disabled")).toBe("false");
    });
    expect(screen.getByRole("list", { name: "Agents" }).children).toHaveLength(
        6,
    );
    fireEvent.click(button);
    expect(
        await screen.findByRole("textbox", { name: "Grok Build entry path" }),
    ).toBeDefined();
    expect(screen.getByText("User config (config.toml)")).toBeDefined();
    expect(document.body.textContent).toContain(
        "No entry was found in the checked locations",
    );
    fireEvent.click(screen.getByRole("button", { name: "Back to agents" }));
    expect(
        screen.getByRole("button", { name: "Open Grok Build details" }),
    ).toBe(document.activeElement);
    await openPiDetail();
    expect(screen.getByText("User settings (settings.json)")).toBeDefined();
});

test.each(["en", "zh-CN"] as const)(
    "all six manual inputs remain independent across navigation and language in %s",
    async (locale) => {
        const { i18n } = await mount(locale);
        await waitFor(() => {
            expect(screen.getByRole("status").textContent).not.toContain("…");
        });
        const names = [
            "Claude Code",
            "Codex",
            "OpenCode",
            "Pi",
            "Grok Build",
            "Hermes",
        ];
        const calls = read.mock.calls.length;
        for (const name of names) {
            fireEvent.click(
                screen.getByRole("button", {
                    name:
                        locale === "en"
                            ? `Open ${name} details`
                            : `打开 ${name} 详情`,
                }),
            );
            fireEvent.change(
                screen.getByRole("textbox", {
                    name:
                        locale === "en"
                            ? `${name} entry path`
                            : `${name} 入口路径`,
                }),
                { target: { value: `/home/test/${name}` } },
            );
            if (name !== "Hermes")
                fireEvent.click(
                    screen.getByRole("button", {
                        name:
                            locale === "en"
                                ? "Back to agents"
                                : "返回 Agent 列表",
                    }),
                );
        }
        expect(read).toHaveBeenCalledTimes(calls);
        fireEvent.click(
            screen.getByRole("button", {
                name: locale === "en" ? "Rediscover" : "重新发现",
            }),
        );
        await waitFor(() => {
            expect(read).toHaveBeenCalledTimes(calls + 1);
        });
        expect(read).toHaveBeenLastCalledWith({
            claudePath: "/home/test/Claude Code",
            codexPath: "/home/test/Codex",
            opencodePath: "/home/test/OpenCode",
            piPath: "/home/test/Pi",
            grokPath: "/home/test/Grok Build",
            hermesPath: "/home/test/Hermes",
        });
        await act(async () => {
            await i18n.changeLanguage(locale === "en" ? "zh-CN" : "en");
        });
        expect(screen.getByRole("textbox")).toHaveProperty(
            "value",
            "/home/test/Hermes",
        );
        expect(read).toHaveBeenCalledTimes(calls + 1);
    },
);

test("catalog is visible while checking and failed refresh labels previous observations", async () => {
    const pending = deferred();
    read.mockReturnValue(pending.promise);
    await mount();
    expect(screen.getByRole("list", { name: "Agents" }).children).toHaveLength(
        6,
    );
    expect(
        screen
            .getByRole("button", { name: "Open Claude Code details" })
            .getAttribute("aria-disabled"),
    ).toBe("true");
    expect(screen.queryByText("Found")).toBeNull();
    expect(
        screen
            .getByRole("button", { name: "Open Pi details" })
            .querySelector("[data-state]")
            ?.getAttribute("data-state"),
    ).toBe("loading");
    await act(async () => {
        pending.resolve(result);
        await pending.promise;
    });
    const live = screen.getByRole("status");
    read.mockRejectedValueOnce(new Error("private-error"));
    fireEvent.click(screen.getByRole("button", { name: "Discover" }));
    await screen.findByRole("alert");
    expect(screen.getByRole("status")).toBe(live);
    expect(screen.getByText("Found · previous result")).toBeDefined();
    const piCard = screen.getByRole("button", { name: "Open Pi details" });
    expect(
        piCard.querySelector("[data-state]")?.getAttribute("data-state"),
    ).toBe("error");
    expect(document.body.textContent).not.toContain("private-error");
    fireEvent.click(
        screen.getByRole("button", { name: "Open Claude Code details" }),
    );
    expect(
        await screen.findByRole("textbox", { name: "Claude Code entry path" }),
    ).toBeDefined();
});

test("an initial failed scan does not report absent installations and remains retryable", async () => {
    read.mockRejectedValue(new Error("private-error"));
    await mount();
    await screen.findByRole("alert");
    expect(screen.getByRole("list", { name: "Agents" }).children).toHaveLength(
        6,
    );
    expect(screen.queryByText("Not found")).toBeNull();
    read.mockResolvedValue(result);
    fireEvent.click(screen.getByRole("button", { name: "Discover" }));
    await screen.findByText("Found");
    expect(screen.queryByRole("alert")).toBeNull();
    expect(
        screen
            .getByRole("button", { name: "Open Pi details" })
            .querySelector("[data-state]")
            ?.getAttribute("data-state"),
    ).toBe("found");
    expect(
        screen
            .getByRole("button", { name: "Open Codex details" })
            .querySelector("[data-state]")
            ?.getAttribute("data-state"),
    ).toBe("notFound");
});

test("confirmed CLI result is separate, blocks scans/navigation and expires at rediscovery", async () => {
    read.mockResolvedValue({
        kind: "desktop",
        items: agentReport([
            {
                ...pi,
                source: "path",
                version: null,
                versionEvidence: "none",
                capabilityEvidence: "unverified",
            },
        ]),
    });
    let resolve: (value: AgentVersionResult) => void = () => {
        throw Error("not initialized");
    };
    const promise = new Promise<AgentVersionResult>((res) => {
        resolve = res;
    });
    probe.mockReturnValue(promise);
    await mount();
    await openPiDetail();
    expect(probe).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Read version" }));
    const run = screen.getByRole("button", { name: "Run version command" });
    fireEvent.click(run);
    fireEvent.click(run);
    const calls = read.mock.calls.length;
    const back = screen.getByRole("button", { name: "Back to agents" });
    expect(back.getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(back);
    fireEvent.click(screen.getByRole("button", { name: "Rediscover" }));
    expect(screen.getByRole("heading", { name: /Pi/u })).toBeDefined();
    expect(screen.getByRole("textbox")).toHaveProperty("readOnly", true);
    expect(read).toHaveBeenCalledTimes(calls);
    expect(probe).toHaveBeenCalledTimes(1);
    expect(probe).toHaveBeenLastCalledWith({
        id: "pi",
        entryPath: pi.entryPath,
        resolvedPath: pi.resolvedPath,
        confirmed: true,
    });
    await act(async () => {
        resolve({
            id: "pi",
            entryPath: "/home/test/pi",
            resolvedPath: "/home/test/package/dist/bundle/cli.js",
            version: "1.1.0",
            evidence: "cliOutput",
        });
        await promise;
    });
    expect(screen.getByText("1.1.0")).toBeDefined();
    fireEvent.click(back);
    const card = screen.getByRole("button", { name: "Open Pi details" });
    expect(card.getAttribute("aria-describedby")).toBe("agent-card-status-pi");
    expect(screen.getByText("1.1.0").getAttribute("title")).toContain(
        "user-triggered CLI",
    );
    read.mockRejectedValueOnce(new Error("synthetic-private-refresh-error"));
    fireEvent.click(screen.getByRole("button", { name: "Discover" }));
    await screen.findByRole("alert");
    expect(screen.queryByText("1.1.0")).toBeNull();
    expect(card.querySelector("[data-state]")?.getAttribute("data-state")).toBe(
        "error",
    );
});

test("editing a manual path requires discovery before a version launch", async () => {
    await mount();
    await openPiDetail();
    const field = screen.getByRole("textbox");
    fireEvent.change(field, { target: { value: "/home/test/pi" } });
    fireEvent.click(screen.getByRole("button", { name: "Read version" }));
    expect(
        screen.getByRole("group", { name: "Confirm version command" }),
    ).toBeDefined();
    fireEvent.change(field, {
        target: { value: "/different/unverified-entry" },
    });
    fireEvent.click(
        screen.getByRole("button", { name: "Run version command" }),
    );
    expect(probe).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(
        screen
            .getByRole("button", { name: "Read version" })
            .getAttribute("aria-disabled"),
    ).toBe("true");
});

test("an unavailable entry is not displayed with a successful status dot", async () => {
    read.mockResolvedValue({
        kind: "desktop",
        items: agentReport([
            {
                ...pi,
                state: "unavailable",
                resolvedPath: null,
                version: null,
                versionEvidence: "none",
                capabilityEvidence: "unverified",
            },
        ]),
    });
    await mount();
    await screen.findByText("Cannot inspect");
    const card = screen.getByRole("button", { name: "Open Pi details" });
    expect(card.querySelector("[data-state]")?.getAttribute("data-state")).toBe(
        "unavailable",
    );
    fireEvent.click(card);
    expect(
        screen
            .getByRole("button", { name: "Read version" })
            .getAttribute("aria-disabled"),
    ).toBe("true");
    expect(probe).not.toHaveBeenCalled();
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
    expect(read).toHaveBeenLastCalledWith(automaticAgentPaths);
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
        items: agentReport([
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
        ]),
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
            ...automaticAgentPaths,
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
        items: agentReport([
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
        ]),
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
