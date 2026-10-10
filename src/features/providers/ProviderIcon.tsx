import type { JSX } from "react";
import commandCode from "../../../assets/providers/command-code.svg";
import deepseek from "../../../assets/providers/deepseek.svg";
import openrouterVolt from "../../../assets/providers/openrouter-volt.svg";
import openrouter from "../../../assets/providers/openrouter.svg";
import styles from "./ProviderIcon.module.css";

/**
 * Official, unmodified brand files (sources in docs/frontend.md). `dark` is
 * only set when the brand ships a separate file for dark backgrounds.
 */
interface IconSource {
    light: string;
    dark?: string;
}

// Keyed by provider kind, never by display name. A Map ignores prototype keys.
const ICONS = new Map<string, IconSource>([
    ["command-code", { light: commandCode }],
    ["deepseek", { light: deepseek }],
    ["openrouter", { light: openrouter, dark: openrouterVolt }],
]);

/**
 * A provider's brand icon in a 20×20 box, scaled with `object-fit: contain` so
 * it is never stretched. It is decorative (`alt=""`): the provider or
 * configuration name next to it already identifies it. Unknown kinds render
 * nothing.
 *
 * For a brand with a dark-background variant, both files are rendered and the
 * stylesheet shows one, using the same conditions App.css uses to pick dark
 * tokens (`data-appearance` and the system scheme), so there is no second
 * theme detection in script.
 */
export function ProviderIcon({ kind }: { kind: string }): JSX.Element | null {
    const source = ICONS.get(kind);
    if (!source) return null;
    return (
        <span className={styles["icon"]}>
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
