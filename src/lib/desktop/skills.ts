import { invoke, isTauri } from "@tauri-apps/api/core";
import { hasExactKeys, isObject } from "./providers";

/** Safe domain codes; no arbitrary backend message may reach the UI. */
const SKILL_CODES = [
    "storage_unavailable",
    "read_failed",
    "write_failed",
    "invalid_request",
    "source_path_invalid",
    "source_not_found",
    "source_not_directory",
    "source_unsupported_entry",
    "source_too_large",
    "source_changed",
    "manifest_missing",
    "manifest_invalid",
    "no_skills_found",
    "name_taken",
    "source_url_invalid",
    "source_revision_invalid",
    "not_found",
    "revision_conflict",
    "invalid_stored_skill",
    "outcome_unknown",
    "operation_failed",
] as const;
/** Sanitized domain, preview or response-validation failure. */
export type SkillErrorCode =
    (typeof SKILL_CODES)[number] | "desktop_required" | "invalid_response";
/** Safe errors deliberately discard the original payload, including paths. */
export class SkillRequestError extends Error {
    readonly code: SkillErrorCode;
    constructor(code: SkillErrorCode) {
        super("Skill library request failed.");
        this.name = "SkillRequestError";
        this.code = code;
    }
}
/** Safe metadata for one imported skill; no file content crosses IPC. */
export interface SkillRecord {
    id: string;
    name: string;
    description: string;
    originPath: string | null;
    sourceUrl: string | null;
    sourceRevision: string | null;
    contentDigest: string;
    fileCount: number;
    byteSize: number;
    revision: number;
    importedAtMs: number;
    updatedAtMs: number;
}
/** One bounded cursor page plus the library total, or an honest browser preview. */
export type SkillPageResult =
    | { kind: "preview" }
    | {
          kind: "desktop";
          page: {
              items: SkillRecord[];
              nextCursor: string | null;
              total: number;
          };
      };
/** The chosen folder, or the user closing the system dialog. */
export type PickedFolder =
    { kind: "cancelled" } | { kind: "selected"; path: string };
/** One skill folder that could not be imported, with a safe reason code. */
export interface SkippedSkill {
    folder: string;
    code: SkillErrorCode;
}
/** What one chosen folder produced: added skills and refused folders. */
export interface ImportReport {
    imported: SkillRecord[];
    skipped: SkippedSkill[];
}

/** Skill names follow the library rules: letters, digits, dot, dash, underscore. */
const NAME_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/;
const DIGEST_PATTERN = /^[0-9a-f]{64}$/;
const ID_PATTERN = /^[0-9a-f]{32}$/;
/**
 * Import bounds, mirroring the Rust constants and the `skill` table constraints.
 * A response outside them is rejected: the backend would not have produced it.
 */
const MAX_FILES = 2048;
const MAX_BYTES = 64 * 1024 * 1024;
/** How many skills one chosen folder may contribute. */
const MAX_CANDIDATES = 64;
/** Sanity bound for the reported library total. */
const MAX_TOTAL = 100_000;

function invalid(): never {
    throw new SkillRequestError("invalid_response");
}
/**
 * Whether text carries a control character. Such a value cannot be shown as a
 * path or a folder label, so a response that contains one is rejected instead of
 * being rendered. Written as a code-point loop because a control-character
 * regular expression is not allowed in this project.
 */
