import {
    useCallback,
    useEffect,
    useImperativeHandle,
    useRef,
    useState,
    type JSX,
    type Ref,
} from "react";
import type { ModuleNavigationHandle } from "../../components/moduleNavigation";
import { useTranslation } from "react-i18next";
import { Icon } from "../../components/Icon";
import { PageModuleHeader } from "../../components/PageModuleHeader";
import { useNotify } from "../../components/Notifications";
import {
    getSkill,
    importSkillFolder,
    listSkills,
    pickSkillFolder,
    SkillRequestError,
    type ImportReport,
    type SkillErrorCode,
    type SkillRecord,
} from "../../lib/desktop/skills";
import { SkillDetail } from "./SkillDetail";
import { skillSource } from "./skillSource";
import buttons from "../../components/actionButtons.module.css";
import surfaceList from "../../components/surfaceList.module.css";
import styles from "./SkillLibrary.module.css";

type ListState =
    | { kind: "loading" | "preview" }
    | { kind: "error"; code: SkillErrorCode }
    | {
          kind: "ready";
          items: SkillRecord[];
          nextCursor: string | null;
          /** Library total, not the number of loaded rows. */
          total: number;
          more: "idle" | "loading";
      };
type PageState = { kind: "list" } | { kind: "detail"; record: SkillRecord };
type ActionState =
    | { kind: "idle" | "picking" | "importing" }
    | { kind: "error"; code: SkillErrorCode; uncertain: boolean };

/**
 * The source line of one list row: the recorded address with its revision, the
 * local import, or an honest unknown value.
 */
function rowSourceText(
    record: SkillRecord,
    t: (key: string, options?: Record<string, unknown>) => string,
): string {
    const source = skillSource(record);
    if (source.kind !== "address") {
        return t(
            source.kind === "local"
                ? "skills.sourceLocalRow"
                : "skills.sourceUnknownValue",
        );
    }
    const address = t("skills.sourceAddress", { url: source.url });
    return source.revision === null
        ? address
        : `${address} · ${t("skills.sourceRevision", { revision: source.revision })}`;
}

/** The sidebar page stays mounted while hidden, so drafts and pages survive. */
export interface SkillLibraryProps {
    hidden?: boolean;
    homeRef?: Ref<ModuleNavigationHandle>;
}

function errorCode(error: unknown): SkillErrorCode {
    return error instanceof SkillRequestError ? error.code : "operation_failed";
}

/** Failures that may have changed something beyond this window's knowledge. */
function uncertain(code: SkillErrorCode): boolean {
    return [
        "operation_failed",
        "invalid_response",
        "outcome_unknown",
        "write_failed",
        "storage_unavailable",
        "revision_conflict",
    ].includes(code);
}

/**
 * Central skill library: import local skill folders, then list what the library
 * holds. The list stays compact; one skill's files, size, digest and source live
 * on its detail page. Importing copies files into the private library, leaves the
 * chosen folders untouched, and never executes a skill command.
 */
