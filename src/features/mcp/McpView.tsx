import {
    useCallback,
    useEffect,
    useImperativeHandle,
    useRef,
    useState,
    type Ref,
    type JSX,
} from "react";
import type { ModuleNavigationHandle } from "../../components/moduleNavigation";
import { useTranslation } from "react-i18next";
import { Icon } from "../../components/Icon";
import { PageModuleHeader } from "../../components/PageModuleHeader";
import { useNotify } from "../../components/Notifications";
import {
    cleanupMcpCredentials,
    getMcpDefinition,
    listMcpDefinitions,
    McpRequestError,
    saveMcpDefinition,
    type McpErrorCode,
    type McpRecord,
    type SaveMcpInput,
} from "../../lib/desktop/mcp";
import { McpForm } from "./McpForm";
import buttons from "../../components/actionButtons.module.css";
import chip from "../../components/infoChip.module.css";
import surfaceList from "../../components/surfaceList.module.css";
import styles from "./Mcp.module.css";

type ListState =
    | { kind: "loading" | "preview" }
    | { kind: "error"; code: McpErrorCode }
    | {
          kind: "ready";
          items: McpRecord[];
          nextCursor: string | null;
          more: "idle" | "loading";
      };
type PageState =
    | { kind: "list" }
    | { kind: "opening"; id: string }
    | { kind: "form"; record: McpRecord | null };
type ActionState =
    | { kind: "idle" | "running" }
    | { kind: "error"; code: McpErrorCode; uncertain: boolean };