function hasControlCharacter(value: string): boolean {
    for (const character of value) {
        const code = character.codePointAt(0) ?? 0;
        if (code < 0x20 || code === 0x7f) return true;
    }
    return false;
}
function object(
    value: unknown,
    keys: readonly string[],
): Record<string, unknown> {
    if (!isObject(value) || !hasExactKeys(value, keys)) invalid();
    return value;
}
function string(value: unknown): string {
    if (typeof value !== "string") invalid();
    return value;
}
function optionalString(value: unknown): string | null {
    return value === null ? null : string(value);
}
function integer(value: unknown, minimum: number): number {
    if (
        typeof value !== "number" ||
        !Number.isSafeInteger(value) ||
        value < minimum
    )
        invalid();
    return value;
}
function record(value: unknown): SkillRecord {
    const data = object(value, [
        "id",
        "name",
        "description",
        "originPath",
        "sourceUrl",
        "sourceRevision",
        "contentDigest",
        "fileCount",
        "byteSize",
        "revision",
        "importedAtMs",
        "updatedAtMs",
    ]);
    const row: SkillRecord = {
        id: string(data["id"]),
        name: string(data["name"]),
        description: string(data["description"]),
        originPath: optionalString(data["originPath"]),
        sourceUrl: optionalString(data["sourceUrl"]),
        sourceRevision: optionalString(data["sourceRevision"]),
        contentDigest: string(data["contentDigest"]),
        fileCount: integer(data["fileCount"], 1),
        byteSize: integer(data["byteSize"], 1),
        revision: integer(data["revision"], 1),
        importedAtMs: integer(data["importedAtMs"], 0),
        updatedAtMs: integer(data["updatedAtMs"], 0),
    };
    if (
        !ID_PATTERN.test(row.id) ||
        !NAME_PATTERN.test(row.name) ||
        row.description.trim() === "" ||
        row.description.length > 1024 ||
        !DIGEST_PATTERN.test(row.contentDigest) ||
        row.fileCount > MAX_FILES ||
        row.byteSize > MAX_BYTES ||
        row.updatedAtMs < row.importedAtMs ||
        // Every row explains where its content came from.
        (row.originPath === null && row.sourceUrl === null)
    )
        invalid();
    // A revision belongs to an address, and every row explains its origin.
    if (row.sourceRevision !== null && row.sourceUrl === null) invalid();
    return row;
}
/** The cursor carries both ordering parts: skill name, then identity. */
function cursor(value: unknown): { name: string; id: string } {
    const text = string(value);
    const separator = text.indexOf("\n");
    if (separator < 0) invalid();
    const name = text.slice(0, separator);
    const id = text.slice(separator + 1);
    if (!NAME_PATTERN.test(name) || !ID_PATTERN.test(id)) invalid();
    return { name, id };
}
function isAfter(
    row: SkillRecord,
    position: { name: string; id: string },
): boolean {
    return (
        row.name > position.name ||
        (row.name === position.name && row.id > position.id)
    );
}
async function call(
    command: string,
    args: Record<string, unknown>,
): Promise<unknown> {
    try {
        return await invoke<unknown>(command, args);
    } catch (error: unknown) {
        throw new SkillRequestError(
            SKILL_CODES.find((code) => code === error) ?? "operation_failed",
        );
    }
}
function desktop(): void {
    if (!isTauri()) throw new SkillRequestError("desktop_required");
}
/** Read bounded metadata; preview performs no IPC and reads no directory. */
export async function listSkills(
    after: string | null,
    limit = 20,
): Promise<SkillPageResult> {
    if (!isTauri()) return { kind: "preview" };
    const position = after === null ? null : cursor(after);
    const data = object(await call("list_skills", { after, limit }), [
        "items",
        "nextCursor",
        "total",
    ]);
    if (!Array.isArray(data["items"])) invalid();
    const items = data["items"].map((value: unknown) => record(value));
    const nextCursor =
        data["nextCursor"] === null ? null : string(data["nextCursor"]);
    // The total counts the whole library, so it can never be smaller than the
    // page that was read; a wilder value is a bug, not a number to display.
    const total = integer(data["total"], 0);
    if (
        items.length > limit ||
        total < items.length ||
        total > MAX_TOTAL ||
        items.some(
            (row, index) =>
                (position !== null && !isAfter(row, position)) ||
                (index > 0 &&
                    !isAfter(row, {
                        name: items[index - 1]?.name ?? "",
                        id: items[index - 1]?.id ?? "",
                    })),
        )
    )
        invalid();
    // The next cursor must repeat the last returned row, so a page can never be
    // resumed from a row the reader has not seen.
    const last = items[items.length - 1];
    if (nextCursor !== null) {
        const position = cursor(nextCursor);
        if (
            last === undefined ||
            position.name !== last.name ||
            position.id !== last.id
        )
            invalid();
    }
    return { kind: "desktop", page: { items, nextCursor, total } };
}
/**
 * Open the system folder picker through Rust. The dialog is owned by the
 * desktop app; closing it resolves as `cancelled` rather than an error.
 */
export async function pickSkillFolder(title: string): Promise<PickedFolder> {
    desktop();
    const data = await call("pick_skill_folder", { title });
    if (!isObject(data)) invalid();
    if (data["kind"] === "cancelled") {
        if (!hasExactKeys(data, ["kind"])) invalid();
        return { kind: "cancelled" };
    }
    const selected = object(data, ["kind", "path"]);
    if (selected["kind"] !== "selected") invalid();
    const path = string(selected["path"]);
    // A path with no characters or with control characters cannot be displayed
    // or sent back; Rust validates the value again before it is used.
    if (hasControlCharacter(path) || path.trim() === "") invalid();
    return { kind: "selected", path };
}
/**
 * Import every skill inside one chosen folder: the folder itself when it is a
 * skill, otherwise each of its immediate skill subfolders. Refused folders come
 * back in the report instead of failing the whole import; the source folders are
 * only read.
 */
export async function importSkillFolder(path: string): Promise<ImportReport> {
    desktop();
    const data = object(
        await call("import_skill_folder", { request: { path } }),
        ["imported", "skipped"],
    );
    if (!Array.isArray(data["imported"]) || !Array.isArray(data["skipped"]))
        invalid();
    const imported = data["imported"].map((value: unknown) => record(value));
    const skipped = data["skipped"].map((value: unknown) =>
        skippedSkill(value),
    );
    if (
        imported.length > MAX_CANDIDATES ||
        skipped.length > MAX_CANDIDATES ||
        imported.length + skipped.length === 0 ||
        new Set(imported.map((row) => row.id)).size !== imported.length
    )
        invalid();
    // An import never records an upstream source for the user.
    if (
        imported.some(
            (row) => row.sourceUrl !== null || row.sourceRevision !== null,
        )
    )
        invalid();
    return { imported, skipped };
}
/** Read one refused folder entry: a safe name and a translatable reason. */
function skippedSkill(value: unknown): SkippedSkill {
    const data = object(value, ["folder", "code"]);
    const folder = string(data["folder"]);
    const code = string(data["code"]);
    if (
        folder.trim() === "" ||
        folder.length > 128 ||
        hasControlCharacter(folder)
    )
        invalid();
    const known = SKILL_CODES.find((candidate) => candidate === code);
    if (known === undefined) invalid();
    return { folder, code: known };
}
/** Re-read one skill before editing its source or retrying an unknown result. */
export async function getSkill(id: string): Promise<SkillRecord> {
    desktop();
    const row = record(await call("get_skill", { id }));
    if (row.id !== id) invalid();
    return row;
}
/** Record or clear the upstream source of an existing skill. */
export async function setSkillSource(input: {
    id: string;
    expectedRevision: number;
    url: string | null;
    revision: string | null;
}): Promise<SkillRecord> {
    desktop();
    const row = record(await call("set_skill_source", { request: input }));
    if (row.id !== input.id || row.revision !== input.expectedRevision + 1)
        invalid();
    return row;
}
