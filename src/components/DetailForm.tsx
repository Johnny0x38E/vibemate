import type { JSX, ReactNode } from "react";
import styles from "./detailForm.module.css";

/**
 * Read-only detail card. The accessible name is the card's only heading.
 * Editable create/edit rows use {@link EditFormRow}.
 */
export function DetailPanel({
    label,
    children,
}: {
    /** Accessible name announced for this card. */
    label: string;
    children: ReactNode;
}): JSX.Element {
    return (
        <section
            className={[styles["card"], styles["panel"]].join(" ")}
            aria-label={label}
        >
            <div className={styles["facts"]}>{children}</div>
        </section>
    );
}

/**
 * One fact inside a {@link DetailPanel}: muted label, end-aligned value.
 */
export function DetailFact({
    label,
    value,
}: {
    label: string;
    value: ReactNode;
}): JSX.Element {
    return (
        <div className={styles["row"]}>
            <span className={styles["label"]}>{label}</span>
            <span className={styles["value"]}>{value}</span>
        </div>
    );
}
