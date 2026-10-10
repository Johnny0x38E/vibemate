import { Checkbox } from "@base-ui/react/checkbox";
import type { JSX } from "react";
import styles from "./ModelCheckbox.module.css";

/** Inputs for {@link ModelCheckbox}. */
export interface ModelCheckboxProps {
    checked: boolean;
    disabled?: boolean;
    /** Shown to assistive tech; visible label lives on the model row. */
    ariaLabel: string;
    onCheckedChange: (checked: boolean) => void;
}

/**
 * Styled checkbox for model rows, using Base UI for focus and keyboard behavior.
 */
export function ModelCheckbox({
    checked,
    disabled = false,
    ariaLabel,
    onCheckedChange,
}: ModelCheckboxProps): JSX.Element {
    return (
        <Checkbox.Root
            className={styles["root"]}
            checked={checked}
            disabled={disabled}
            aria-label={ariaLabel}
            onCheckedChange={(next) => {
                onCheckedChange(next);
            }}
        >
            <Checkbox.Indicator className={styles["indicator"]}>
                <svg
                    className={styles["checkIcon"]}
                    viewBox="0 0 24 24"
                    aria-hidden="true"
                >
                    <path
                        d="M5 12l5 5L20 7"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="2.5"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                    />
                </svg>
            </Checkbox.Indicator>
        </Checkbox.Root>
    );
}
