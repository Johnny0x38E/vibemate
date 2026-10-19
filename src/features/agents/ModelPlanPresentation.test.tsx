import { cleanup, render, screen } from "@testing-library/react";
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
                text: i === index ? (files[0]?.text ?? null) : null,
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

test("shows an inline preview with summary on top and field diffs below", async () => {
    const { apply } = await mount();
    // Inline in the Write dialog host — never a nested preview Dialog.
    expect(screen.queryByRole("dialog")).toBeNull();
    await screen.findByText("/qa/models.json");
    expect(read).toHaveBeenLastCalledWith(plan, "/qa/pi");
    expect(screen.getAllByText("defaultModel").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Replaced").length).toBeGreaterThan(0);
    expect(screen.queryByText("Show full configuration text")).toBeNull();
    expect(screen.queryByText("Planned configuration")).toBeNull();
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
    expect(screen.queryByRole("dialog")).toBeNull();
    await userEvent.click(
        screen.getByRole("button", { name: "Apply these settings" }),
    );
    expect(apply).toHaveBeenCalledOnce();
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
