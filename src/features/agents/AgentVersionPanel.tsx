import { useEffect, useLayoutEffect, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import { DetailFact } from "../../components/DetailForm";
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
    | { kind: "running" }
    | { kind: "error"; code: VersionProbeErrorCode };

/** User-triggered inline version probe; never probes on mount.
 * The parent owns window-local results and the shared navigation/scan busy guard.
 * Remount this panel when a discovery generation or its selected target changes.
 * It contributes one row to the Agent detail card; the reported value is shown
 * in the title row, so this only offers the action and its failures.
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
    async function run(): Promise<void> {
        if (!canProbe || pending.current) return;
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
        <>
            <DetailFact
                label={t("agents.versionProbe.title")}
                value={
                    <button
                        type="button"
                        className={buttons["secondary"]}
                        aria-disabled={!canProbe || state.kind === "running"}
                        onClick={() => {
                            void run();
                        }}
                    >
                        {t("agents.versionProbe.read")}
                    </button>
                }
            />
            {/* No busy label: the probe is fast, and extra card content made
                the card jump. Repeat clicks are blocked by aria-disabled. */}
            {state.kind === "error" && (
                <p className={styles["error"]} role="alert">
                    {t(`agents.versionProbe.errors.${state.code}`)}
                </p>
            )}
        </>
    );
}
