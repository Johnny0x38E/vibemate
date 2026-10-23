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
    "source_repository_invalid",
    "source_revision_invalid",
    "git_revision_invalid",
    "source_revision_required",
    "installer_version_invalid",
    "installer_unavailable",
    "installer_failed",
    "installer_output_invalid",
    "install_timed_out",
    "install_workspace_failed",
    "skill_not_found_in_source",
    "not_found",
    "revision_conflict",
    "invalid_stored_skill",
    "outcome_unknown",
    "operation_failed",
    "skill_in_use",
    "agent_location_unavailable",
    "target_conflict",
    "target_changed",
    "symlink_unavailable",
    "deploy_mode_invalid",
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
/** How a recorded source is fetched for an update check. */
export type SkillFetchMethod = "git" | "npx";
/** One reviewed remote acquisition path. */
export type SkillInstallSource =
    | {
          method: "git";
          source: {
              repository: string;
              revision: string;
              skill: string | null;
          };
      }
    | {
          method: "npx";
          source: {
              source: string;
              /** Null asks Rust for the installer version it reviewed. */
              installerVersion: string | null;
              skill: string | null;
          };
      };
/** What the install dialog shows before the user confirms. */
export interface SkillInstallPreview {
    method: SkillFetchMethod;
    sourceUrl: string;
    revision: string | null;
    installer: string;
    installerVersion: string | null;
    command: string;
}
/** Safe metadata for one imported skill; no file content crosses IPC. */
export interface SkillRecord {
    id: string;
    name: string;
    description: string;
    originPath: string | null;
    sourceUrl: string | null;
    sourceRevision: string | null;
    fetchMethod: SkillFetchMethod | null;
    installerVersion: string | null;
    installerCommand: string | null;
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
/** What one reviewed remote install produced. */
export interface SkillInstallReport {
    method: SkillFetchMethod;
    sourceUrl: string;
    revision: string | null;
    installer: string;
    installerVersion: string | null;
    command: string;
    exitStatus: number;
    imported: SkillRecord[];
    skipped: SkippedSkill[];
}

/** Skill names follow the library rules: letters, digits, dot, dash, underscore. */
const NAME_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/;
const DIGEST_PATTERN = /^[0-9a-f]{64}$/;
const ID_PATTERN = /^[0-9a-f]{32}$/;
const INSTALLER_VERSION_PATTERN = /^\d{1,3}\.\d{1,3}\.\d{1,3}$/;
/** The only npx installer version this build displays or accepts. */
const SKILLS_INSTALLER_VERSION = "1.7.4";
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
function fetchMethod(value: unknown): SkillFetchMethod | null {
    if (value === null) return null;
    if (value === "git" || value === "npx") return value;
    invalid();
}
/** A reviewed command is bounded display text, never a shell script. */
function command(value: string): string {
    if (value === "" || value.length > 1024 || hasControlCharacter(value))
        invalid();
    return value;
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
function boolean(value: unknown): boolean {
    if (typeof value !== "boolean") invalid();
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
        "fetchMethod",
        "installerVersion",
        "installerCommand",
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
        fetchMethod: fetchMethod(data["fetchMethod"]),
        installerVersion: optionalString(data["installerVersion"]),
        installerCommand: optionalString(data["installerCommand"]),
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
    // A fetch method and its metadata exist together and only with an address.
    if (row.fetchMethod === null) {
        if (row.installerVersion !== null || row.installerCommand !== null)
            invalid();
    } else if (row.fetchMethod === "git") {
        if (
            row.sourceUrl === null ||
            row.sourceRevision === null ||
            row.installerVersion !== null ||
            row.installerCommand === null
        )
            invalid();
    } else if (
        row.sourceUrl === null ||
        row.sourceRevision !== null ||
        row.installerVersion === null ||
        row.installerCommand === null
    ) {
        invalid();
    }
    if (
        row.installerVersion !== null &&
        !INSTALLER_VERSION_PATTERN.test(row.installerVersion)
    )
        invalid();
    if (row.installerCommand !== null) command(row.installerCommand);
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
    // An import never records an upstream source or fetch method for the user.
    if (
        imported.some(
            (row) =>
                row.sourceUrl !== null ||
                row.sourceRevision !== null ||
                row.fetchMethod !== null,
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
    fetchMethod: SkillFetchMethod | null;
    installerVersion: string | null;
}): Promise<SkillRecord> {
    desktop();
    const row = record(await call("set_skill_source", { request: input }));
    if (row.id !== input.id || row.revision !== input.expectedRevision + 1)
        invalid();
    return row;
}
/** Validate the command shown before any installer runs. */
function installPreview(value: unknown): SkillInstallPreview {
    const data = object(value, [
        "method",
        "sourceUrl",
        "revision",
        "installer",
        "installerVersion",
        "command",
    ]);
    const method = fetchMethod(data["method"]);
    if (method === null) invalid();
    const sourceUrl = string(data["sourceUrl"]);
    const revision = optionalString(data["revision"]);
    const installer = string(data["installer"]);
    const installerVersion = optionalString(data["installerVersion"]);
    const reviewed = command(string(data["command"]));
    if (
        sourceUrl === "" ||
        sourceUrl.length > 512 ||
        hasControlCharacter(sourceUrl) ||
        installer === "" ||
        installer.length > 64 ||
        hasControlCharacter(installer)
    )
        invalid();
    if (method === "git") {
        if (
            installer !== "git" ||
            revision === null ||
            installerVersion !== null ||
            !reviewed.startsWith("git fetch --depth 1 ")
        )
            invalid();
    } else if (
        installer !== "skills" ||
        revision !== null ||
        installerVersion !== SKILLS_INSTALLER_VERSION ||
        !INSTALLER_VERSION_PATTERN.test(installerVersion) ||
        !reviewed.startsWith("npx --yes skills@")
    ) {
        invalid();
    }
    return {
        method,
        sourceUrl,
        revision,
        installer,
        installerVersion,
        command: reviewed,
    };
}
/**
 * Review a source without network access or a process. Browser mode returns an
 * honest preview marker; the dialog never invents a command.
 */
export async function previewSkillInstall(
    source: SkillInstallSource,
): Promise<
    { kind: "preview" } | { kind: "desktop"; preview: SkillInstallPreview }
> {
    if (!isTauri()) return { kind: "preview" };
    const preview = installPreview(
        await call("preview_skill_install", { source }),
    );
    // A different acquisition path is a contract break, not a new choice.
    if (preview.method !== source.method) invalid();
    return { kind: "desktop", preview };
}
/** Run the reviewed installer and import every skill it acquired. */
export async function installSkill(
    source: SkillInstallSource,
): Promise<SkillInstallReport> {
    desktop();
    const data = object(await call("install_skill", { source }), [
        "method",
        "sourceUrl",
        "revision",
        "installer",
        "installerVersion",
        "command",
        "exitStatus",
        "imported",
        "skipped",
    ]);
    const method = fetchMethod(data["method"]);
    if (method === null) invalid();
    const sourceUrl = string(data["sourceUrl"]);
    const revision = optionalString(data["revision"]);
    const installer = string(data["installer"]);
    const installerVersion = optionalString(data["installerVersion"]);
    const reviewed = command(string(data["command"]));
    const exitStatus = integer(data["exitStatus"], 0);
    if (method === "git") {
        if (
            installer !== "git" ||
            installerVersion !== null ||
            revision === null
        )
            invalid();
    } else if (
        installer !== "skills" ||
        installerVersion !== SKILLS_INSTALLER_VERSION ||
        revision !== null
    ) {
        invalid();
    }
    if (!Array.isArray(data["imported"]) || !Array.isArray(data["skipped"]))
        invalid();
    const imported = data["imported"].map((value: unknown) => record(value));
    const skipped = data["skipped"].map((value: unknown) =>
        skippedSkill(value),
    );
    if (
        method !== source.method ||
        exitStatus !== 0 ||
        imported.length > MAX_CANDIDATES ||
        skipped.length > MAX_CANDIDATES ||
        imported.length + skipped.length === 0 ||
        new Set(imported.map((row) => row.id)).size !== imported.length ||
        imported.some(
            (row) =>
                row.originPath !== null ||
                row.sourceUrl !== sourceUrl ||
                row.fetchMethod !== method ||
                row.installerVersion !== installerVersion ||
                row.installerCommand !== reviewed ||
                (method === "git"
                    ? row.sourceRevision !== revision
                    : row.sourceRevision !== null),
        )
    )
        invalid();
    return {
        method,
        sourceUrl,
        revision,
        installer,
        installerVersion,
        command: reviewed,
        exitStatus,
        imported,
        skipped,
    };
}
/**
 * Remove one reviewed skill record and vibemate's managed copy of it. The
 * folder the skill was imported from is never part of this request. A stale
 * confirmation is refused, and copied files that could not be removed come
 * back as `cleanupPending` instead of being reported as gone.
 */
export async function deleteSkill(request: {
    id: string;
    expectedRevision: number;
}): Promise<{ id: string; cleanupPending: boolean }> {
    desktop();
    const data = object(await call("delete_skill", { request }), [
        "id",
        "cleanupPending",
    ]);
    const id = string(data["id"]);
    if (id !== request.id || !ID_PATTERN.test(id)) invalid();
    return { id, cleanupPending: boolean(data["cleanupPending"]) };
}
/** Retry removing parked copies left by finished deletions; false means more. */
export async function cleanupDeletedSkillCopies(): Promise<boolean> {
    desktop();
    return boolean(await call("cleanup_deleted_skill_copies", {}));
}

/** How a skill is linked into an Agent skills directory. */
export type SkillDeployMode = "symlink" | "copy";
/** Filesystem classification of the deploy target at preview time. */
export type SkillDeployTargetState =
    "absent" | "owned_symlink" | "owned_copy" | "foreign";
/** Preview of deploying one skill into Pi's user skills directory. */
export interface PiSkillDeployPreview {
    skillId: string;
    skillName: string;
    skillRevision: number;
    contentDigest: string;
    configRoot: string;
    targetPath: string;
    targetName: string;
    targetState: SkillDeployTargetState;
    symlinkAvailable: boolean;
    suggestedMode: SkillDeployMode;
    canApply: boolean;
}
/** Result of applying a reviewed Pi skill deployment. */
export interface PiSkillDeployed {
    skillId: string;
    skillName: string;
    skillRevision: number;
    mode: SkillDeployMode;
    targetPath: string;
}
/** Outcome of disabling a Pi skill deployment. */
export interface PiSkillDeployDisabled {
    skillId: string;
    targetPath: string;
    targetRemoved: boolean;
    leftForeign: boolean;
}
/** One recorded skill deployment for an Agent configuration root. */
export interface SkillDeploymentRecord {
    skillId: string;
    skillName: string;
    agentId: string;
    configRoot: string;
    mode: SkillDeployMode;
    skillRevision: number;
    contentDigest: string;
    inSync: boolean;
    deployedAtMs: number;
}

const DEPLOY_MODES = ["symlink", "copy"] as const;
const TARGET_STATES = [
    "absent",
    "owned_symlink",
    "owned_copy",
    "foreign",
] as const;

function deployMode(value: unknown): SkillDeployMode {
    const text = string(value);
    const known = DEPLOY_MODES.find((mode) => mode === text);
    if (known === undefined) invalid();
    return known;
}

function targetState(value: unknown): SkillDeployTargetState {
    const text = string(value);
    const known = TARGET_STATES.find((state) => state === text);
    if (known === undefined) invalid();
    return known;
}

function deployPreview(value: unknown): PiSkillDeployPreview {
    const data = object(value, [
        "skillId",
        "skillName",
        "skillRevision",
        "contentDigest",
        "configRoot",
        "targetPath",
        "targetName",
        "targetState",
        "symlinkAvailable",
        "suggestedMode",
        "canApply",
    ]);
    const skillId = string(data["skillId"]);
    const skillName = string(data["skillName"]);
    const contentDigest = string(data["contentDigest"]);
    const configRoot = string(data["configRoot"]);
    const targetPath = string(data["targetPath"]);
    const targetName = string(data["targetName"]);
    if (
        !ID_PATTERN.test(skillId) ||
        !NAME_PATTERN.test(skillName) ||
        !DIGEST_PATTERN.test(contentDigest) ||
        targetName !== skillName ||
        hasControlCharacter(configRoot) ||
        hasControlCharacter(targetPath) ||
        configRoot.trim() === "" ||
        targetPath.trim() === ""
    )
        invalid();
    return {
        skillId,
        skillName,
        skillRevision: integer(data["skillRevision"], 1),
        contentDigest,
        configRoot,
        targetPath,
        targetName,
        targetState: targetState(data["targetState"]),
        symlinkAvailable: boolean(data["symlinkAvailable"]),
        suggestedMode: deployMode(data["suggestedMode"]),
        canApply: boolean(data["canApply"]),
    };
}

function deploymentRecord(value: unknown): SkillDeploymentRecord {
    const data = object(value, [
        "skillId",
        "skillName",
        "agentId",
        "configRoot",
        "mode",
        "skillRevision",
        "contentDigest",
        "inSync",
        "deployedAtMs",
    ]);
    const skillId = string(data["skillId"]);
    const skillName = string(data["skillName"]);
    const agentId = string(data["agentId"]);
    const configRoot = string(data["configRoot"]);
    const contentDigest = string(data["contentDigest"]);
    if (
        !ID_PATTERN.test(skillId) ||
        !NAME_PATTERN.test(skillName) ||
        agentId !== "pi" ||
        !DIGEST_PATTERN.test(contentDigest) ||
        hasControlCharacter(configRoot) ||
        configRoot.trim() === ""
    )
        invalid();
    return {
        skillId,
        skillName,
        agentId,
        configRoot,
        mode: deployMode(data["mode"]),
        skillRevision: integer(data["skillRevision"], 1),
        contentDigest,
        inSync: boolean(data["inSync"]),
        deployedAtMs: integer(data["deployedAtMs"], 0),
    };
}

/**
 * Preview deploying one library skill into Pi. Browser mode reports that the
 * desktop app is required; it never invents a successful path.
 */
export async function previewPiSkillDeploy(
    skillId: string,
): Promise<
    { kind: "preview" } | { kind: "desktop"; preview: PiSkillDeployPreview }
> {
    if (!isTauri()) return { kind: "preview" };
    const preview = deployPreview(
        await call("preview_pi_skill_deploy", { skillId }),
    );
    if (preview.skillId !== skillId) invalid();
    return { kind: "desktop", preview };
}

/** Apply a reviewed Pi skill deployment. */
export async function applyPiSkillDeploy(input: {
    skillId: string;
    expectedSkillRevision: number;
    configRoot: string;
    mode: SkillDeployMode;
    targetState: SkillDeployTargetState;
}): Promise<PiSkillDeployed> {
    desktop();
    const data = object(
        await call("apply_pi_skill_deploy", { reviewed: input }),
        ["skillId", "skillName", "skillRevision", "mode", "targetPath"],
    );
    const skillId = string(data["skillId"]);
    const skillName = string(data["skillName"]);
    const targetPath = string(data["targetPath"]);
    if (
        skillId !== input.skillId ||
        !NAME_PATTERN.test(skillName) ||
        hasControlCharacter(targetPath) ||
        targetPath.trim() === ""
    )
        invalid();
    return {
        skillId,
        skillName,
        skillRevision: integer(data["skillRevision"], 1),
        mode: deployMode(data["mode"]),
        targetPath,
    };
}

/** Revoke one vibemate-managed Pi skill deployment. */
export async function disablePiSkillDeploy(input: {
    skillId: string;
    configRoot: string;
}): Promise<PiSkillDeployDisabled> {
    desktop();
    const data = object(
        await call("disable_pi_skill_deploy", { request: input }),
        ["skillId", "targetPath", "targetRemoved", "leftForeign"],
    );
    const skillId = string(data["skillId"]);
    const targetPath = string(data["targetPath"]);
    if (
        skillId !== input.skillId ||
        hasControlCharacter(targetPath) ||
        targetPath.trim() === ""
    )
        invalid();
    return {
        skillId,
        targetPath,
        targetRemoved: boolean(data["targetRemoved"]),
        leftForeign: boolean(data["leftForeign"]),
    };
}

/** List deployments for the resolved Pi agent directory. */
export async function listPiSkillDeployments(): Promise<
    { kind: "preview" } | { kind: "desktop"; items: SkillDeploymentRecord[] }
> {
    if (!isTauri()) return { kind: "preview" };
    const data = await call("list_pi_skill_deployments", {});
    if (!Array.isArray(data)) invalid();
    return {
        kind: "desktop",
        items: data.map((value: unknown) => deploymentRecord(value)),
    };
}

/** List deployments recorded for one central skill. */
export async function listSkillDeployments(
    skillId: string,
): Promise<
    { kind: "preview" } | { kind: "desktop"; items: SkillDeploymentRecord[] }
> {
    if (!isTauri()) return { kind: "preview" };
    const data = await call("list_skill_deployments", { skillId });
    if (!Array.isArray(data)) invalid();
    const items = data.map((value: unknown) => deploymentRecord(value));
    if (items.some((row) => row.skillId !== skillId)) invalid();
    return { kind: "desktop", items };
}
