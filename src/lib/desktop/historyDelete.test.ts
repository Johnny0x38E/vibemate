import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";
import {
    HistoryDeleteRequestError,
    deleteHistoryEntry,
    type HistoryDeleteOutcome,
} from "./historyDelete";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));

const ID = "a".repeat(32);
const BACKUP = "100-1-0";
const PATH = `/Users/test/Library/Application Support/vibemate/backups/${BACKUP}`;

function outcome(
    overrides: Partial<HistoryDeleteOutcome> = {},
): HistoryDeleteOutcome {
    return {
        entryKey: `operation:${ID}`,
        state: "completed",
        leftovers: [],
        ...overrides,
    };
}

beforeEach(() => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(invoke).mockReset();
});

test("browser preview requires the desktop app", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    await expect(
        deleteHistoryEntry("claudeCode", `operation:${ID}`),
    ).rejects.toMatchObject({
        code: "desktop_required",
    });
    expect(invoke).not.toHaveBeenCalled();
});

test("rejects a malformed entry key before IPC", async () => {
    await expect(
        deleteHistoryEntry("claudeCode", "operation:nope"),
    ).rejects.toBeInstanceOf(HistoryDeleteRequestError);
    expect(invoke).not.toHaveBeenCalled();
});

test("accepts a completed delete outcome", async () => {
    vi.mocked(invoke).mockResolvedValue(outcome());
    await expect(
        deleteHistoryEntry("claudeCode", `operation:${ID}`),
    ).resolves.toEqual(outcome());
    expect(invoke).toHaveBeenCalledWith("delete_configuration_history_entry", {
        input: { agent: "claudeCode", entryKey: `operation:${ID}` },
    });
});

test("accepts a failed outcome with leftovers", async () => {
    const failed = outcome({
        state: "failed",
        leftovers: [
            {
                backupId: BACKUP,
                path: PATH,
                errorCode: "backup_unavailable",
            },
        ],
    });
    vi.mocked(invoke).mockResolvedValue(failed);
    await expect(deleteHistoryEntry("pi", `batch:${BACKUP}`)).resolves.toEqual(
        failed,
    );
});

test("rejects completed outcomes that still list leftovers", async () => {
    vi.mocked(invoke).mockResolvedValue(
        outcome({
            leftovers: [
                {
                    backupId: BACKUP,
                    path: PATH,
                    errorCode: "backup_unavailable",
                },
            ],
        }),
    );
    await expect(
        deleteHistoryEntry("claudeCode", `operation:${ID}`),
    ).rejects.toMatchObject({ code: "invalid_response" });
});

test("rejects unknown response fields", async () => {
    vi.mocked(invoke).mockResolvedValue({ ...outcome(), secret: "x" });
    await expect(
        deleteHistoryEntry("claudeCode", `operation:${ID}`),
    ).rejects.toMatchObject({ code: "invalid_response" });
});

test("maps protection error codes from IPC", async () => {
    vi.mocked(invoke).mockRejectedValue("history_referenced");
    await expect(
        deleteHistoryEntry("grokBuild", `operation:${ID}`),
    ).rejects.toMatchObject({ code: "history_referenced" });
});
