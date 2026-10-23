import { useEffect, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import {
    PiMcpError,
    readPiMcpState,
    type PiMcpErrorCode,
    type PiMcpState,
} from "../../lib/desktop/piMcp";
import buttons from "../../components/actionButtons.module.css";
import styles from "./McpPiPreview.module.css";

type State =
    | { kind: "loading" }
    | { kind: "ready"; value: PiMcpState }
    | { kind: "error"; code: PiMcpErrorCode };
/** Explain every retained Pi-root reference before deletion. This is advisory;
 * the backend repeats its transactional reference check at actual deletion. */
export function McpDeleteReferences({
    definitionId,
    onChecked,
    onManage,
}: {
    definitionId: string;
    onChecked: (allowed: boolean) => void;
    onManage: () => void;
}): JSX.Element {
    const { t } = useTranslation();
    const [state, setState] = useState<State>({ kind: "loading" });
    const [attempt, setAttempt] = useState(0);
    useEffect(() => {
        let current = true;
        readPiMcpState(definitionId).then(
            (value) => {
                if (current) {
                    setState({ kind: "ready", value });
                    onChecked(
                        value.deployments.length === 0 &&
                            value.recovery?.definitionId !== definitionId,
                    );
                }
            },
            (error: unknown) => {
                if (current) {
                    setState({
                        kind: "error",
                        code:
                            error instanceof PiMcpError
                                ? error.code
                                : "outcome_unknown",
                    });
                    onChecked(false);
                }
            },
        );
        return () => {
            current = false;
        };
    }, [definitionId, attempt, onChecked]);
    return (
        <div className={styles["section"]}>
            {state.kind === "loading" && (
                <p role="status">{t("mcp.pi.manage.loading")}</p>
            )}
            {state.kind === "error" && (
                <>
                    <p role="alert">{t(`mcp.pi.errors.${state.code}`)}</p>
                    <button
                        type="button"
                        className={buttons["secondary"]}
                        onClick={() => {
                            setState({ kind: "loading" });
                            setAttempt((value) => value + 1);
                        }}
                    >
                        {t("mcp.retry")}
                    </button>
                </>
            )}
            {state.kind === "ready" && (
                <>
                    {state.value.deployments.map((row) => (
                        <p key={row.configRoot}>
                            {t("mcp.pi.manage.deleteReference", {
                                path: row.configRoot,
                                state: t(`mcp.pi.states.${row.state}`),
                            })}
                        </p>
                    ))}
                    {state.value.recovery?.definitionId === definitionId && (
                        <p>
                            {t("mcp.pi.recovery.pending", {
                                name: state.value.recovery.serverName,
                            })}
                        </p>
                    )}
                    {(state.value.deployments.length > 0 ||
                        state.value.recovery?.definitionId ===
                            definitionId) && (
                        <button
                            type="button"
                            className={buttons["secondary"]}
                            onClick={onManage}
                        >
                            {t("mcp.pi.manage.resolveReferences")}
                        </button>
                    )}
                </>
            )}
        </div>
    );
}
