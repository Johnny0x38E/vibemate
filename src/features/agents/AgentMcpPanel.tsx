import { useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import type { AgentId } from "../../lib/desktop/agents";
import { usePiMcpRegistrations } from "../mcp/usePiMcpRegistrations";
import buttons from "../../components/actionButtons.module.css";
import styles from "./AgentList.module.css";

/** Read-only Pi registration observations. Unsupported Agents do not run IPC;
 * management stays in the central MCP module, not a second configuration editor. */
export function AgentMcpPanel({
    agentId,
    hidden,
    onOpenMcp,
}: {
    agentId: AgentId;
    hidden: boolean;
    onOpenMcp: () => void;
}): JSX.Element {
    const { t } = useTranslation();
    const [epoch, setEpoch] = useState(0);
    const result = usePiMcpRegistrations(hidden || agentId !== "pi", epoch);
    return (
        <div className={styles["skillPanel"]}>
            {agentId !== "pi" ? (
                <p className={styles["hint"]}>
                    {t("agents.mcpPanel.unsupported")}
                </p>
            ) : (
                <>
                    <p className={styles["hint"]}>
                        {t("agents.mcpPanel.readOnly")}
                    </p>
                    {result.kind === "loading" && (
                        <p role="status">{t("mcp.pi.manage.loading")}</p>
                    )}
                    {result.kind === "error" && (
                        <p role="alert">{t(`mcp.pi.errors.${result.code}`)}</p>
                    )}
                    {result.kind === "ready" && (
                        <>
                            {result.value.recovery && (
                                <p role="status">
                                    {t("mcp.pi.recovery.pending", {
                                        name: result.value.recovery.serverName,
                                    })}
                                </p>
                            )}
                            {result.value.deployments.length === 0 ? (
                                <p>{t("agents.mcpPanel.empty")}</p>
                            ) : (
                                <ul
                                    className={styles["skillDeployList"]}
                                    aria-label={t("agents.mcpPanel.listLabel")}
                                >
                                    {result.value.deployments.map((row) => (
                                        <li key={row.definitionId}>
                                            <p
                                                className={
                                                    styles["skillDeployRow"]
                                                }
                                            >
                                                {t("agents.mcpPanel.row", {
                                                    name: row.displayName,
                                                    state: t(
                                                        `mcp.pi.states.${row.state}`,
                                                    ),
                                                    enabled: t(
                                                        row.ownership.enabled
                                                            ? "mcp.piPreview.on"
                                                            : "mcp.piPreview.off",
                                                    ),
                                                })}
                                            </p>
                                        </li>
                                    ))}
                                </ul>
                            )}
                        </>
                    )}
                    <button
                        type="button"
                        className={buttons["secondary"]}
                        onClick={() => {
                            setEpoch((value) => value + 1);
                        }}
                    >
                        {t("mcp.pi.manage.refresh")}
                    </button>
                </>
            )}
            <button
                type="button"
                className={buttons["secondary"]}
                onClick={onOpenMcp}
            >
                {t("agents.mcpPanel.openMcp")}
            </button>
        </div>
    );
}
