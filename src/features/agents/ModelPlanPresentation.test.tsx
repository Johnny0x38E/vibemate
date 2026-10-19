import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import {
    readModelConfigurationPreview,
    type ModelPreviewFile,
} from "../../lib/desktop/configurationPreview";
import { VersionRestoreRequestError } from "../../lib/desktop/versionRestore";
import type { PiPlan } from "../../lib/desktop/pi";
import { ModelPlanPresentation } from "./ModelPlanPresentation";
vi.mock(import("../../lib/desktop/configurationPreview"), () => ({
    readModelConfigurationPreview: vi.fn(),
}));
const target = (name: string) => ({
    path: `/qa/${name}.json`,
    exists: true,
    contentDigest: "a".repeat(64),
    symlink: false,
});
const plan: PiPlan = {
    agent: "pi",
    providerKey: "qa",
    previousProviderKey: null,
    files: [
        { target: target("models"), changes: [] },
        { target: target("settings"), changes: [] },
    ],
    authTarget: target("auth"),
    source: {
        providerId: "a".repeat(32),
        modelId: "model",
        providerRevision: 1,
        modelContentDigest: "b".repeat(64),
    },
    restartRequired: true,
};
const files: ModelPreviewFile[] = plan.files.map((file) => ({
    targetPath: file.target.path,
    summary: {
        format: "json",
        changes: [
            {
                path: "defaultModel",
                kind: "replaced",
                before: "old",
                after: "new",
                secret: false,
            },
        ],
        reason: null,
        truncated: false,
    },
    text: null,
}));
const read = vi.mocked(readModelConfigurationPreview);
beforeEach(() => {
    read.mockReset().mockImplementation((_plan, _path, index = null) =>
        Promise.resolve({
            kind: "desktop",
            files: files.map((file, i) => ({
                ...file,
                text:
                    i === index
                        ? {
                              revisionId: `file-${String(i)}`,
                              targetPath: file.targetPath,
                              format: "json",
                              current: {
                                  state: "present",
                                  text: '{"model":"old"}',
                                  bytes: 15,
                              },
                              target: {
                                  state: "present",
                                  text: '{"model":"new"}',
                                  bytes: 15,
                              },
                          }
                        : null,
            })),
        }),
    );
});
afterEach(cleanup);
async function mount() {
    const apply = vi.fn();
    const i18n = await createAppI18n("en");
    const view = render(
        <I18nextProvider i18n={i18n}>
            <ModelPlanPresentation
                plan={plan}
                entryPath="/qa/pi"
                applying={false}
                onApply={apply}
            />
        </I18nextProvider>,
    );
    return { apply, ...view };
}

test("loads summaries first, reads text only on demand, switches files and returns focus", async () => {
    const { apply } = await mount();
    const button = await screen.findByRole("button", {
        name: /View detailed configuration.*models.json/u,
    });
    expect(read).toHaveBeenLastCalledWith(plan, "/qa/pi");
    await userEvent.click(button);
    await screen.findByRole("dialog");
    await screen.findByRole("region", { name: "Differences" });
    await userEvent.click(screen.getByText("Show full configuration text"));
    await screen.findByText("Planned configuration");
    expect(read).toHaveBeenLastCalledWith(plan, "/qa/pi", 0);
    await userEvent.click(
        screen.getByRole("button", { name: "/qa/settings.json" }),
    );
    await waitFor(() => {
        expect(read).toHaveBeenLastCalledWith(plan, "/qa/pi", 1);
    });
    await userEvent.keyboard("{Escape}");
    await waitFor(() => {
        expect(screen.queryByRole("dialog")).toBeNull();
        expect(document.activeElement).toBe(button);
    });
    await userEvent.click(
        screen.getByRole("button", { name: "Apply these settings" }),
    );
    expect(apply).toHaveBeenCalledOnce();
});
test("summary failures block apply and retry the same reviewed plan", async () => {
    read.mockRejectedValueOnce(new VersionRestoreRequestError("plan_stale"));
    const { apply } = await mount();
    await screen.findByRole("alert");
    await userEvent.click(
        screen.getByRole("button", { name: "Apply these settings" }),
    );
    expect(apply).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    await screen.findByText("/qa/models.json");
    await userEvent.click(
        screen.getByRole("button", { name: "Apply these settings" }),
    );
    expect(apply).toHaveBeenCalledOnce();
});
test("detail failures offer retry without replacing the plan", async () => {
    await mount();
    await screen.findByText("/qa/models.json");
    read.mockRejectedValueOnce(new VersionRestoreRequestError("plan_stale"));
    await userEvent.click(
        screen.getByRole("button", {
            name: /View detailed configuration.*models.json/u,
        }),
    );
    await screen.findByRole("alert");
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    await screen.findByRole("region", { name: "Differences" });
    await userEvent.click(screen.getByText("Show full configuration text"));
    await screen.findByText("Planned configuration");
    expect(read).toHaveBeenLastCalledWith(plan, "/qa/pi", 0);
});
test("ignores a summary reply after leaving the page", async () => {
    let finish:
        | ((value: { kind: "desktop"; files: ModelPreviewFile[] }) => void)
        | undefined;
    read.mockReturnValue(
        new Promise((resolve) => {
            finish = resolve;
        }),
    );
    const { unmount, apply } = await mount();
    unmount();
    finish?.({ kind: "desktop", files });
    expect(apply).not.toHaveBeenCalled();
    expect(screen.queryByText("/qa/models.json")).toBeNull();
});
