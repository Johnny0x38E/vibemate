import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";
import { RestoreRequestError, type ConfigurableAgent } from "./agents";
import {
    reviewConfigurationResolution,
    resolveConfigurationOperation,
    type ResolutionReview,
    type RevisionObservation,
} from "./recovery";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
const ID = "a".repeat(32);
const revision: RevisionObservation = {
    id: "100-1-0",
    batchId: ID,
    ordinal: 0,
    intent: "apply",
    state: "prepared",
    targetPath: "/Users/test/.pi/agent/models.json",
    previousDigest: "b".repeat(64),
    intendedDigest: "c".repeat(64),
    currentDigest: "c".repeat(64),
    currentMatch: "intended",
    errorCode: null,
    backupState: "available",
};
const review: ResolutionReview = {
    operationId: ID,
    reviewedDigest: "d".repeat(64),
    resolvable: true,
    revisions: [revision],
};

beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(isTauri).mockReturnValue(true);
});

test.each<ConfigurableAgent>(["claudeCode", "pi", "grokBuild"])(
    "%s sends only fixed Agent identity and review digest, not frontend observations",
    async (agent) => {
        vi.mocked(invoke)
            .mockResolvedValueOnce(review)
            .mockResolvedValueOnce({ operationId: ID, state: "applied" });
        expect(await reviewConfigurationResolution(agent, ID)).toEqual({
            kind: "desktop",
            review,
        });
        expect(invoke).toHaveBeenNthCalledWith(
            1,
            "review_configuration_resolution",
            { input: { agent, operationId: ID } },
        );
        expect(await resolveConfigurationOperation(agent, review)).toEqual({
            operationId: ID,
            state: "applied",
        });
        expect(invoke).toHaveBeenNthCalledWith(
            2,
            "resolve_configuration_operation",
            {
                input: {
                    agent,
                    operationId: ID,
                    reviewedDigest: review.reviewedDigest,
                },
            },
        );
    },
);

test("browser preview cannot invent a review or a successful journal mutation", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(await reviewConfigurationResolution("pi", ID)).toEqual({
        kind: "preview",
    });
    await expect(
        resolveConfigurationOperation("pi", review),
    ).rejects.toMatchObject({ code: "desktop_required" });
    expect(invoke).not.toHaveBeenCalled();
});

test.each([
    null,
    { ...review, extra: true },
    { ...review, operationId: "b".repeat(32) },
    { ...review, reviewedDigest: "short" },
    { ...review, resolvable: "yes" },
    { ...review, revisions: [] },
    { ...review, revisions: [revision, revision] },
    { ...review, revisions: [{ ...revision, ordinal: -1 }] },
    { ...review, revisions: [{ ...revision, targetPath: "relative" }] },
    { ...review, revisions: [{ ...revision, id: "../outside" }] },
    { ...review, revisions: [{ ...revision, intent: "automatic-restore" }] },
    { ...review, revisions: [{ ...revision, extra: "synthetic-secret" }] },
    { ...review, revisions: [{ ...revision, currentMatch: "previous" }] },
    { ...review, revisions: [{ ...revision, currentDigest: null }] },
    { ...review, revisions: [{ ...revision, backupState: "not_needed" }] },
    { ...review, revisions: [{ ...revision, backupState: "unavailable" }] },
    { ...review, revisions: [{ ...revision, errorCode: "target_unreadable" }] },
    {
        ...review,
        revisions: [
            {
                ...revision,
                state: "completed",
                currentMatch: "previous",
                currentDigest: revision.previousDigest,
            },
        ],
    },
])(
    "rejects malformed or contradictory observation reviews %#",
    async (payload) => {
        vi.mocked(invoke).mockResolvedValue(payload);
        await expect(
            reviewConfigurationResolution("pi", ID),
        ).rejects.toMatchObject({ code: "invalid_response" });
    },
);

test.each<RevisionObservation>([
    {
        ...revision,
        currentMatch: "previous",
        currentDigest: revision.previousDigest,
    },
    {
        ...revision,
        intent: "restore",
        intendedDigest: null,
        currentDigest: null,
        currentMatch: "intended",
    },
    { ...revision, intent: "version_restore" },
    { ...revision, previousDigest: null, backupState: "not_needed" },
    {
        ...revision,
        currentMatch: "both",
        previousDigest: revision.currentDigest,
    },
    { ...revision, currentMatch: "diverged", currentDigest: "f".repeat(64) },
    {
        ...revision,
        currentMatch: "unreadable",
        currentDigest: null,
        errorCode: "symlink_refused",
    },
    { ...revision, backupState: "unavailable" },
])(
    "accepts truthful absence, direction and blocked observations %# without inventing eligibility",
    async (observed) => {
        const payload = { ...review, resolvable: false, revisions: [observed] };
        vi.mocked(invoke).mockResolvedValue(payload);
        expect(await reviewConfigurationResolution("pi", ID)).toEqual({
            kind: "desktop",
            review: payload,
        });
    },
);

test.each([
    { operationId: "b".repeat(32), state: "applied" },
    { operationId: ID, state: "effective" },
    { operationId: ID, state: "applied", config: "synthetic-secret" },
])("rejects a mismatched or unsafe resolution result %#", async (payload) => {
    vi.mocked(invoke).mockResolvedValue(payload);
    await expect(
        resolveConfigurationOperation("pi", review),
    ).rejects.toMatchObject({ code: "invalid_response" });
});

test("preserves safe stale errors and drops unknown IPC diagnostics", async () => {
    vi.mocked(invoke)
        .mockRejectedValueOnce("plan_stale")
        .mockRejectedValueOnce("synthetic-sensitive-diagnostic");
    await expect(
        resolveConfigurationOperation("pi", review),
    ).rejects.toMatchObject({ code: "plan_stale" });
    await expect(reviewConfigurationResolution("pi", ID)).rejects.toEqual(
        new RestoreRequestError("operation_failed"),
    );
});