export function SkillLibrary({
    hidden,
    homeRef,
}: SkillLibraryProps): JSX.Element {
    const { t } = useTranslation();
    const notify = useNotify();
    const [list, setList] = useState<ListState>({ kind: "loading" });
    const [page, setPage] = useState<PageState>({ kind: "list" });
    const [action, setAction] = useState<ActionState>({ kind: "idle" });
    const [report, setReport] = useState<ImportReport | null>(null);
    const [formBlocked, setFormBlocked] = useState(false);
    const active = useRef(true);
    const readGeneration = useRef(0);
    const pageGeneration = useRef(0);
    const mutationPending = useRef(false);
    const root = useRef<HTMLElement>(null);
    const focusRequested = useRef(false);
    const busy = action.kind === "picking" || action.kind === "importing";
    const blocked =
        busy ||
        (action.kind === "error" && action.uncertain) ||
        (list.kind === "ready" && list.more === "loading");

    useImperativeHandle(homeRef, () => ({
        openHome() {
            if (blocked || formBlocked || mutationPending.current) return false;
            if (page.kind !== "list") returnToList();
            return true;
        },
    }));
    useEffect(() => {
        // Async replies after unmount must not touch state.
        active.current = true;
        return () => {
            active.current = false;
        };
    }, []);
    // Read through a function so TypeScript cannot narrow the ref value: the
    // picker and the import are two separate awaits in one handler.
    const stillMounted = useCallback((): boolean => active.current, []);
    useEffect(() => {
        // The heading is focused after an async recovery, once it can be seen.
        if (!hidden && focusRequested.current) {
            root.current
                ?.querySelector<HTMLElement>('[data-focus="heading"]')
                ?.focus();
            focusRequested.current = false;
        }
    });
    const reload = useCallback(async (): Promise<void> => {
        const generation = ++readGeneration.current;
        setList({ kind: "loading" });
        setAction({ kind: "idle" });
        try {
            const result = await listSkills(null);
            if (!active.current || generation !== readGeneration.current)
                return;
            setList(
                result.kind === "preview"
                    ? { kind: "preview" }
                    : { kind: "ready", ...result.page, more: "idle" },
            );
        } catch (error: unknown) {
            if (active.current && generation === readGeneration.current)
                setList({ kind: "error", code: errorCode(error) });
        }
    }, []);
    useEffect(() => {
        // One initial read, including StrictMode cleanup protection.
        const timer = window.setTimeout(() => {
            void reload();
        }, 0);
        return () => {
            clearTimeout(timer);
        };
    }, [reload]);
    function returnToList(): void {
        pageGeneration.current++;
        focusRequested.current = true;
        setPage({ kind: "list" });
    }
    async function more(): Promise<void> {
        if (
            list.kind !== "ready" ||
            list.nextCursor === null ||
            list.more === "loading" ||
            blocked
        )
            return;
        const generation = readGeneration.current;
        const snapshot = list;
        setList({ ...list, more: "loading" });
        try {
            const result = await listSkills(snapshot.nextCursor);
            if (!active.current || generation !== readGeneration.current)
                return;
            if (result.kind === "preview") {
                setList({ kind: "preview" });
                return;
            }
            const seen = new Set(snapshot.items.map((row) => row.id));
            setList({
                kind: "ready",
                items: [
                    ...snapshot.items,
                    ...result.page.items.filter((row) => !seen.has(row.id)),
                ],
                nextCursor: result.page.nextCursor,
                // Later pages repeat the same library total.
                total: result.page.total,
                more: "idle",
            });
            // The last page removes Load more; keep focus on the heading instead.
            if (
                result.page.nextCursor === null &&
                document.activeElement?.getAttribute("data-more") === "true"
            )
                focusRequested.current = true;
        } catch (error: unknown) {
            if (active.current && generation === readGeneration.current) {
                setList({ ...snapshot, more: "idle" });
                setAction({
                    kind: "error",
                    code: errorCode(error),
                    uncertain: false,
                });
            }
        }
    }
    /**
     * Ask the desktop app for a folder, then import what it holds: one skill or
     * every skill folder inside it. Closing the picker changes nothing, and a
     * refused folder is reported instead of failing the whole import.
     */
    async function chooseFolder(): Promise<void> {
        if (blocked || mutationPending.current) return;
        mutationPending.current = true;
        setReport(null);
        setAction({ kind: "picking" });
        try {
            const picked = await pickSkillFolder(t("skills.pickTitle"));
            if (!stillMounted()) return;
            if (picked.kind === "cancelled") {
                setAction({ kind: "idle" });
                return;
            }
            setAction({ kind: "importing" });
            const result = await importSkillFolder(picked.path);
            if (!stillMounted()) return;
            setReport(result);
            setAction({ kind: "idle" });
            // A clean import is announced once; a partial one is explained by the
            // report panel, which already states the counts.
            if (result.imported.length > 0 && result.skipped.length === 0) {
                notify({
                    tone: "success",
                    message: t("skills.reportImported", {
                        count: result.imported.length,
                    }),
                });
            }
            // Imported rows can belong on a later page, so the first page is re-read.
            await reload();
        } catch (error: unknown) {
            if (active.current) {
                const code = errorCode(error);
                setAction({ kind: "error", code, uncertain: uncertain(code) });
            }
        } finally {
            mutationPending.current = false;
        }
    }
    async function openDetail(record: SkillRecord): Promise<void> {
        if (blocked || mutationPending.current) return;
        const generation = ++pageGeneration.current;
        setPage({ kind: "detail", record });
        try {
            const current = await getSkill(record.id);
            // Re-read the revision so an edited source cannot overwrite a newer one.
            if (active.current && generation === pageGeneration.current)
                setPage({ kind: "detail", record: current });
        } catch (error: unknown) {
            if (active.current && generation === pageGeneration.current) {
                setPage({ kind: "list" });
                setList({ kind: "error", code: errorCode(error) });
                setAction({ kind: "idle" });
            }
        }
    }
    function sourceSaved(record: SkillRecord): void {
        // The saved record is authoritative, so the detail page and the list row
        // both show it without another read.
        setPage({ kind: "detail", record });
        notify({ tone: "success", message: t("skills.sourceSaved") });
        setList((state) =>
            state.kind === "ready"
                ? {
                      ...state,
                      items: state.items.map((row) =>
                          row.id === record.id ? record : row,
                      ),
                  }
                : state,
        );
    }
    return (
        <section
            ref={root}
            className={styles["view"]}
            hidden={hidden}
            data-secondary-page={page.kind === "detail" ? true : undefined}
        >
            {page.kind === "detail" ? (
                <SkillDetail
                    key={`${page.record.id}-${String(page.record.revision)}`}
                    record={page.record}
                    onSaved={sourceSaved}
                    onCancel={returnToList}
                    onReload={() => {
                        void openDetail(page.record);
                    }}
                    onNavigationBlockedChange={setFormBlocked}
                />
            ) : (
                <>
                    <PageModuleHeader
                        icon="skills"
                        title={t("skills.title")}
                        tabIndex={-1}
                        dataFocus="heading"
                        trailing={
                            // Importing adds an existing skill folder, so the
                            // folder icon and its place in the header match the
                            // other module pages.
                            <button
                                className={[
                                    buttons["primary"],
                                    buttons["withIcon"],
                                ].join(" ")}
                                type="button"
                                aria-disabled={blocked}
                                onClick={() => {
                                    void chooseFolder();
                                }}
                            >
                                <Icon name="folder" />
                                {t("skills.localImport")}
                            </button>
                        }
                    />
                    {/* Import feedback sits under the header, next to the list. */}
                    {(busy ||
                        action.kind === "error" ||
                        (report !== null && report.skipped.length > 0)) && (
                        <div className={styles["importFeedback"]}>
                            {busy && (
                                <p className={styles["hint"]} role="status">
                                    {t(
                                        action.kind === "picking"
                                            ? "skills.picking"
                                            : "skills.importing",
                                    )}
                                </p>
                            )}
                            {report !== null && report.skipped.length > 0 && (
                                <div className={styles["report"]}>
                                    <p
                                        className={
                                            report.imported.length === 0
                                                ? styles["error"]
                                                : styles["hint"]
                                        }
                                        role="status"
                                    >
                                        {report.imported.length === 0
                                            ? t("skills.reportNone")
                                            : t("skills.reportImported", {
                                                  count: report.imported.length,
                                              })}
                                    </p>
                                    <p className={styles["hint"]}>
                                        {t("skills.reportSkipped", {
                                            count: report.skipped.length,
                                        })}
                                    </p>
                                    <ul className={styles["reportList"]}>
                                        {report.skipped.map((row) => (
                                            <li key={row.folder}>
                                                {t("skills.reportItem", {
                                                    folder: row.folder,
                                                    reason: t(
                                                        `skills.errors.${row.code}`,
                                                    ),
                                                })}
                                            </li>
                                        ))}
                                    </ul>
                                </div>
                            )}
                            {action.kind === "error" && (
                                <>
                                    <p className={styles["error"]} role="alert">
                                        {t(`skills.errors.${action.code}`)}
                                    </p>
                                    {action.uncertain && (
                                        <button
                                            className={buttons["secondary"]}
                                            type="button"
                                            onClick={() => {
                                                void reload();
                                            }}
                                        >
                                            {t("skills.reload")}
                                        </button>
                                    )}
                                </>
                            )}
                        </div>
                    )}
                    {list.kind === "loading" && (
                        <p role="status">{t("skills.loading")}</p>
                    )}
                    {list.kind === "preview" && (
                        <p role="status">{t("skills.preview")}</p>
                    )}
                    {list.kind === "error" && (
                        <>
                            <p className={styles["error"]} role="alert">
                                {t(`skills.errors.${list.code}`)}
                            </p>
                            <button
                                className={buttons["secondary"]}
                                type="button"
                                onClick={() => {
                                    void reload();
                                }}
                            >
                                {t("skills.retry")}
                            </button>
                        </>
                    )}
                    {list.kind === "ready" && (
                        <>
                            {list.total > 0 && (
                                <p className={styles["count"]}>
                                    {t("skills.count", { count: list.total })}
                                </p>
                            )}
                            {list.items.length === 0 ? (
                                <p role="status">{t("skills.empty")}</p>
                            ) : (
                                <ul
                                    className={[
                                        surfaceList["list"],
                                        styles["list"],
                                    ].join(" ")}
                                    aria-label={t("skills.list")}
                                >
                                    {list.items.map((record) => (
                                        <li
                                            className={surfaceList["item"]}
                                            key={record.id}
                                        >
                                            <div className={styles["details"]}>
                                                <h2 className={styles["name"]}>
                                                    {record.name}
                                                </h2>
                                                {/* Long text is clamped here; the
                                                detail page shows it in full. */}
                                                <p
                                                    className={
                                                        styles["description"]
                                                    }
                                                >
                                                    {record.description}
                                                </p>
                                                <p className={styles["meta"]}>
                                                    {rowSourceText(record, t)}
                                                </p>
                                            </div>
                                            <div className={styles["trailing"]}>
                                                <button
                                                    className={[
                                                        buttons["text"],
                                                        buttons["withIcon"],
                                                    ].join(" ")}
                                                    type="button"
                                                    aria-disabled={blocked}
                                                    aria-label={t(
                                                        "skills.detailsLabel",
                                                        { name: record.name },
                                                    )}
                                                    onClick={() => {
                                                        void openDetail(record);
                                                    }}
                                                >
                                                    <Icon name="edit" />
                                                    {t("skills.details")}
                                                </button>
                                            </div>
                                        </li>
                                    ))}
                                </ul>
                            )}
                            <p className={styles["hint"]}>
                                {t("skills.notDeployed")}
                            </p>
                            {list.nextCursor !== null && (
                                <button
                                    className={buttons["secondary"]}
                                    type="button"
                                    data-more="true"
                                    aria-disabled={blocked}
                                    onClick={() => {
                                        void more();
                                    }}
                                >
                                    {list.more === "loading"
                                        ? t("skills.loadingMore")
                                        : t("skills.loadMore")}
                                </button>
                            )}
                        </>
                    )}
                </>
            )}
        </section>
    );
}
