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
    "%s launches the fixed version command on the first click",
    async (locale) => {
        const { onResult, onBusyChange } = await mount(locale);
        expect(probe).not.toHaveBeenCalled();
        fireEvent.click(
            screen.getByRole("button", {
                name: locale === "en" ? "Read version" : "读取版本",
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
test("missing discovery/preview keeps the action inert", async () => {
    const { rerender, i18n, props } = await mount("en", false);
    fireEvent.click(screen.getByRole("button", { name: "Read version" }));
    expect(probe).not.toHaveBeenCalled();
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
    const read = screen.getByRole("button", { name: "Read version" });
    fireEvent.click(read);
    fireEvent.click(read);
    expect(probe).toHaveBeenCalledTimes(1);
    expect(read.getAttribute("aria-disabled")).toBe("true");
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
    await waitFor(() => {
        expect(onResult).toHaveBeenCalledExactlyOnceWith(result);
    });
});
test("known timeout is translated rather than reported as an installation failure", async () => {
    probe.mockRejectedValue(new VersionProbeRequestError("timed_out"));
    await mount();
    fireEvent.click(screen.getByRole("button", { name: "Read version" }));
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
test("language change preserves an in-flight probe without relaunching", async () => {
    let resolve: (value: AgentVersionResult) => void = () => {
        throw Error("not initialized");
    };
    const promise = new Promise<AgentVersionResult>((res) => {
        resolve = res;
    });
    probe.mockReturnValue(promise);
    const { i18n, onResult } = await mount();
    fireEvent.click(screen.getByRole("button", { name: "Read version" }));
    await act(async () => {
        await i18n.changeLanguage("zh-CN");
    });
    expect(screen.getByRole("button", { name: "读取版本" })).toBeDefined();
    expect(probe).toHaveBeenCalledTimes(1);
    await act(async () => {
        resolve(result);
        await promise;
    });
    expect(onResult).toHaveBeenCalledExactlyOnceWith(result);
});
