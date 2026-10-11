import { useEffect, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
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
import styles from "./AgentList.module.css";

type State =
    | { kind: "loading" }
    | DiscoveryResult
    | { kind: "error"; code: DiscoveryErrorCode };
function failure(error: unknown): DiscoveryErrorCode {
    return error instanceof DiscoveryRequestError
        ? error.code
        : "operation_failed";
}

/**
 * Read-only entry discovery with window-local manual paths. Hidden navigation
 * preserves input/report, and performs no new scan. Generation checks discard
 * replies after unmount or StrictMode effect cleanup; no Agent is launched.
 */
export function AgentList({
    hidden = false,
}: {
    hidden?: boolean;
}): JSX.Element {
    const { t } = useTranslation();
    const [state, setState] = useState<State>({ kind: "loading" });
    const [piPath, setPiPath] = useState("");
    const [grokPath, setGrokPath] = useState("");
    const generation = useRef(0);
    const pending = useRef(false);
    const mounted = useRef(false);

    useEffect(() => {
        // Scan only on first mount. Typing paths or changing locale must not
        // perform filesystem work; Refresh/Enter explicitly submits new input.
        const current = ++generation.current;
        mounted.current = true;
        pending.current = true;
        void discoverAgents({ piPath: null, grokPath: null }).then(
            (result) => {
                if (mounted.current && current === generation.current) {
                    pending.current = false;
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
            if (mounted.current && current === generation.current)
                setState(result);
        } catch (error: unknown) {
            if (mounted.current && current === generation.current)
                setState({ kind: "error", code: failure(error) });
        } finally {
            if (mounted.current && current === generation.current)
                pending.current = false;
        }
    }
    const blocked = state.kind === "loading" || state.kind === "preview";
    function report(row: AgentObservation): JSX.Element {
        return (
            <div className={styles["report"]}>
                <p>
                    {t(`agents.states.${row.state}`)} ·{" "}
                    {t(`agents.sources.${row.source}`)}
                </p>
                {row.entryPath !== null && (
                    <p>{t("agents.entry", { path: row.entryPath })}</p>
                )}
                {row.resolvedPath !== null &&
                    row.resolvedPath !== row.entryPath && (
                        <p>{t("agents.target", { path: row.resolvedPath })}</p>
                    )}
                <p>
                    {t("agents.version", {
                        version: row.version ?? t("agents.unknown"),
                    })}{" "}
                    · {t(`agents.evidence.${row.versionEvidence}`)}
                </p>
                <p>{t(`agents.capabilities.${row.capabilityEvidence}`)}</p>
                <p>
                    {t("agents.directory", {
                        path: row.configDirectory ?? t("agents.unknown"),
                    })}{" "}
                    · {t(`agents.directories.${row.directorySource}`)}
                </p>
                <ul className={styles["files"]}>
                    {row.files.map((file) => (
                        <li key={file.name}>
                            {file.name} · {t(`agents.files.${file.state}`)}
                        </li>
                    ))}
                </ul>
            </div>
        );
    }
    return (
        <section className={styles["view"]} hidden={hidden}>
            <form
                onSubmit={(event) => {
                    event.preventDefault();
                    void refresh();
                }}
            >
                <PageModuleHeader
                    icon="agents"
                    title={t("desktop.nav.agents")}
                    trailing={
                        <button
                            type="submit"
                            className={buttons["primary"]}
                            aria-disabled={blocked}
                        >
                            {t("agents.refresh")}
                        </button>
                    }
                />
                <p className={styles["notice"]}>{t("agents.readOnly")}</p>
                {state.kind === "loading" && (
                    <p role="status">{t("agents.loading")}</p>
                )}
                {state.kind === "preview" && (
                    <p role="status">{t("agents.preview")}</p>
                )}
                {state.kind === "error" && (
                    <p className={styles["error"]} role="alert">
                        {t(`agents.errors.${state.code}`)}
                    </p>
                )}
                <ul
                    className={styles["list"]}
                    aria-label={t("desktop.nav.agents")}
                >
                    {(["pi", "grokBuild"] as const).map((id: AgentId) => {
                        const row =
                            state.kind === "desktop"
                                ? state.items.find((item) => item.id === id)
                                : undefined;
                        return (
                            <li className={styles["item"]} key={id}>
                                <h2 className={styles["name"]}>
                                    {t(`agents.names.${id}`)}
                                </h2>
                                <label className={styles["field"]}>
                                    <span>
                                        {t("agents.pathLabel", {
                                            name: t(`agents.names.${id}`),
                                        })}
                                    </span>
                                    <input
                                        className={styles["input"]}
                                        value={id === "pi" ? piPath : grokPath}
                                        readOnly={blocked}
                                        spellCheck={false}
                                        autoComplete="off"
                                        placeholder={t(
                                            "agents.pathPlaceholder",
                                        )}
                                        onChange={(event) => {
                                            if (id === "pi")
                                                setPiPath(event.target.value);
                                            else
                                                setGrokPath(event.target.value);
                                        }}
                                    />
                                </label>
                                {row !== undefined && report(row)}
                                <p className={styles["hint"]}>
                                    {t(`agents.precedence.${id}`)}
                                </p>
                            </li>
                        );
                    })}
                </ul>
            </form>
            <p className={styles["hint"]}>{t("agents.scope")}</p>
        </section>
    );
}
