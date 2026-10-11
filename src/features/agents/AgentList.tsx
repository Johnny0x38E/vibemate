import {
    useEffect,
    useImperativeHandle,
    useRef,
    useState,
    type Ref,
    type JSX,
} from "react";
import type { ModuleNavigationHandle } from "../../components/moduleNavigation";
import { Tooltip } from "@base-ui/react/tooltip";
import { useTranslation } from "react-i18next";
import { DetailFact, DetailPanel } from "../../components/DetailForm";
import editForm from "../../components/editForm.module.css";
import detailForm from "../../components/detailForm.module.css";
import { PageModuleHeader } from "../../components/PageModuleHeader";
import {
    discoverAgents,
    DiscoveryRequestError,
    type AgentId,
    type AgentObservation,
    type DiscoveryErrorCode,
    type DiscoveryResult,
} from "../../lib/desktop/agents";
import buttons from "../providers/providerButtons.module.css";
import pageStyles from "../providers/ProviderPage.module.css";
import { AgentIcon } from "./AgentIcon";
import { IconButton } from "../../components/IconButton";
import { AgentPage } from "./AgentPage";
import styles from "./AgentList.module.css";

type State =
    | { kind: "loading" }
    | DiscoveryResult
    | { kind: "error"; code: DiscoveryErrorCode };

type AgentsPage = { kind: "list" } | { kind: "detail"; id: AgentId };

function failure(error: unknown): DiscoveryErrorCode {
    return error instanceof DiscoveryRequestError
        ? error.code
        : "operation_failed";
}

function versionLabel(row: AgentObservation, unknownLabel: string): string {
    return row.version ?? unknownLabel;
}

/**
 * Read-only discovery: auto-scan on first mount, card list for found entries,
 * and a detail view for paths and config file presence. Hidden navigation keeps
 * the last report and manual paths without rescanning.
 */
