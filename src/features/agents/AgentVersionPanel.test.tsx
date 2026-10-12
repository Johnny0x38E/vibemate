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
import { agentReport } from "../../test/agentDiscovery";
import {
    probeAgentVersion,
    VersionProbeRequestError,
    type AgentVersionResult,
} from "../../lib/desktop/agentVersions";
import { AgentVersionPanel } from "./AgentVersionPanel";

vi.mock(import("../../lib/desktop/agentVersions"), async (original) => ({
    ...(await original()),
    probeAgentVersion: vi.fn(),
}));
const base = agentReport().find((row) => row.id === "codex");
if (base === undefined) throw Error("missing test entry");
const row = {
    ...base,
    state: "found" as const,
    source: "path" as const,
    entryPath: "/test/codex entry ; literal",
    resolvedPath: "/test/native/codex",
};
const result: AgentVersionResult = {
    id: "codex",
    entryPath: row.entryPath,
    resolvedPath: row.resolvedPath,
    version: "0.98.0",
    evidence: "cliOutput",
};
const probe = vi.mocked(probeAgentVersion);
beforeEach(() => {
    probe.mockReset().mockResolvedValue(result);
});
afterEach(cleanup);
async function mount(locale: "en" | "zh-CN" = "en", enabled = true) {
    const i18n = await createAppI18n(locale),
        onResult = vi.fn(),
        onBusyChange = vi.fn();
    const props = { row, enabled, hidden: false, onResult, onBusyChange };
    const view = render(
        <StrictMode>
            <I18nextProvider i18n={i18n}>
                <AgentVersionPanel {...props} />
            </I18nextProvider>
        </StrictMode>,
    );
    return { ...view, i18n, props, onResult, onBusyChange };
}

test.each(["en", "zh-CN"] as const)(
    "%s requires a second explicit confirmation and exposes fixed command risks",
    async (locale) => {
        const { onResult, onBusyChange } = await mount(locale);
        expect(probe).not.toHaveBeenCalled();
        fireEvent.click(
            screen.getByRole("button", {
                name: locale === "en" ? "Read version" : "读取版本",
            }),
        );
        expect(probe).not.toHaveBeenCalled();
        expect(screen.getByText(`${row.resolvedPath} --version`)).toBeDefined();
        expect(screen.getByText(row.entryPath)).toBeDefined();
        expect(document.body.textContent).toContain(
            locale === "en" ? "not a sandbox" : "不是沙箱",
        );
        fireEvent.click(
            screen.getByRole("button", {
                name: locale === "en" ? "Run version command" : "执行版本命令",
            }),
        );
        await waitFor(() => {
            expect(onResult).toHaveBeenCalledExactlyOnceWith(result);
        });
        expect(probe).toHaveBeenCalledExactlyOnceWith({
            id: row.id,
            entryPath: row.entryPath,
            resolvedPath: row.resolvedPath,
            confirmed: true,
        });
        expect(onBusyChange.mock.calls).toEqual([[true], [false]]);
    },
);
test("cancel and Escape return focus without launching", async () => {
    await mount();
    const opener = screen.getByRole("button", { name: "Read version" });
    fireEvent.click(opener);
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(document.activeElement).toBe(opener);
    fireEvent.click(opener);
    fireEvent.keyDown(
        screen.getByRole("button", { name: "Run version command" }),
        { key: "Escape" },
    );
    expect(
        screen.queryByRole("group", { name: "Confirm version command" }),
    ).toBeNull();
    expect(document.activeElement).toBe(opener);
    expect(probe).not.toHaveBeenCalled();
});
test("missing discovery/preview disables confirmation", async () => {
    const { rerender, i18n, props } = await mount("en", false);
    fireEvent.click(screen.getByRole("button", { name: "Read version" }));
    expect(screen.queryByRole("group")).toBeNull();
    rerender(
        <I18nextProvider i18n={i18n}>
            <AgentVersionPanel {...props} enabled={true} row={undefined} />
        </I18nextProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Read version" }));
    expect(probe).not.toHaveBeenCalled();
});
test("repeat click cannot launch while pending and unsafe failure remains retryable", async () => {
    let reject: (error: unknown) => void = () => {
        throw Error("not initialized");
    };
    probe.mockReturnValueOnce(
        new Promise((_resolve, rej) => {
            reject = rej;
        }),
    );
    const { onResult, onBusyChange } = await mount();
    fireEvent.click(screen.getByRole("button", { name: "Read version" }));
    const run = screen.getByRole("button", { name: "Run version command" });
    fireEvent.click(run);
    fireEvent.click(run);
    fireEvent.click(screen.getByRole("button", { name: "Read version" }));
    expect(probe).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("status").textContent).toContain(
        "Reading CLI version",
    );
    await act(async () => {
        reject(new Error("synthetic-private-output"));
        await Promise.resolve();
    });
    expect(screen.getByRole("alert").textContent).toContain(
        "could not complete",
    );
    expect(document.body.textContent).not.toContain("synthetic-private-output");
    expect(onResult).not.toHaveBeenCalled();
    expect(onBusyChange.mock.calls).toEqual([[true], [false]]);
    fireEvent.click(screen.getByRole("button", { name: "Read version" }));
    fireEvent.click(
        screen.getByRole("button", { name: "Run version command" }),
    );
    await waitFor(() => {
        expect(onResult).toHaveBeenCalledExactlyOnceWith(result);
    });
});
test("known timeout is translated rather than reported as an installation failure", async () => {
    probe.mockRejectedValue(new VersionProbeRequestError("timed_out"));
    await mount();
    fireEvent.click(screen.getByRole("button", { name: "Read version" }));
    fireEvent.click(
        screen.getByRole("button", { name: "Run version command" }),
    );
    expect((await screen.findByRole("alert")).textContent).toContain(
        "Descendants or side effects may remain",
    );
});
test.each(["hide", "unmount"] as const)(
    "late successful reply cannot publish after %s",
    async (action) => {
        let resolve: (value: AgentVersionResult) => void = () => {
            throw Error("not initialized");
        };
        const promise = new Promise<AgentVersionResult>((res) => {
            resolve = res;
        });
        probe.mockReturnValue(promise);
        const { props, rerender, unmount, i18n, onResult } = await mount();
        fireEvent.click(screen.getByRole("button", { name: "Read version" }));
        fireEvent.click(
            screen.getByRole("button", { name: "Run version command" }),
        );
        if (action === "hide")
            rerender(
                <I18nextProvider i18n={i18n}>
                    <AgentVersionPanel {...props} hidden={true} />
                </I18nextProvider>,
            );
        else unmount();
        await act(async () => {
            resolve(result);
            await promise;
        });
        expect(onResult).not.toHaveBeenCalled();
    },
);
test("language change preserves confirmation without executing", async () => {
    const { i18n } = await mount();
    fireEvent.click(screen.getByRole("button", { name: "Read version" }));
    await act(async () => {
        await i18n.changeLanguage("zh-CN");
    });
    expect(screen.getByRole("button", { name: "执行版本命令" })).toBeDefined();
    expect(screen.getByText(`${row.resolvedPath} --version`)).toBeDefined();
    expect(probe).not.toHaveBeenCalled();
});
