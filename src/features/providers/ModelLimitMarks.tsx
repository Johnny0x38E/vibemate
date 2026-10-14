import type { JSX } from "react";
import { useTranslation } from "react-i18next";
import { tokenStopLabel } from "./tokenStops";
import styles from "./ModelLimitMarks.module.css";

/** Inputs for {@link ModelLimitMarks}. */
export interface ModelLimitMarksProps {
    contextWindow: number | null;
    maxOutputTokens: number | null;
}

/**
 * Context window and maximum output on a model row. Each mark is an icon plus
 * the nearest preset label. A missing limit is omitted.
 */
export function ModelLimitMarks({
    contextWindow,
    maxOutputTokens,
}: ModelLimitMarksProps): JSX.Element | null {
    const { t } = useTranslation();
    const marks = [
        {
            key: "context",
            label: tokenStopLabel(contextWindow),
            name: "providers.models.limits.contextWindow" as const,
            icon: "context" as const,
        },
        {
            key: "output",
            label: tokenStopLabel(maxOutputTokens),
            name: "providers.models.limits.maxOutput" as const,
            icon: "output" as const,
        },
    ].filter((mark) => mark.label !== null);
    if (marks.length === 0) return null;
    return (
        <span className={styles["marks"]}>
            {marks.map((mark) => (
                <span
                    key={mark.key}
                    className={styles["mark"]}
                    role="img"
                    aria-label={t(mark.name, { size: mark.label })}
                >
                    <svg
                        className={styles["icon"]}
                        viewBox="0 0 24 24"
                        aria-hidden="true"
                    >
                        {mark.icon === "context" ? (
                            <>
                                <path d="M4 8v8M20 8v8M4 12h16" />
                                <path d="m8 9-3 3 3 3M16 9l3 3-3 3" />
                            </>
                        ) : (
                            <>
                                <path d="M6 5h12" />
                                <path d="M12 9v8" />
                                <path d="m8 13 4 4 4-4" />
                            </>
                        )}
                    </svg>
                    <span className={styles["label"]} aria-hidden="true">
                        {mark.label}
                    </span>
                </span>
            ))}
        </span>
    );
}
