import { useEffect, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import {
    listAgentSkillDeployments,
    SkillRequestError,
    type SkillDeployAgent,
    type SkillDeploymentRecord,
    type SkillErrorCode,
} from "../../lib/desktop/skills";
import type { AgentId } from "../../lib/desktop/agents";
import buttons from "../../components/actionButtons.module.css";
import styles from "./AgentList.module.css";

type PanelState =
    | { kind: "loading" }
    | { kind: "preview" }
    | { kind: "ready"; items: SkillDeploymentRecord[] }
    | { kind: "error"; code: SkillErrorCode };

function errorCode(error: unknown): SkillErrorCode {
    return error instanceof SkillRequestError ? error.code : "operation_failed";
}

function asDeployAgent(agentId: AgentId): SkillDeployAgent | null {
    if (agentId === "pi" || agentId === "grokBuild" || agentId === "claudeCode")
        return agentId;
    return null;
}

/** Inputs for {@link AgentSkillPanel}. */
export interface AgentSkillPanelProps {
    agentId: AgentId;
    /** Jump to the Skills module home. */
    onOpenSkills: () => void;
}

/**
 * Read-only Skill slot on an Agent detail page. Claude Code, Pi and Grok Build
 * list vibemate-managed deployments and offer a jump to the Skills module;
 * other Agents stay honest about missing support.
 */
export function AgentSkillPanel({
    agentId,
    onOpenSkills,
}: AgentSkillPanelProps): JSX.Element {
    const { t } = useTranslation();
    const deployAgent = asDeployAgent(agentId);
    const [answer, setAnswer] = useState<{
        agentId: AgentId;
        state: PanelState;
    }>({ agentId, state: { kind: "loading" } });
    const generation = useRef(0);

    // When the Agent identity changes, reset in render so the effect only
    // starts the supported read and never calls setState synchronously for
    // "unsupported".
    if (answer.agentId !== agentId) {
        setAnswer({ agentId, state: { kind: "loading" } });
    }
    const state: PanelState | { kind: "unsupported" } =
        deployAgent === null
            ? { kind: "unsupported" }
            : answer.agentId === agentId
              ? answer.state
              : { kind: "loading" };

    useEffect(() => {
        if (deployAgent === null) return;
        const token = ++generation.current;
        void listAgentSkillDeployments(deployAgent).then(
            (result) => {
                if (generation.current !== token) return;
                setAnswer({
                    agentId,
                    state:
                        result.kind === "preview"
                            ? { kind: "preview" }
                            : { kind: "ready", items: result.items },
                });
            },
            (error: unknown) => {
                if (generation.current !== token) return;
                setAnswer({
                    agentId,
                    state: { kind: "error", code: errorCode(error) },
                });
            },
        );
        return () => {
            generation.current += 1;
        };
    }, [agentId, deployAgent]);

    return (
        <div className={styles["skillPanel"]}>
            {state.kind === "unsupported" && (
                <p className={styles["hint"]}>
                    {t("agents.skillPanel.unsupported")}
                </p>
            )}
            {state.kind === "loading" && (
                <p className={styles["hint"]}>
                    {t("agents.skillPanel.loading")}
                </p>
            )}
            {state.kind === "preview" && (
                <p className={styles["hint"]}>
                    {t("agents.skillPanel.desktopRequired")}
                </p>
            )}
            {state.kind === "error" && (
                <p className={styles["hint"]} role="alert">
                    {t(`skills.errors.${state.code}`)}
                </p>
            )}
            {state.kind === "ready" && state.items.length === 0 && (
                <p className={styles["hint"]}>{t("agents.skillPanel.empty")}</p>
            )}
            {state.kind === "ready" && state.items.length > 0 && (
                <ul
                    className={styles["skillDeployList"]}
                    aria-label={t("agents.skillPanel.listLabel")}
                >
                    {state.items.map((row) => (
                        <li key={row.skillId}>
                            <p className={styles["skillDeployRow"]}>
                                {t("agents.skillPanel.rowSummary", {
                                    name: row.skillName,
                                    mode: t(
                                        row.mode === "symlink"
                                            ? "skills.deploy.modeSymlink"
                                            : "skills.deploy.modeCopy",
                                    ),
                                    sync: t(
                                        row.inSync
                                            ? "skills.deploy.inSync"
                                            : "skills.deploy.outOfSync",
                                    ),
                                })}
                            </p>
                        </li>
                    ))}
                </ul>
            )}
            <button
                className={buttons["secondary"]}
                type="button"
                onClick={onOpenSkills}
            >
                {t("agents.skillPanel.openSkills")}
            </button>
        </div>
    );
}