export function AgentList({
    hidden = false,
    homeRef,
}: {
    hidden?: boolean;
    homeRef?: Ref<ModuleNavigationHandle>;
}): JSX.Element {
    const { t } = useTranslation();
    const [state, setState] = useState<State>({ kind: "loading" });
    /** Last successful desktop report; kept across refresh errors and loading. */
    const [snapshot, setSnapshot] = useState<AgentObservation[]>([]);
    const [page, setPage] = useState<AgentsPage>({ kind: "list" });
    const [piPath, setPiPath] = useState("");
    const [grokPath, setGrokPath] = useState("");
    const generation = useRef(0);
    const pending = useRef(false);
    const mounted = useRef(false);
    const focusCard = useRef<AgentId | null>(null);
    useImperativeHandle(homeRef, () => ({
        openHome() {
            if (pending.current) return false;
            focusCard.current = null;
            setPage({ kind: "list" });
            return true;
        },
    }));

    useEffect(() => {
        const current = ++generation.current;
        mounted.current = true;
        pending.current = true;
        void discoverAgents({ piPath: null, grokPath: null }).then(
            (result) => {
                if (mounted.current && current === generation.current) {
                    pending.current = false;
                    if (result.kind === "desktop") setSnapshot(result.items);
                    setState(result);
                }
            },
            (error: unknown) => {
                if (mounted.current && current === generation.current) {
                    pending.current = false;
                    setState({ kind: "error", code: failure(error) });
                }
            },
        );
        return () => {
            mounted.current = false;
            pending.current = false;
        };
    }, []);

    useEffect(() => {
        if (hidden || page.kind !== "list" || focusCard.current === null)
            return;
        const id = focusCard.current;
        focusCard.current = null;
        const target = document.querySelector<HTMLElement>(
            `[data-agent-card="${id}"]`,
        );
        target?.focus();
    }, [hidden, page]);

    async function refresh(): Promise<void> {
        if (pending.current || state.kind === "preview") return;
        pending.current = true;
        const current = ++generation.current;
        setState({ kind: "loading" });
        try {
            const result = await discoverAgents({
                piPath: piPath.trim() || null,
                grokPath: grokPath.trim() || null,
            });
            if (mounted.current && current === generation.current) {
                if (result.kind === "desktop") setSnapshot(result.items);
                setState(result);
            }
        } catch (error: unknown) {
            if (mounted.current && current === generation.current)
                setState({ kind: "error", code: failure(error) });
        } finally {
            if (mounted.current && current === generation.current)
                pending.current = false;
        }
    }

    const blocked = state.kind === "loading" || state.kind === "preview";

    function items(): AgentObservation[] {
        return state.kind === "desktop" ? state.items : snapshot;
    }

    function observation(id: AgentId): AgentObservation | undefined {
        return items().find((item) => item.id === id);
    }

    function foundRows(): AgentObservation[] {
        return items().filter((row) => row.state === "found");
    }

    function openDetail(id: AgentId): void {
        if (blocked) return;
        setPage({ kind: "detail", id });
    }

    function returnToList(from: AgentId): void {
        focusCard.current = from;
        setPage({ kind: "list" });
    }

    function configFileLabel(name: string): string {
        switch (name) {
            case "settings.json":
                return t("agents.files.items.settings.json");
            case "models.json":
                return t("agents.files.items.models.json");
            case "mcp.json":
                return t("agents.files.items.mcp.json");
            case "config.toml":
                return t("agents.files.items.config.toml");
            case "managed_config.toml":
                return t("agents.files.items.managed_config.toml");
            case "requirements.toml":
                return t("agents.files.items.requirements.toml");
            default:
                return name;
        }
    }

    function factBadge(value: string, hint: string): JSX.Element {
        // Evidence remains available without turning a version hint into a
        // verified installation claim. Focusable text also exposes it to keyboard users.
        return (
            <Tooltip.Provider delay={300}>
                <Tooltip.Root>
                    <Tooltip.Trigger
                        render={<span tabIndex={0} />}
                        className={styles["badge"]}
                    >
                        {value}
                    </Tooltip.Trigger>
                    <Tooltip.Portal>
                        <Tooltip.Positioner
                            className={styles["badgePositioner"]}
                            sideOffset={6}
                        >
                            <Tooltip.Popup className={styles["badgeHint"]}>
                                {hint}
                            </Tooltip.Popup>
                        </Tooltip.Positioner>
                    </Tooltip.Portal>
                </Tooltip.Root>
            </Tooltip.Provider>
        );
    }

    function report(row: AgentObservation): JSX.Element {
        return (
            <>
                <DetailFact
                    label={t("agents.detail.labels.status")}
                    value={factBadge(
                        t(`agents.states.${row.state}`),
                        t(`agents.sources.${row.source}`),
                    )}
                />
                {row.entryPath !== null && (
                    <DetailFact
                        label={t("agents.detail.labels.entry")}
                        value={row.entryPath}
                    />
                )}
                {row.resolvedPath !== null &&
                    row.resolvedPath !== row.entryPath && (
                        <DetailFact
                            label={t("agents.detail.labels.target")}
                            value={row.resolvedPath}
                        />
                    )}
                <DetailFact
                    label={t("agents.detail.labels.version")}
                    value={factBadge(
                        row.version ?? t("agents.unknown"),
                        t(`agents.evidence.${row.versionEvidence}`),
                    )}
                />
            </>
        );
    }

    function configFiles(row: AgentObservation): JSX.Element {
        return (
            <>
                <DetailFact
                    label={t("agents.detail.labels.configDirectory")}
                    value={`${row.configDirectory ?? t("agents.unknown")} · ${t(`agents.directories.${row.directorySource}`)}`}
                />
                {row.files.map((file) => (
                    <DetailFact
                        key={file.name}
                        label={configFileLabel(file.name)}
                        value={t(`agents.files.${file.state}`)}
                    />
                ))}
            </>
        );
    }

    function refreshButton(): JSX.Element {
        return (
            <button
                type="button"
                className={buttons["primary"]}
                aria-disabled={blocked}
                onClick={() => {
                    void refresh();
                }}
            >
                {t("agents.refresh")}
            </button>
        );
    }

    function statusLine(): JSX.Element | null {
        if (state.kind === "loading") {
            return (
                <p className={styles["hint"]} role="status">
                    {t("agents.loading")}
                </p>
            );
        }
        if (state.kind === "preview") {
            return (
                <p className={styles["hint"]} role="status">
                    {t("agents.preview")}
                </p>
            );
        }
        return null;
    }

    function listView(): JSX.Element {
        const rows = foundRows();
        return (
            <>
                <PageModuleHeader
                    icon="agents"
                    title={t("desktop.nav.agents")}
                    trailing={refreshButton()}
                />
                {statusLine()}
                {state.kind === "error" && (
                    <p className={styles["error"]} role="alert">
                        {t(`agents.errors.${state.code}`)}
                    </p>
                )}
                {state.kind !== "loading" &&
                    state.kind !== "preview" &&
                    rows.length === 0 && (
                        <p className={styles["notice"]} role="status">
                            {t("agents.list.empty")}
                        </p>
                    )}
                {rows.length > 0 && (
                    <ul
                        className={styles["cards"]}
                        aria-label={t("agents.list.label")}
                    >
                        {rows.map((row) => (
                            <li key={row.id}>
                                <button
                                    type="button"
                                    className={styles["card"]}
                                    data-agent-card={row.id}
                                    aria-disabled={blocked}
                                    aria-label={t("agents.list.openDetail", {
                                        name: t(`agents.names.${row.id}`),
                                    })}
                                    onClick={() => {
                                        openDetail(row.id);
                                    }}
                                >
                                    <AgentIcon id={row.id} size="row" />
                                    <span className={styles["cardText"]}>
                                        <span className={styles["cardName"]}>
                                            {t(`agents.names.${row.id}`)}
                                        </span>
                                        <span className={styles["cardVersion"]}>
                                            {versionLabel(
                                                row,
                                                t("agents.unknown"),
                                            )}
                                        </span>
                                    </span>
                                </button>
                            </li>
                        ))}
                    </ul>
                )}
            </>
        );
    }

    function detailView(id: AgentId): JSX.Element {
        const row = observation(id);
        const pathValue = id === "pi" ? piPath : grokPath;
        const setPath = id === "pi" ? setPiPath : setGrokPath;
        const name = t(`agents.names.${id}`);

        return (
            <AgentPage
                key={id}
                title={
                    <span className={pageStyles["titleBrand"]}>
                        <AgentIcon id={id} />
                        <span className={pageStyles["titleBrandText"]}>
                            {name}
                        </span>
                    </span>
                }
                onBack={() => {
                    returnToList(id);
                }}
                backBlocked={blocked}
            >
                {statusLine()}
                {state.kind === "error" && (
                    <p className={styles["error"]} role="alert">
                        {t(`agents.errors.${state.code}`)}
                    </p>
                )}
                {row === undefined || row.state !== "found"
                    ? state.kind !== "error" && (
                          <p className={styles["error"]} role="alert">
                              {t("agents.detail.notFound")}
                          </p>
                      )
                    : null}
                <section className={styles["detailGroup"]} data-layout="page">
                    {row !== undefined && row.state === "found" && (
                        <>
                            <DetailPanel
                                label={t("agents.detail.reportLabel", {
                                    name,
                                })}
                            >
                                {report(row)}
                            </DetailPanel>
                            <DetailPanel
                                label={t("agents.detail.configFilesHeading")}
                            >
                                {configFiles(row)}
                            </DetailPanel>
                        </>
                    )}
                    <form
                        className={[
                            detailForm["card"],
                            styles["pathForm"],
                        ].join(" ")}
                        aria-label={t("agents.detail.manualPathTitle")}
                        onSubmit={(event) => {
                            event.preventDefault();
                            void refresh();
                        }}
                    >
                        <label
                            className={detailForm["label"]}
                            htmlFor={`agent-entry-path-${id}`}
                        >
                            {t("agents.pathLabel", { name })}
                        </label>
                        <div className={styles["pathControls"]}>
                            <input
                                id={`agent-entry-path-${id}`}
                                className={[
                                    editForm["input"],
                                    styles["pathInput"],
                                ].join(" ")}
                                value={pathValue}
                                readOnly={blocked}
                                spellCheck={false}
                                autoComplete="off"
                                placeholder={t("agents.pathPlaceholder")}
                                onChange={(event) => {
                                    setPath(event.currentTarget.value);
                                }}
                            />
                            <IconButton
                                type="submit"
                                icon="refresh"
                                label={t("agents.detail.applyPath")}
                                blocked={blocked}
                            />
                        </div>
                    </form>
                </section>
            </AgentPage>
        );
    }

    return (
        <section
            className={styles["view"]}
            hidden={hidden}
            data-secondary-page={page.kind !== "list" ? true : undefined}
        >
            {page.kind === "list" ? listView() : detailView(page.id)}
        </section>
    );
}
