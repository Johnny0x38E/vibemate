import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";
import {
    getClaudeEffortPreference,
    saveClaudeEffortPreference,
} from "./claudePreference";
import { effortPreferenceFixture } from "../../test/claudePreference";

vi.mock(import("@tauri-apps/api/core"));
const first = effortPreferenceFixture();
const identity = {
    providerId: first.snapshot.model.providerId,
    modelId: first.snapshot.model.modelId,
};
const saved = {
    ...first,
    selectedEffort: "max" as const,
    revision: "a".repeat(32),
};
const request = {
    ...identity,
    expectedModelDigest: first.snapshot.contentDigest,
    expectedRevision: null,
    selectedEffort: "max" as const,
};
beforeEach(() => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(invoke).mockReset();
});

test("browser preview never invents a starting choice or a successful remember action", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(await getClaudeEffortPreference(identity)).toEqual({
        kind: "preview",
    });
    await expect(saveClaudeEffortPreference(request)).rejects.toMatchObject({
        code: "desktop_required",
    });
    expect(invoke).not.toHaveBeenCalled();
});

test("reads one exact binding and sends both CAS tokens when remembering", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(first).mockResolvedValueOnce(saved);
    expect(await getClaudeEffortPreference(identity)).toEqual({
        kind: "desktop",
        preference: first,
    });
    expect(invoke).toHaveBeenNthCalledWith(1, "get_claude_effort_preference", {
        request: identity,
    });
    expect(await saveClaudeEffortPreference(request)).toEqual(saved);
    expect(invoke).toHaveBeenNthCalledWith(2, "save_claude_effort_preference", {
        request,
    });
});

test("accepts a clear with a retained revision and does not infer a model default", async () => {
    const cleared = { ...first, revision: "b".repeat(32) };
    vi.mocked(invoke).mockResolvedValue(cleared);
    expect(
        await saveClaudeEffortPreference({
            ...request,
            selectedEffort: null,
            expectedRevision: saved.revision,
        }),
    ).toEqual(cleared);
});

test.each([
    null,
    {},
    { ...first, extra: "untrusted" },
    { ...first, selectedEffort: "max" },
    { ...saved, revision: "invalid" },
    { ...saved, selectedEffort: "none" },
    { ...saved, selectedEffort: "ultracode" },
    { ...saved, snapshot: { ...first.snapshot, contentDigest: "invalid" } },
    {
        ...saved,
        snapshot: {
            ...first.snapshot,
            model: { ...first.snapshot.model, providerId: "f".repeat(32) },
        },
    },
    {
        ...saved,
        snapshot: {
            ...first.snapshot,
            model: { ...first.snapshot.model, modelId: "another-id" },
        },
    },
])(
    "rejects malformed or misbound preference response %# without retaining it",
    async (value) => {
        vi.mocked(invoke).mockResolvedValue(value);
        await expect(getClaudeEffortPreference(identity)).rejects.toMatchObject(
            { code: "invalid_response" },
        );
    },
);

test("an obsolete but typed effort is returned honestly, not changed to inheritance", async () => {
    const obsolete = {
        ...saved,
        snapshot: {
            ...saved.snapshot,
            model: { ...saved.snapshot.model, thinkingEffort: null },
        },
    };
    vi.mocked(invoke).mockResolvedValue(obsolete);
    expect(await getClaudeEffortPreference(identity)).toEqual({
        kind: "desktop",
        preference: obsolete,
    });
});

test.each([
    { ...saved, selectedEffort: "high" },
    {
        ...saved,
        snapshot: { ...saved.snapshot, contentDigest: "2".repeat(64) },
    },
])(
    "does not report a mismatched save acknowledgment as success %#",
    async (value) => {
        vi.mocked(invoke).mockResolvedValue(value);
        await expect(saveClaudeEffortPreference(request)).rejects.toMatchObject(
            { code: "invalid_response" },
        );
    },
);

test.each([
    "preference_changed",
    "model_changed",
    "effort_not_mapped",
    "write_failed",
])("preserves safe native refusal %s", async (code) => {
    vi.mocked(invoke).mockRejectedValue(code);
    await expect(saveClaudeEffortPreference(request)).rejects.toMatchObject({
        code,
    });
});

test("unknown native diagnostics are discarded", async () => {
    vi.mocked(invoke).mockRejectedValue("private-database-path-sentinel");
    const result = await getClaudeEffortPreference(identity).catch(
        (error: unknown) => error,
    );
    expect(result).toMatchObject({ code: "operation_failed" });
    expect(String(result)).not.toContain("private-database-path-sentinel");
});
