import {
    useEffect,
    useLayoutEffect,
    useRef,
    useState,
    type JSX,
    type KeyboardEvent,
} from "react";
import { useTranslation } from "react-i18next";
import { DetailFact, DetailPanel } from "../../components/DetailForm";
import buttons from "../../components/actionButtons.module.css";
import { type AgentObservation } from "../../lib/desktop/agents";
import {
    probeAgentVersion,
    VersionProbeRequestError,
    type AgentVersionResult,
    type VersionProbeErrorCode,
} from "../../lib/desktop/agentVersions";
import styles from "./AgentList.module.css";

type ProbeState =
    | { kind: "idle" }
    | { kind: "confirm" }
    | { kind: "running" }
    | { kind: "error"; code: VersionProbeErrorCode };

/** User-triggered, inline execution confirmation; never probes on mount.
 * The parent owns window-local results and the shared navigation/scan busy guard.
 * Remount this panel when a discovery generation or its selected target changes.
 */
export function AgentVersionPanel({
    row,
    enabled,
    hidden,
    onBusyChange,
    onResult,
}: {
    row: AgentObservation | undefined;
    enabled: boolean;
    hidden: boolean;
    onBusyChange: (busy: boolean) => void;
    onResult: (result: AgentVersionResult) => void;
}): JSX.Element {
    const { t } = useTranslation();
    const [state, setState] = useState<ProbeState>({ kind: "idle" });
    const mounted = useRef(false);
    const pending = useRef(false);
    const hiddenNow = useRef(hidden);
    // Synchronize before passive effects so an IPC reply cannot publish a
    // result between a hidden-view commit and its visibility update.
    useLayoutEffect(() => {
        hiddenNow.current = hidden;
    }, [hidden]);
    const opener = useRef<HTMLButtonElement>(null);
    useEffect(() => {
        mounted.current = true;
        // IPC launch cannot be undone by hiding the UI. Ignore results after
        // unmount; Rust independently bounds and cleans up the direct child.
        return () => {
            mounted.current = false;
        };
    }, []);
    const entryPath = row?.entryPath;
    const resolvedPath = row?.resolvedPath;
    const canProbe =
        enabled &&
        !hidden &&
        row?.state === "found" &&
        typeof entryPath === "string" &&
        typeof resolvedPath === "string";
    function cancel(): void {
        if (pending.current) return;
        setState({ kind: "idle" });
        opener.current?.focus();
    }
    function cancelOnEscape(event: KeyboardEvent<HTMLButtonElement>): void {
        if (event.key === "Escape" && state.kind === "confirm") {
            event.preventDefault();
            cancel();
        }
    }
    async function run(): Promise<void> {
        if (!canProbe || state.kind !== "confirm" || pending.current) return;
        pending.current = true;
        onBusyChange(true);
        setState({ kind: "running" });
        try {
            const result = await probeAgentVersion({
                id: row.id,
                entryPath,
                resolvedPath,
                confirmed: true,
            });
            if (mounted.current) {
                if (!hiddenNow.current) onResult(result);
                setState({ kind: "idle" });
            }
        } catch (error: unknown) {
            if (mounted.current)
                setState({
                    kind: "error",
                    code:
                        error instanceof VersionProbeRequestError
                            ? error.code
                            : "operation_failed",
                });
        } finally {
            pending.current = false;
            if (mounted.current) onBusyChange(false);
        }
    }
    return (
        <DetailPanel label={t("agents.versionProbe.title")}>
            <DetailFact
                label={t("agents.versionProbe.title")}
                value={
                    <button
                        ref={opener}
                        onKeyDown={cancelOnEscape}
                        type="button"
                        className={buttons["secondary"]}
                        aria-disabled={
                            !canProbe ||
                            state.kind === "running" ||
                            state.kind === "confirm"
                        }
                        onClick={() => {
                            if (
                                !canProbe ||
                                pending.current ||
                                state.kind === "confirm"
                            )
                                return;
                            setState({ kind: "confirm" });
                        }}
                    >
                        {t("agents.versionProbe.read")}
                    </button>
                }
            />
            <p className={styles["hint"]}>{t("agents.versionProbe.hint")}</p>
            {!canProbe && state.kind !== "running" && (
                <p className={styles["hint"]}>
                    {t("agents.versionProbe.requiresDiscovery")}
                </p>
            )}
            {state.kind === "confirm" && row !== undefined && (
                <div
                    role="group"
                    aria-label={t("agents.versionProbe.confirmTitle")}
                >
                    <DetailFact
                        label={t("agents.versionProbe.command")}
                        value={<code>{row.resolvedPath} --version</code>}
                    />
                    {row.entryPath !== row.resolvedPath && (
                        <DetailFact
                            label={t("agents.detail.labels.entry")}
                            value={row.entryPath}
                        />
                    )}
                    <p className={styles["notice"]}>
                        {t("agents.versionProbe.warning")}
                    </p>
                    <p className={styles["hint"]}>
                        {t("agents.versionProbe.limits")}
                    </p>
                    <div className={styles["pathControls"]}>
                        <button
                            type="button"
                            className={buttons["secondary"]}
                            aria-disabled={!canProbe}
                            onKeyDown={cancelOnEscape}
                            onClick={() => {
                                void run();
                            }}
                        >
                            {t("agents.versionProbe.run")}
                        </button>
                        <button
                            type="button"
                            className={buttons["secondary"]}
                            onClick={cancel}
                            onKeyDown={cancelOnEscape}
                        >
                            {t("agents.versionProbe.cancel")}
                        </button>
                    </div>
                </div>
            )}
            <p
                className={styles["hint"]}
                aria-live="polite"
                role={state.kind === "running" ? "status" : undefined}
            >
                {state.kind === "running"
                    ? t("agents.versionProbe.running")
                    : ""}
            </p>
            {state.kind === "error" && (
                <p className={styles["error"]} role="alert">
                    {t(`agents.versionProbe.errors.${state.code}`)}
                </p>
            )}
        </DetailPanel>
    );
}
