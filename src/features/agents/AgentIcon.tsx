import type { JSX } from "react";
import { Icon } from "../../components/Icon";
import claudeCode from "../../../assets/agents/claude-code.svg";
import codexDark from "../../../assets/agents/codex-dark.svg";
import codexLight from "../../../assets/agents/codex-light.svg";
import opencodeDark from "../../../assets/agents/opencode-dark.svg";
import opencodeLight from "../../../assets/agents/opencode-light.svg";
import hermesDark from "../../../assets/agents/hermes-dark.png";
import hermesLight from "../../../assets/agents/hermes-light.png";
import grokBuildDark from "../../../assets/agents/grok-build-dark.svg";
import grokBuildLight from "../../../assets/agents/grok-build-light.svg";
import piDark from "../../../assets/agents/pi-dark.svg";
import piLight from "../../../assets/agents/pi-light.svg";
import type { AgentId } from "../../lib/desktop/agents";
import styles from "./AgentIcon.module.css";

interface IconSource {
    light: string;
    dark?: string;
}

const ICONS = new Map<AgentId, IconSource>([
    ["claudeCode", { light: claudeCode }],
    ["codex", { light: codexLight, dark: codexDark }],
    ["openCode", { light: opencodeLight, dark: opencodeDark }],
    ["hermes", { light: hermesLight, dark: hermesDark }],
    ["pi", { light: piLight, dark: piDark }],
    ["grokBuild", { light: grokBuildLight, dark: grokBuildDark }],
]);

/** `row` matches discovery cards; `default` is the 20px inline box. */
export type AgentIconSize = "default" | "row";

/**
 * Official mark where available, otherwise a neutral Agent symbol. Decorative
 * (`alt=""`); the agent name beside it identifies the row.
 */
export function AgentIcon({
    id,
    size = "default",
}: {
    id: AgentId;
    size?: AgentIconSize;
}): JSX.Element | null {
    const source = ICONS.get(id);
    if (source === undefined)
        return (
            <span className={styles["icon"]} data-size={size}>
                <Icon name="agents" />
            </span>
        );
    return (
        <span className={styles["icon"]} data-size={size}>
            {source.dark === undefined ? (
                <img
                    src={source.light}
                    alt=""
                    draggable={false}
                    data-agent={id}
                />
            ) : (
                <>
                    <img
                        className={styles["light"]}
                        src={source.light}
                        alt=""
                        draggable={false}
                        data-agent={id}
                        data-scheme="light"
                    />
                    <img
                        className={styles["dark"]}
                        src={source.dark}
                        alt=""
                        draggable={false}
                        data-agent={id}
                        data-scheme="dark"
                    />
                </>
            )}
        </span>
    );
}
