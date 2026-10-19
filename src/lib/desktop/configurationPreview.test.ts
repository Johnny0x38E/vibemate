import { beforeEach, expect, test, vi } from "vitest";
import { invoke, isTauri } from "@tauri-apps/api/core";
import {
    readModelConfigurationPreview,
    type ModelConfigurationPlan,
} from "./configurationPreview";
import { VersionRestoreRequestError } from "./versionRestore";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));

const plan: ModelConfigurationPlan = {
    agent: "claudeCode",
    target: {
        path: "/qa/settings.json",
        exists: true,
        symlink: false,
        contentDigest: "a".repeat(64),
    },
    source: {
        providerId: "a".repeat(32),
        modelId: "model",
        providerRevision: 1,
        modelContentDigest: "b".repeat(64),
    },
    changes: [],
    notices: [],
    restartRequired: true,
};
const file = {
    targetPath: plan.target.path,
    summary: {
        format: "json",
        changes: [
            {
                path: "model",
                kind: "replaced",
                before: "a",
                after: "b",
                secret: false,
            },
        ],
        truncated: false,
        reason: null,
    },
    text: null,
};
beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(invoke).mockResolvedValue([file]);
});

test("browser preview never invokes or invents file data", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(await readModelConfigurationPreview(plan, null)).toEqual({
        kind: "preview",
    });
    expect(invoke).not.toHaveBeenCalled();
});
test("binds summaries and on-demand text to the entire reviewed plan", async () => {
    expect(await readModelConfigurationPreview(plan, null)).toEqual({
        kind: "desktop",
        files: [file],
    });
    expect(invoke).toHaveBeenCalledWith("read_model_configuration_preview", {
        input: {
            agent: "claudeCode",
            providerId: plan.source.providerId,
            modelId: "model",
            entryPath: null,
            reviewed: plan,
            fileIndex: null,
        },
    });
    const text = {
        revisionId: "file-0",
        targetPath: file.targetPath,
        format: "json",
        current: { state: "present", text: "{}", bytes: 2 },
        target: { state: "present", text: "{}", bytes: 2 },
    };
    vi.mocked(invoke).mockResolvedValue([{ ...file, text }]);
    expect(await readModelConfigurationPreview(plan, null, 0)).toEqual({
        kind: "desktop",
        files: [{ ...file, text }],
    });
});
const malformedPages: unknown[] = [
    [],
    [file, file],
    [{ ...file, extra: "untrusted" }],
    [{ ...file, targetPath: "/other.json" }],
    [{ ...file, summary: { ...file.summary, format: "toml" } }],
    [
        {
            ...file,
            summary: {
                ...file.summary,
                changes: [
                    {
                        path: "x",
                        kind: "added",
                        before: "not absent",
                        after: "a",
                        secret: false,
                    },
                ],
            },
        },
    ],
    [
        {
            ...file,
            text: {
                revisionId: "file-0",
                targetPath: file.targetPath,
                format: "json",
                current: { state: "absent" },
                target: { state: "absent" },
            },
        },
    ],
];
test.each(malformedPages.map((response) => ({ response })))(
    "rejects malformed or mismatched summary pages without exposing payloads: $response",
    async ({ response }) => {
        vi.mocked(invoke).mockResolvedValue(response);
        await expect(
            readModelConfigurationPreview(plan, null),
        ).rejects.toMatchObject({ code: "invalid_response" });
    },
);
test("refuses missing requested text and invalid indexes", async () => {
    await expect(
        readModelConfigurationPreview(plan, null, 0),
    ).rejects.toMatchObject({ code: "invalid_response" });
    vi.mocked(invoke).mockClear();
    await expect(
        readModelConfigurationPreview(plan, null, 1),
    ).rejects.toMatchObject({ code: "invalid_request" });
    expect(invoke).not.toHaveBeenCalled();
});
test("keeps stale-plan errors and discards unexpected diagnostics", async () => {
    vi.mocked(invoke).mockRejectedValue("plan_stale");
    await expect(
        readModelConfigurationPreview(plan, null),
    ).rejects.toMatchObject({ code: "plan_stale" });
    vi.mocked(invoke).mockRejectedValue({ secret: "synthetic-private" });
    const error: unknown = await readModelConfigurationPreview(
        plan,
        null,
    ).catch((error: unknown) => error);
    expect(error).toBeInstanceOf(VersionRestoreRequestError);
    expect(JSON.stringify(error)).not.toContain("synthetic-private");
});
