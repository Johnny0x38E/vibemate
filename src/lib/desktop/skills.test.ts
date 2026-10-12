import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";
import {
    importSkillFolder,
    listSkills,
    pickSkillFolder,
    setSkillSource,
    SkillRequestError,
    type SkillRecord,
} from "./skills";
vi.mock(import("@tauri-apps/api/core"));

const ID = "0123456789abcdef0123456789abcdef";
const OTHER_ID = "fedcba9876543210fedcba9876543210";
const DIGEST = "ab".repeat(32);
const row: SkillRecord = {
    id: ID,
    name: "demo-skill",
    description: "Explains the demo skill.",
    originPath: "/tmp/sources/demo",
    sourceUrl: null,
    sourceRevision: null,
    contentDigest: DIGEST,
    fileCount: 2,
    byteSize: 128,
    revision: 1,
    importedAtMs: 1_700_000_000_000,
    updatedAtMs: 1_700_000_000_000,
};

beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(isTauri).mockReturnValue(true);
});

test("preview reads nothing and rejects every write", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    expect(await listSkills(null)).toEqual({ kind: "preview" });
    await expect(pickSkillFolder("Choose")).rejects.toMatchObject({
        code: "desktop_required",
    });
    await expect(importSkillFolder("/tmp/sources/demo")).rejects.toMatchObject({
        code: "desktop_required",
    });
    await expect(
        setSkillSource({
            id: ID,
            expectedRevision: 1,
            url: "https://example.com/skills",
            revision: null,
        }),
    ).rejects.toMatchObject({ code: "desktop_required" });
    expect(vi.mocked(invoke)).not.toHaveBeenCalled();
});

test("a valid page is returned with the last-row cursor", async () => {
    const second: SkillRecord = {
        ...row,
        id: "fedcba9876543210fedcba9876543210",
        name: "other-skill",
    };
    vi.mocked(invoke).mockResolvedValue({
        items: [row, second],
        nextCursor: `${second.name}\n${second.id}`,
        total: 5,
    });
    const result = await listSkills(null, 2);
    expect(result).toEqual({
        kind: "desktop",
        page: {
            items: [row, second],
            nextCursor: `${second.name}\n${second.id}`,
            total: 5,
        },
    });
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("list_skills", {
        after: null,
        limit: 2,
    });
});

test("malformed pages are rejected instead of shown", async () => {
    const cases: unknown[] = [
        // Unknown extra field.
        { items: [{ ...row, extra: true }], nextCursor: null, total: 1 },
        // Digest that is not a SHA-256 hex string.
        {
            items: [{ ...row, contentDigest: "short" }],
            nextCursor: null,
            total: 1,
        },
        // Skill name with a space.
        { items: [{ ...row, name: "demo skill" }], nextCursor: null, total: 1 },
        // Revision without a source address.
        {
            items: [{ ...row, source_revision: "v1", sourceRevision: "v1" }],
            nextCursor: null,
            total: 1,
        },
        // No origin path and no source address.
        { items: [{ ...row, originPath: null }], nextCursor: null, total: 1 },
        // Negative timestamps and out-of-order updates.
        { items: [{ ...row, updatedAtMs: 0 }], nextCursor: null, total: 1 },
        // Cursor that does not repeat the last row.
        {
            items: [row],
            nextCursor: "other-skill\n0123456789abcdef0123456789abcdef",
            total: 1,
        },
        // Cursor without the name separator.
        { items: [row], nextCursor: ID, total: 1 },
        // More rows than the requested limit.
        { items: [row, row], nextCursor: null, total: 2 },
        // A missing, negative or impossible library total.
        { items: [row], nextCursor: null },
        { items: [row], nextCursor: null, total: -1 },
        { items: [row], nextCursor: null, total: 0 },
        { items: [row], nextCursor: null, total: 100_001 },
        { items: [row], nextCursor: null, total: 1.5 },
    ];
    for (const value of cases) {
        vi.mocked(invoke).mockResolvedValue(value);
        await expect(listSkills(null, 1)).rejects.toMatchObject({
            code: "invalid_response",
        });
    }
});

test("a page after the first must continue in name order", async () => {
    const cursor = `${row.name}\n${row.id}`;
    // The same row again would repeat a visible skill.
    vi.mocked(invoke).mockResolvedValue({
        items: [row],
        nextCursor: null,
        total: 1,
    });
    await expect(listSkills(cursor, 20)).rejects.toMatchObject({
        code: "invalid_response",
    });
    const later: SkillRecord = { ...row, name: "zebra" };
    vi.mocked(invoke).mockResolvedValue({
        items: [later],
        nextCursor: null,
        total: 2,
    });
    await expect(listSkills(cursor, 20)).resolves.toMatchObject({
        kind: "desktop",
    });
});

