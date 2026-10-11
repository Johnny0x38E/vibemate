import type { JSX } from "react";
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
    ["pi", { light: piLight, dark: piDark }],
    ["grokBuild", { light: grokBuildLight, dark: grokBuildDark }],
]);

/** `row` matches discovery cards; `default` is the 20px inline box. */
export type AgentIconSize = "default" | "row";

/**
 * Official agent mark in a fixed box (`object-fit: contain`). Decorative
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
    if (source === undefined) return null;
    return (
        <span className={styles["icon"]} data-size={size}>
            {source.dark === undefined ? (
                <img src={source.light} alt="" draggable={false} />
            ) : (
                <>
                    <img
                        className={styles["light"]}
                        src={source.light}
                        alt=""
                        draggable={false}
                        data-scheme="light"
                    />
                    <img
                        className={styles["dark"]}
                        src={source.dark}
                        alt=""
                        draggable={false}
                        data-scheme="dark"
                    />
                </>
            )}
        </span>
    );
}
