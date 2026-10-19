import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import {
    VersionRestoreRequestError,
    readVersionText,
    type VersionRestorePlan,
    type VersionText,
} from "../../lib/desktop/versionRestore";
import { VersionTextDialog } from "./VersionTextDialog";

vi.mock(import("../../lib/desktop/versionRestore"), async (original) => ({
    ...(await original()),
    readVersionText: vi.fn(),
}));

const ID = "a".repeat(32);
const REVISION = "100-1-0";
const OTHER_REVISION = "101-1-0";
const PATH = "/Users/test/.claude/settings.json";
const OTHER_PATH = "/Users/test/.claude/other.json";

function file(
    revisionId: string,
    targetPath: string,
): VersionRestorePlan["files"][number] {
    return {
        revisionId,
        ordinal: revisionId === REVISION ? 0 : 1,
        targetPath,
        action: "restore",
        reason: null,
        currentDigest: "c".repeat(64),
        targetDigest: "b".repeat(64),
        restoredOwnerRef: null,
        summary: {
            format: "json",
            changes: [],
            truncated: false,
            reason: null,
        },
    };
}

const plan: VersionRestorePlan = {
    entryKey: `operation:${ID}`,
    operationId: ID,
    intent: "apply",
    ownerRef: null,
    createdAt: 100,
    files: [file(REVISION, PATH), file(OTHER_REVISION, OTHER_PATH)],
    restorable: true,
    reason: null,
};

function text(
    revisionId: string,
    current: string,
    target: string,
): VersionText {
    return {
        revisionId,
        targetPath: PATH,
        format: "json",
        current: { state: "present", text: current, bytes: current.length },
        target: { state: "present", text: target, bytes: target.length },
    };
}

const loadText = vi.mocked(readVersionText);

beforeEach(() => {
    loadText.mockReset().mockResolvedValue({
        kind: "desktop",
        text: text(
            REVISION,
            '{\n  "model": "current-model"\n}',
            '{\n  "model": "retained-model"\n}',
        ),
    });
});
afterEach(cleanup);

async function mount(
    onClose = vi.fn(),
    initialRevisionId = REVISION,
): Promise<void> {
    const i18n = await createAppI18n("en");
    render(
        <I18nextProvider i18n={i18n}>
            <VersionTextDialog
                agent="claudeCode"
                plan={plan}
                initialRevisionId={initialRevisionId}
                onClose={onClose}
                returnFocus={() => null}
            />
        </I18nextProvider>,
    );
}

test("shows sanitized current and retained text with a line diff", async () => {
    await mount();
    expect(await screen.findByText("Detailed configuration")).toBeTruthy();
    await screen.findByText(/Values under credential-like keys are masked/u);
    const current = screen.getByRole("region", {
        name: "Current configuration",
    });
    const retained = screen.getByRole("region", {
        name: "Retained version",
    });
    await waitFor(() => {
        expect(current.textContent).toContain("current-model");
        expect(retained.textContent).toContain("retained-model");
    });
    expect(current.textContent).toContain("Original size:");
    expect(screen.getByLabelText(/"model": "current-model"/u)).toBeTruthy();
    expect(
        screen.getByLabelText(/Removed line: .*current-model/u),
    ).toBeTruthy();
    expect(screen.getByLabelText(/Added line: .*retained-model/u)).toBeTruthy();
});

test("switches files and loads each text with its own digest", async () => {
    loadText.mockImplementation((_agent, _entry, revisionId) =>
        Promise.resolve({
            kind: "desktop",
            text: text(
                revisionId,
                revisionId === REVISION ? "current one" : "current two",
                revisionId === REVISION ? "retained one" : "retained two",
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
            plan.entryKey,
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

test("reports absence and unavailable sides without raw content", async () => {
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
    expect(screen.getByText(/This text cannot be shown safely/u)).toBeTruthy();
    expect(
        screen.getByText(/A line diff needs both sides to be present/u),
    ).toBeTruthy();
});

test("a stale preview is reported and can be retried", async () => {
    loadText
        .mockRejectedValueOnce(new VersionRestoreRequestError("plan_stale"))
        .mockResolvedValueOnce({
            kind: "desktop",
            text: text(REVISION, "current", "retained"),
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

test("closes from the button, Escape and initial focus", async () => {
    const onClose = vi.fn();
    await mount(onClose);
    const close = screen.getByRole("button", { name: "Close" });
    await waitFor(() => {
        expect(document.activeElement).toBe(close);
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
