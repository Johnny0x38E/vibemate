import type { JSX, ReactNode } from "react";
import styles from "./editForm.module.css";

/**
 * One editable form row: label on the left, control on the right.
 *
 * The row uses the shared 64px create/edit baseline. Children are the control
 * (an input, a dropdown, or a small field group) and sit in the shared column.
 * A non-string label is rendered as given, so it must already use the shared
 * label class. Settings rows that only host a narrow dropdown apply the
 * `editForm` classes directly and do not use this column.
 */
export function EditFormRow({
    label,
    htmlFor,
    children,
    className,
}: {
    /** Visible label. A string is wrapped in the shared label element. */
    label: ReactNode;
    /** When set, a string label becomes a `<label>` for this control id. */
    htmlFor?: string;
    children: ReactNode;
    /**
     * Extra row class, for example a section that grows past one control.
     * Admits `undefined` because CSS-module lookups can miss.
     */
    className?: string | undefined;
}): JSX.Element {
    const rowClassName = [styles["row"], styles["rowUniform"]];
    if (className !== undefined && className !== "") {
        rowClassName.push(className);
    }

    return (
        <div className={rowClassName.join(" ")}>
            {typeof label === "string" ? (
                htmlFor === undefined ? (
                    <span className={styles["label"]}>{label}</span>
                ) : (
                    <label className={styles["label"]} htmlFor={htmlFor}>
                        {label}
                    </label>
                )
            ) : (
                label
            )}
            <div className={styles["control"]}>{children}</div>
        </div>
    );
}