/** The sidebar page stays mounted while hidden; password fields clear on hide. */
export interface McpViewProps {
    hidden?: boolean;
    homeRef?: Ref<ModuleNavigationHandle>;
}
function errorCode(error: unknown): McpErrorCode {
    return error instanceof McpRequestError ? error.code : "operation_failed";
}
function uncertain(code: McpErrorCode): boolean {
    return [
        "operation_failed",
        "invalid_response",
        "outcome_unknown",
        "revision_conflict",
    ].includes(code);
}
function preservedRequest(record: McpRecord): SaveMcpInput {
    const connection = record.connection;
    return {
        id: record.id,
        expectedRevision: record.revision,
        displayName: record.displayName,
        serverName: record.serverName,
        enabled: !record.enabled,
        connection:
            connection.type === "stdio"
                ? {
                      ...connection,
                      env: connection.env.map((name) => ({
                          name,
                          value: null,
                      })),
                  }
                : {
                      ...connection,
                      headers: connection.headers.map((name) => ({
                          name,
                          value: null,
                      })),
                  },
    };
}
/** Central list/edit/enablement management; it exposes no deployment or execution action. */
export function McpView({
    hidden = false,
    homeRef,
}: McpViewProps): JSX.Element {
    const { t } = useTranslation();
    const notify = useNotify();
    const [list, setList] = useState<ListState>({ kind: "loading" });
    const [page, setPage] = useState<PageState>({ kind: "list" });
    const [action, setAction] = useState<ActionState>({ kind: "idle" });
    const [formBlocked, setFormBlocked] = useState(false);
    const active = useRef(true);
    const readGeneration = useRef(0);
    const pageGeneration = useRef(0);
    const mutationPending = useRef(false);
    const root = useRef<HTMLElement>(null);
    const focusRequested = useRef(false);
    const blocked =
        action.kind === "running" ||
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
        active.current = true;
        return () => {
            active.current = false;
        };
    }, []);
    useEffect(() => {
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
            const result = await listMcpDefinitions(null);
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
        // One initial read, including StrictMode cleanup protection. Revealing a
        // hidden form keeps its non-secret input instead of replacing it.
        const timer = window.setTimeout(() => {
            void reload();
        }, 0);
        return () => {
            clearTimeout(timer);
        };
    }, [reload]);
    async function open(id: string): Promise<void> {
        if (blocked || mutationPending.current) return;
        const generation = ++pageGeneration.current;
        setPage({ kind: "opening", id });
        try {
            const record = await getMcpDefinition(id);
            if (active.current && generation === pageGeneration.current) {
                setPage({ kind: "form", record });
                focusRequested.current = true;
            }
        } catch (error: unknown) {
            if (active.current && generation === pageGeneration.current) {
                setPage({ kind: "list" });
                // A failed reconciliation must not expose stale editable rows.
                setList({ kind: "error", code: errorCode(error) });
                setAction({ kind: "idle" });
            }
        }
    }
    function returnToList(): void {
        pageGeneration.current++;
        focusRequested.current = true;
        setPage({ kind: "list" });
        void reload();
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
            const result = await listMcpDefinitions(snapshot.nextCursor);
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
                more: "idle",
            });
            // The last page removes Load more; focus the heading if its button was focused.
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
    async function mutate(record: McpRecord, cleanup: boolean): Promise<void> {
        if (blocked || mutationPending.current) return;
        mutationPending.current = true;
        setAction({ kind: "running" });
        try {
            const saved = cleanup
                ? await cleanupMcpCredentials(record.id)
                : await saveMcpDefinition(preservedRequest(record));
            if (!active.current) return;
            setList((state) =>
                state.kind === "ready"
                    ? {
                          ...state,
                          items: state.items.map((row) =>
                              row.id === saved.id ? saved : row,
                          ),
                      }
                    : state,
            );
            setAction({ kind: "idle" });
            notify({
                tone: "success",
                message: t(
                    saved.cleanupPending ? "mcp.cleanupPending" : "mcp.saved",
                ),
            });
        } catch (error: unknown) {
            if (active.current) {
                const code = errorCode(error);
                setAction({ kind: "error", code, uncertain: uncertain(code) });
            }
        } finally {
            mutationPending.current = false;
        }
    }
    function saved(record: McpRecord): void {
        setPage({ kind: "list" });
        focusRequested.current = true;
        notify({
            tone: "success",
            message: t(
                record.cleanupPending ? "mcp.cleanupPending" : "mcp.saved",
            ),
        });
        void reload();
    }
    return (
        <section
            ref={root}
            className={styles["view"]}
            hidden={hidden}
            data-mcp-editor={page.kind === "form" ? true : undefined}
        >
            {page.kind !== "form" && (
                <PageModuleHeader
                    icon="mcp"
                    title={t("mcp.title")}
                    tabIndex={-1}
                    dataFocus="heading"
                    trailing={
                        page.kind === "list" ? (
                            <button
                                type="button"
                                className={[
                                    buttons["primary"],
                                    buttons["withIcon"],
                                ]
                                    .filter(
                                        (value): value is string =>
                                            value !== undefined,
                                    )
                                    .join(" ")}
                                aria-disabled={blocked || list.kind !== "ready"}
                                onClick={() => {
                                    if (!blocked && list.kind === "ready") {
                                        setPage({ kind: "form", record: null });
                                        focusRequested.current = true;
                                    }
                                }}
                            >
                                <Icon name="plus" />
                                {t("mcp.new")}
                            </button>
                        ) : undefined
                    }
                />
            )}
            {page.kind === "form" ? (
                <McpForm
                    key={`${page.record?.id ?? "new"}-${String(page.record?.revision ?? 0)}`}
                    record={page.record}
                    hidden={hidden}
                    onSaved={saved}
                    onNavigationBlockedChange={setFormBlocked}
                    onCancel={returnToList}
                    onReload={() => {
                        const id = page.record?.id;
                        if (id === undefined) returnToList();
                        else void open(id);
                    }}
                />
            ) : page.kind === "opening" ? (
                <p role="status">{t("mcp.loading")}</p>
            ) : (
                <>
                    {list.kind === "loading" && (
                        <p role="status">{t("mcp.loading")}</p>
                    )}
                    {list.kind === "preview" && (
                        <p role="status">{t("mcp.preview")}</p>
                    )}
                    {list.kind === "error" && (
                        <>
                            <p className={styles["error"]} role="alert">
                                {t(`mcp.errors.${list.code}`)}
                            </p>
                            <button
                                className={buttons["secondary"]}
                                type="button"
                                onClick={() => {
                                    void reload();
                                }}
                            >
                                {t("mcp.retry")}
                            </button>
                        </>
                    )}
                    {action.kind === "error" && (
                        <>
                            <p className={styles["error"]} role="alert">
                                {t(`mcp.errors.${action.code}`)}
                            </p>
                            {action.uncertain && (
                                <>
                                    <p className={styles["hint"]}>
                                        {t("mcp.reloadUnknown")}
                                    </p>
                                    <button
                                        className={buttons["secondary"]}
                                        type="button"
                                        onClick={() => {
                                            void reload();
                                        }}
                                    >
                                        {t("mcp.reload")}
                                    </button>
                                </>
                            )}
                        </>
                    )}
                    {action.kind === "running" && (
                        <p role="status">{t("mcp.saving")}</p>
                    )}
                    {list.kind === "ready" && (
                        <>
                            {list.items.length === 0 ? (
                                <p role="status">{t("mcp.empty")}</p>
                            ) : (
                                <ul
                                    className={[
                                        surfaceList["list"],
                                        styles["list"],
                                    ].join(" ")}
                                    aria-label={t("mcp.title")}
                                >
                                    {list.items.map((record) => (
                                        <li
                                            className={surfaceList["item"]}
                                            key={record.id}
                                        >
                                            <div className={styles["details"]}>
                                                <h2 className={styles["name"]}>
                                                    {record.displayName}
                                                </h2>
                                                <p
                                                    className={
                                                        styles["subline"]
                                                    }
                                                >
                                                    {t(
                                                        "mcp.list.registeredAgents",
                                                    )}
                                                </p>
                                                {record.cleanupPending && (
                                                    <p
                                                        className={
                                                            styles["hint"]
                                                        }
                                                    >
                                                        {t(
                                                            "mcp.cleanupPending",
                                                        )}
                                                    </p>
                                                )}
                                            </div>
                                            <div className={styles["trailing"]}>
                                                <span className={chip["chip"]}>
                                                    {t(
                                                        record.connection
                                                            .type === "stdio"
                                                            ? "mcp.stdio"
                                                            : "mcp.http",
                                                    )}
                                                </span>
                                                <div
                                                    className={
                                                        styles["actions"]
                                                    }
                                                >
                                                    <button
                                                        className={[
                                                            buttons["text"],
                                                            buttons["withIcon"],
                                                        ].join(" ")}
                                                        type="button"
                                                        aria-disabled={blocked}
                                                        aria-label={t(
                                                            "mcp.editLabel",
                                                            {
                                                                name: record.displayName,
                                                            },
                                                        )}
                                                        onClick={() => {
                                                            void open(
                                                                record.id,
                                                            );
                                                        }}
                                                    >
                                                        <Icon name="edit" />
                                                        {t("mcp.list.edit")}
                                                    </button>
                                                    <button
                                                        className={[
                                                            buttons["text"],
                                                            buttons["withIcon"],
                                                        ].join(" ")}
                                                        type="button"
                                                        aria-disabled={blocked}
                                                        aria-label={t(
                                                            record.enabled
                                                                ? "mcp.toggleDisable"
                                                                : "mcp.toggleEnable",
                                                            {
                                                                name: record.displayName,
                                                            },
                                                        )}
                                                        onClick={() => {
                                                            void mutate(
                                                                record,
                                                                false,
                                                            );
                                                        }}
                                                    >
                                                        <Icon
                                                            name={
                                                                record.enabled
                                                                    ? "disable"
                                                                    : "enable"
                                                            }
                                                        />
                                                        {t(
                                                            record.enabled
                                                                ? "mcp.disable"
                                                                : "mcp.enable",
                                                        )}
                                                    </button>
                                                    {record.cleanupPending && (
                                                        <button
                                                            className={
                                                                buttons[
                                                                    "secondary"
                                                                ]
                                                            }
                                                            type="button"
                                                            aria-disabled={
                                                                blocked
                                                            }
                                                            onClick={() => {
                                                                void mutate(
                                                                    record,
                                                                    true,
                                                                );
                                                            }}
                                                        >
                                                            {t("mcp.cleanup")}
                                                        </button>
                                                    )}
                                                </div>
                                            </div>
                                        </li>
                                    ))}
                                </ul>
                            )}
                            {list.nextCursor !== null && (
                                <button
                                    className={buttons["secondary"]}
                                    type="button"
                                    data-more="true"
                                    aria-disabled={
                                        blocked || list.more === "loading"
                                    }
                                    onClick={() => {
                                        void more();
                                    }}
                                >
                                    {t("mcp.loadMore")}
                                </button>
                            )}
                        </>
                    )}
                </>
            )}
        </section>
    );
}
