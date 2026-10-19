import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import {
    VersionRestoreRequestError,
    readCurrentConfigurationText,
    type CurrentConfigText,
} from "../../lib/desktop/versionRestore";
import { CurrentConfigurationDialog } from "./VersionTextDialog";

vi.mock(import("../../lib/desktop/versionRestore"), async (original) => ({
    ...(await original()),
    readCurrentConfigurationText: vi.fn(),
}));

const PATH = "/Users/test/.claude/settings.json";
const OTHER_PATH = "/Users/test/.claude/other.json";
const files = [PATH, OTHER_PATH];

function text(targetPath: string, current: string): CurrentConfigText {
    return {
        targetPath,
        format: "json",
        current: { state: "present", text: current, bytes: current.length },
    };
}

const loadText = vi.mocked(readCurrentConfigurationText);

function renderedText(): string {
    return document.querySelector("pre")?.textContent ?? "";
}

beforeEach(() => {
    loadText.mockReset().mockResolvedValue({
        kind: "desktop",
        text: text(PATH, '{\n  "model": "current-model"\n}'),
    });
});
afterEach(cleanup);

async function mount(onClose = vi.fn()): Promise<void> {
    const i18n = await createAppI18n("en");
    render(
        <I18nextProvider i18n={i18n}>
            <CurrentConfigurationDialog
                agent="claudeCode"
                files={files}
                onClose={onClose}
                returnFocus={() => null}
            />
        </I18nextProvider>,
    );
}

test("shows sanitized current configuration text only", async () => {
    await mount();
    expect(await screen.findByText("Detailed configuration")).toBeTruthy();
    // Credential/Rust masking notice is intentionally not shown in this dialog.
    expect(
        screen.queryByText(/Values under credential-like keys are masked/u),
    ).toBeNull();
    await waitFor(() => {
        expect(renderedText()).toContain("current-model");
    });
    // The current-config heading and original size are not part of this view.
    expect(screen.queryByText("Current configuration")).toBeNull();
    expect(screen.queryByText(/Original size/u)).toBeNull();
    // No retained/planned dual column and no full-text expand.
    expect(screen.queryByText("Show full configuration text")).toBeNull();
    expect(screen.queryByText("Retained version")).toBeNull();
    expect(screen.queryByText("Planned configuration")).toBeNull();
    expect(screen.queryByRole("region", { name: "Differences" })).toBeNull();
});

test("switches files and loads each text by its target path", async () => {
    loadText.mockImplementation((_agent, targetPath) =>
        Promise.resolve({
            kind: "desktop",
            text: text(
                targetPath,
                targetPath === PATH ? "current one" : "current two",
            ),
        }),
    );
    await mount();
    await waitFor(() => {
        expect(renderedText()).toContain("current one");
    });
    await userEvent.click(screen.getByRole("button", { name: OTHER_PATH }));
    await waitFor(() => {
        expect(loadText).toHaveBeenCalledWith("claudeCode", OTHER_PATH);
    });
    await waitFor(() => {
        expect(renderedText()).toContain("current two");
    });
});

test("reports absence and unavailable current text without raw content", async () => {
    loadText.mockResolvedValue({
        kind: "desktop",
        text: {
            targetPath: PATH,
            format: "json",
            current: { state: "absent" },
        },
    });
    await mount();
    expect(
        await screen.findByText("The file did not exist at this point."),
    ).toBeTruthy();
});

test("a failed read is reported and can be retried", async () => {
    loadText
        .mockRejectedValueOnce(
            new VersionRestoreRequestError("operation_failed"),
        )
        .mockResolvedValueOnce({
            kind: "desktop",
            text: text(PATH, "current"),
        });
    await mount();
    expect(
        await screen.findByText(/The restore could not be completed/u),
    ).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => {
        expect(renderedText()).toContain("current");
    });
    expect(loadText).toHaveBeenCalledTimes(2);
});

test("browser preview explains that text is unavailable", async () => {
    loadText.mockResolvedValue({ kind: "preview" });
    await mount();
    expect(
        await screen.findByText(/browser preview cannot read configuration/u),
    ).toBeTruthy();
});

test("focuses the title without skipping content and closes from the button or Escape", async () => {
    const onClose = vi.fn();
    await mount(onClose);
    const close = screen.getByRole("button", { name: "Close" });
    await waitFor(() => {
        expect(document.activeElement).toBe(
            screen.getByRole("heading", { name: "Detailed configuration" }),
        );
    });
    await userEvent.click(close);
    await waitFor(() => {
        expect(onClose).toHaveBeenCalledTimes(1);
    });
    await userEvent.keyboard("{Escape}");
    await waitFor(() => {
        expect(onClose.mock.calls.length).toBeGreaterThan(1);
    });
});
