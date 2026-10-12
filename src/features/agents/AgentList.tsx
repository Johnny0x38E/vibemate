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
import { Icon } from "../../components/Icon";
import { PageModuleHeader } from "../../components/PageModuleHeader";
import {
    discoverAgents,
    DiscoveryRequestError,
    AGENT_IDS,
    type DiscoveryInput,
    type AgentId,
    type AgentObservation,
    type DiscoveryErrorCode,
    type DiscoveryResult,
} from "../../lib/desktop/agents";
import buttons from "../../components/actionButtons.module.css";
import pageStyles from "../providers/ProviderPage.module.css";
import { AgentIcon } from "./AgentIcon";
import { IconButton } from "../../components/IconButton";
import { AgentPage } from "./AgentPage";
import { AgentVersionPanel } from "./AgentVersionPanel";
import type { AgentVersionResult } from "../../lib/desktop/agentVersions";
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

/**
 * Fixed six-Agent catalog: auto-scan on first mount, including absent entries,
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
    const [versions, setVersions] = useState<
        Partial<Record<AgentId, AgentVersionResult>>
    >({});
    const [versionBusy, setVersionBusy] = useState(false);
    const [confirmationEpoch, setConfirmationEpoch] = useState(0);
    const [paths, setPaths] = useState<Record<AgentId, string>>({
        claudeCode: "",
        codex: "",
        openCode: "",
        pi: "",
        grokBuild: "",
        hermes: "",
    });
    function discoveryInput(): DiscoveryInput {
        return {
            claudePath: paths.claudeCode.trim() || null,
            codexPath: paths.codex.trim() || null,
            opencodePath: paths.openCode.trim() || null,
            piPath: paths.pi.trim() || null,
            grokPath: paths.grokBuild.trim() || null,
            hermesPath: paths.hermes.trim() || null,
        };
    }
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
        void discoverAgents({
            claudePath: null,
            codexPath: null,
            opencodePath: null,
            piPath: null,
            grokPath: null,
            hermesPath: null,
        }).then(
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
        // CLI results describe a previously checked target. A new discovery
        // invalidates them even when it later fails or returns the same path.
        setVersions({});
        setConfirmationEpoch((previous) => previous + 1);
        setState({ kind: "loading" });
        try {
            const result = await discoverAgents(discoveryInput());
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

    const blocked =
        state.kind === "loading" || state.kind === "preview" || versionBusy;

    function cliVersion(row: AgentObservation): AgentVersionResult | undefined {
        const result = versions[row.id];
        return result?.entryPath === row.entryPath &&
            result.resolvedPath === row.resolvedPath
            ? result
            : undefined;
    }

    function items(): AgentObservation[] {
        return state.kind === "desktop" ? state.items : snapshot;
    }

    function observation(id: AgentId): AgentObservation | undefined {
        return items().find((item) => item.id === id);
    }

    function discoveryLabel(row: AgentObservation | undefined): string {
        if (state.kind === "loading") return t("agents.list.checking");
        if (state.kind === "preview") return t("agents.list.preview");
        if (state.kind === "error") {
            return row === undefined
                ? t("agents.list.failed")
                : t("agents.list.previousStatus", {
                      status: t(`agents.states.${row.state}`),
                  });
        }
        return row === undefined
            ? t("agents.list.failed")
            : t(`agents.states.${row.state}`);
    }

    function openDetail(id: AgentId): void {
        if (state.kind === "loading" || versionBusy) return;
        setPage({ kind: "detail", id });
    }

    function returnToList(from: AgentId): void {
        if (pending.current) return;
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
            case "auth.json":
                return t("agents.files.items.auth.json");
            case "opencode.json":
                return t("agents.files.items.opencode.json");
            case "opencode.jsonc":
                return t("agents.files.items.opencode.jsonc");
            case "config.yaml":
                return t("agents.files.items.config.yaml");
            case ".env":
                return t("agents.files.items.env");
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
        const reported = cliVersion(row);
        return (
            <>
                <DetailFact
                    label={t("agents.detail.labels.status")}
                    value={factBadge(
                        discoveryLabel(row),
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
                        reported?.version ?? row.version ?? t("agents.unknown"),
                        reported === undefined
                            ? t(`agents.evidence.${row.versionEvidence}`)
                            : t("agents.versionProbe.evidence"),
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
                className={[buttons["primary"], buttons["withIcon"]]
                    .filter((value): value is string => value !== undefined)
                    .join(" ")}
                aria-disabled={blocked}
                onClick={() => {
                    void refresh();
                }}
            >
                <Icon name="refresh" />
                {t("agents.refresh")}
            </button>
        );
    }

    function statusLine(): JSX.Element {
        const message =
            state.kind === "loading"
                ? t("agents.loading")
                : state.kind === "preview"
                  ? ""
                  : state.kind === "error"
                    ? t("agents.list.failed")
                    : t("agents.complete");
        // Keep the live region mounted so completion is announced without
        // moving focus away from the path or refresh action.
        return (
            <p
                className={styles["hint"]}
                role="status"
                hidden={state.kind === "preview"}
            >
                {message}
            </p>
        );
    }

    function listView(): JSX.Element {
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
                {
                    <ul
                        className={styles["cards"]}
                        aria-label={t("agents.list.label")}
                    >
                        {AGENT_IDS.map((id) => {
                            const row = observation(id);
                            const reported =
                                row === undefined ? undefined : cliVersion(row);
                            return (
                                <li key={id}>
                                    <button
                                        type="button"
                                        className={styles["card"]}
                                        data-agent-card={id}
                                        aria-disabled={
                                            state.kind === "loading" ||
                                            versionBusy
                                        }
                                        aria-describedby={`agent-card-status-${id}`}
                                        aria-label={t(
                                            "agents.list.openDetail",
                                            {
                                                name: t(`agents.names.${id}`),
                                            },
                                        )}
                                        onClick={() => {
                                            openDetail(id);
                                        }}
                                    >
                                        <AgentIcon id={id} size="row" />
                                        <span className={styles["cardText"]}>
                                            <span
                                                className={styles["cardName"]}
                                            >
                                                {t(`agents.names.${id}`)}
                                            </span>
                                            <span
                                                className={
                                                    styles["cardStatusRow"]
                                                }
                                                id={`agent-card-status-${id}`}
                                            >
                                                <span
                                                    className={
                                                        styles["cardStatus"]
                                                    }
                                                >
                                                    <span
                                                        className={
                                                            styles["statusDot"]
                                                        }
                                                        aria-hidden="true"
                                                        data-state={
                                                            state.kind ===
                                                            "desktop"
                                                                ? (row?.state ??
                                                                  "unavailable")
                                                                : state.kind
                                                        }
                                                    />
                                                    <span
                                                        className={
                                                            styles["statusText"]
                                                        }
                                                    >
                                                        {discoveryLabel(row)}
                                                    </span>
                                                </span>
                                                <span
                                                    className={
                                                        styles["cardVersion"]
                                                    }
                                                    title={
                                                        reported === undefined
                                                            ? row === undefined
                                                                ? undefined
                                                                : t(
                                                                      `agents.evidence.${row.versionEvidence}`,
                                                                  )
                                                            : t(
                                                                  "agents.versionProbe.evidence",
                                                              )
                                                    }
                                                >
                                                    {row?.state === "found"
                                                        ? (reported?.version ??
                                                          row.version ??
                                                          t("agents.unknown"))
                                                        : null}
                                                </span>
                                            </span>
                                        </span>
                                    </button>
                                </li>
                            );
                        })}
                    </ul>
                }
            </>
        );
    }

    function detailView(id: AgentId): JSX.Element {
        const row = observation(id);
        const pathValue = paths[id];
        const name = t(`agents.names.${id}`);
        const pathMatches =
            pathValue.trim() === ""
                ? row?.source !== "manual"
                : pathValue.trim() === row?.entryPath;

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
                {row?.state === "notFound" && state.kind === "desktop" ? (
                    <p className={styles["notice"]}>
                        {t("agents.detail.notFound")}
                    </p>
                ) : null}
                <section className={styles["detailGroup"]} data-layout="page">
                    {row !== undefined && (
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
                                    setPaths({
                                        ...paths,
                                        [id]: event.currentTarget.value,
                                    });
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
                    <AgentVersionPanel
                        key={JSON.stringify([
                            confirmationEpoch,
                            id,
                            row?.entryPath,
                            row?.resolvedPath,
                        ])}
                        row={row}
                        enabled={
                            !blocked && state.kind === "desktop" && pathMatches
                        }
                        hidden={hidden}
                        onBusyChange={(busy) => {
                            pending.current = busy;
                            if (mounted.current) setVersionBusy(busy);
                        }}
                        onResult={(result) => {
                            if (mounted.current && !hidden)
                                setVersions((previous) => ({
                                    ...previous,
                                    [result.id]: result,
                                }));
                        }}
                    />
                </section>
                <p className={styles["notice"]}>{t("agents.readOnly")}</p>
                <p className={styles["hint"]}>{t(`agents.precedence.${id}`)}</p>
                <p className={styles["hint"]}>{t("agents.scope")}</p>
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
