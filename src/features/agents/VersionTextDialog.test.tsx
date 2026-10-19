import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import {
    VersionRestoreRequestError,
    readVersionText,
    type VersionText,
} from "../../lib/desktop/versionRestore";
import { CurrentConfigurationDialog } from "./VersionTextDialog";

vi.mock(import("../../lib/desktop/versionRestore"), async (original) => ({
    ...(await original()),
    readVersionText: vi.fn(),
}));

const ID = "a".repeat(32);
const REVISION = "100-1-0";
const OTHER_REVISION = "101-1-0";
const PATH = "/Users/test/.claude/settings.json";
const OTHER_PATH = "/Users/test/.claude/other.json";
const ENTRY = `operation:${ID}`;

const files = [
    {
        revisionId: REVISION,
        targetPath: PATH,
        currentDigest: "c".repeat(64),
    },
    {
        revisionId: OTHER_REVISION,
        targetPath: OTHER_PATH,
        currentDigest: "c".repeat(64),
    },
];

function text(revisionId: string, current: string): VersionText {
    return {
        revisionId,
        targetPath: PATH,
        format: "json",
        current: { state: "present", text: current, bytes: current.length },
        target: { state: "present", text: "{}", bytes: 2 },
    };
}

const loadText = vi.mocked(readVersionText);

beforeEach(() => {
    loadText.mockReset().mockResolvedValue({
        kind: "desktop",
        text: text(REVISION, '{\n  "model": "current-model"\n}'),
    });
});
afterEach(cleanup);

async function mount(onClose = vi.fn()): Promise<void> {
    const i18n = await createAppI18n("en");
    render(
        <I18nextProvider i18n={i18n}>
            <CurrentConfigurationDialog
                agent="claudeCode"
                entryKey={ENTRY}
                files={files}
                initialRevisionId={REVISION}
                onClose={onClose}
                returnFocus={() => null}
            />
        </I18nextProvider>,
    );
}

test("shows sanitized current configuration text only", async () => {
    await mount();
    expect(await screen.findByText("Detailed configuration")).toBeTruthy();
    await screen.findByText(/Values under credential-like keys are masked/u);
    const current = await screen.findByRole("region", {
        name: "Current configuration",
    });
    await waitFor(() => {
        expect(current.textContent).toContain("current-model");
    });
    expect(current.textContent).toContain("Original size:");
    // No retained/planned dual column and no full-text expand.
    expect(screen.queryByText("Show full configuration text")).toBeNull();
    expect(screen.queryByText("Retained version")).toBeNull();
    expect(screen.queryByText("Planned configuration")).toBeNull();
    expect(screen.queryByRole("region", { name: "Differences" })).toBeNull();
});

test("switches files and loads each text with its own digest", async () => {
    loadText.mockImplementation((_agent, _entry, revisionId) =>
        Promise.resolve({
            kind: "desktop",
            text: text(
                revisionId,
                revisionId === REVISION ? "current one" : "current two",
            ),
        }),
    );
    await mount();
    const current = await screen.findByRole("region", {
        name: "Current configuration",
    });
    await waitFor(() => {
        expect(current.textContent).toContain("current one");
    });
    await userEvent.click(screen.getByRole("button", { name: OTHER_PATH }));
    await waitFor(() => {
        expect(loadText).toHaveBeenCalledWith(
            "claudeCode",
            ENTRY,
            OTHER_REVISION,
            "c".repeat(64),
        );
    });
    await waitFor(() => {
        expect(
            screen.getByRole("region", {
                name: "Current configuration",
            }).textContent,
        ).toContain("current two");
    });
});

test("reports absence and unavailable current text without raw content", async () => {
    loadText.mockResolvedValue({
        kind: "desktop",
        text: {
            revisionId: REVISION,
            targetPath: PATH,
            format: "json",
            current: { state: "absent" },
            target: {
                state: "unavailable",
                reason: "backup_unavailable",
            },
        },
    });
    await mount();
    expect(
        await screen.findByText("The file did not exist at this point."),
    ).toBeTruthy();
});

test("a stale preview is reported and can be retried", async () => {
    loadText
        .mockRejectedValueOnce(new VersionRestoreRequestError("plan_stale"))
        .mockResolvedValueOnce({
            kind: "desktop",
            text: text(REVISION, "current"),
        });
    await mount();
    expect(
        await screen.findByText(
            /A file changed since the preview, so nothing was written/u,
        ),
    ).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    const current = await screen.findByRole("region", {
        name: "Current configuration",
    });
    await waitFor(() => {
        expect(current.textContent).toContain("current");
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