test("the folder picker reports a choice or a cancel", async () => {
    vi.mocked(invoke).mockResolvedValue({ kind: "cancelled" });
    await expect(pickSkillFolder("Choose a skill folder")).resolves.toEqual({
        kind: "cancelled",
    });
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("pick_skill_folder", {
        title: "Choose a skill folder",
    });

    vi.mocked(invoke).mockResolvedValue({
        kind: "selected",
        path: "/tmp/sources/demo",
    });
    await expect(pickSkillFolder("Choose a skill folder")).resolves.toEqual({
        kind: "selected",
        path: "/tmp/sources/demo",
    });

    const cases: unknown[] = [
        // A selected folder without a path.
        { kind: "selected" },
        // Cancellation must not carry extra fields.
        { kind: "cancelled", path: "/tmp/x" },
        // An unknown outcome.
        { kind: "failed" },
        // A path the interface cannot display.
        { kind: "selected", path: "/tmp/\u0000bad" },
        { kind: "selected", path: "   " },
    ];
    for (const value of cases) {
        vi.mocked(invoke).mockResolvedValue(value);
        await expect(pickSkillFolder("Choose")).rejects.toMatchObject({
            code: "invalid_response",
        });
    }
});

test("a folder import reports added skills and refused folders", async () => {
    vi.mocked(invoke).mockResolvedValue({
        imported: [row],
        skipped: [{ folder: "broken", code: "manifest_invalid" }],
    });
    await expect(importSkillFolder("/tmp/sources/demo")).resolves.toEqual({
        imported: [row],
        skipped: [{ folder: "broken", code: "manifest_invalid" }],
    });
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("import_skill_folder", {
        request: { path: "/tmp/sources/demo" },
    });
});

test("impossible import reports are rejected", async () => {
    const cases: unknown[] = [
        // Nothing imported and nothing refused cannot describe a chosen folder.
        { imported: [], skipped: [] },
        // Duplicate rows for one skill.
        { imported: [row, row], skipped: [] },
        // A reason code the interface does not translate.
        {
            imported: [],
            skipped: [{ folder: "broken", code: "unknown_reason" }],
        },
        // A transport code is not a folder reason.
        {
            imported: [],
            skipped: [{ folder: "broken", code: "desktop_required" }],
        },
        // A folder label that is empty, too long or contains control characters.
        { imported: [], skipped: [{ folder: "  ", code: "name_taken" }] },
        {
            imported: [],
            skipped: [{ folder: "x".repeat(129), code: "name_taken" }],
        },
        {
            imported: [],
            skipped: [{ folder: "bad\u0007", code: "name_taken" }],
        },
        // An import never records an upstream source for the user.
        {
            imported: [{ ...row, sourceUrl: "https://example.com/skills" }],
            skipped: [],
        },
    ];
    for (const value of cases) {
        vi.mocked(invoke).mockResolvedValue(value);
        await expect(
            importSkillFolder("/tmp/sources/demo"),
        ).rejects.toMatchObject({
            code: "invalid_response",
        });
    }

    // A second imported skill keeps the report valid.
    const second: SkillRecord = { ...row, id: OTHER_ID, name: "other-skill" };
    vi.mocked(invoke).mockResolvedValue({
        imported: [row, second],
        skipped: [],
    });
    await expect(importSkillFolder("/tmp/sources/demo")).resolves.toMatchObject(
        {
            imported: [row, second],
        },
    );
});

test("source records must advance the revision they were edited from", async () => {
    const updated: SkillRecord = {
        ...row,
        sourceUrl: "https://example.com/skills",
        sourceRevision: "v1.2.0",
        revision: 2,
        updatedAtMs: row.updatedAtMs + 5,
    };
    vi.mocked(invoke).mockResolvedValue(updated);
    await expect(
        setSkillSource({
            id: ID,
            expectedRevision: 1,
            url: "https://example.com/skills",
            revision: "v1.2.0",
        }),
    ).resolves.toEqual(updated);

    vi.mocked(invoke).mockResolvedValue({ ...updated, id: "f".repeat(32) });
    await expect(
        setSkillSource({
            id: ID,
            expectedRevision: 1,
            url: "https://example.com/skills",
            revision: null,
        }),
    ).rejects.toMatchObject({ code: "invalid_response" });

    vi.mocked(invoke).mockResolvedValue({ ...updated, revision: 5 });
    await expect(
        setSkillSource({
            id: ID,
            expectedRevision: 1,
            url: "https://example.com/skills",
            revision: null,
        }),
    ).rejects.toMatchObject({ code: "invalid_response" });
});

test("domain codes are kept and unknown failures are generic", async () => {
    vi.mocked(invoke).mockRejectedValue("no_skills_found");
    await expect(importSkillFolder("/tmp/sources/demo")).rejects.toMatchObject({
        code: "no_skills_found",
    });
    vi.mocked(invoke).mockRejectedValue(new Error("transport"));
    await expect(importSkillFolder("/tmp/sources/demo")).rejects.toMatchObject({
        code: "operation_failed",
    });
    vi.mocked(invoke).mockRejectedValue("not_a_documented_code");
    await expect(importSkillFolder("/tmp/sources/demo")).rejects.toMatchObject({
        code: "operation_failed",
    });
    await expect(importSkillFolder("/tmp/sources/demo")).rejects.toBeInstanceOf(
        SkillRequestError,
    );
});
