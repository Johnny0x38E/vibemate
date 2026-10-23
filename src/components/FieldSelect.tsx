import { Select } from "@base-ui/react/select";
import { useState, type JSX, type ReactNode } from "react";
import styles from "./FieldSelect.module.css";

/** One row in a {@link FieldSelect} list. */
export interface FieldSelectOption {
    value: string;
    label: ReactNode;
    /** Shown before the label in the trigger and list (for example a brand icon). */
    leading?: ReactNode;
    /** Applied to the list option element (for example `lang` on language rows). */
    optionLang?: string;
}

/** Props for {@link FieldSelect}. */
export interface FieldSelectProps {
    id: string;
    value: string;
    options: readonly FieldSelectOption[];
    onChange: (value: string) => void;
    /** Blocks interaction but keeps focusable (`aria-disabled`), like provider forms. */
    blocked?: boolean;
    /** Native disabled state for settings controls that use `disabled` while busy. */
    disabled?: boolean;
    /**
     * Trigger width. Default is the narrow settings control; `fill` stretches the
     * trigger to the parent column, where the row's other controls are full width
     * (MCP transport, provider protocol and provider kind, Agent detail model and
     * provider pickers) or a long value would truncate (provider model IDs).
     */
    size?: "default" | "fill";
    /** Accessible name when the visible label is not a native `<label>`. */
    "aria-label"?: string | undefined;
    "aria-describedby"?: string | undefined;
    "aria-invalid"?: boolean | undefined;
    /** Optional test hook (for example provider kind). */
    "data-test-value"?: string;
}

/**
 * Shared single-choice field, styled by vibemate and operated by Base UI.
 * The parent owns the saved value; Base UI owns navigation, focus and dismissal.
 * Portaling the popup avoids clipping by the settings cards and scrolling pages.
 */
export function FieldSelect({
    id,
    value,
    options,
    onChange,
    blocked = false,
    disabled = false,
    size = "default",
    "aria-label": ariaLabel,
    "aria-describedby": ariaDescribedBy,
    "aria-invalid": ariaInvalid,
    "data-test-value": dataTestValue,
}: FieldSelectProps): JSX.Element {
    const [open, setOpen] = useState(false);
    const locked = blocked || disabled;
    const selected = options.find((entry) => entry.value === value);
    // The size class only changes the trigger width; `.field` always supplies the
    // positioning context for the chevron overlay.
    const fieldClass =
        size === "fill"
            ? [styles["field"], styles["fieldFill"]].join(" ")
            : styles["field"];

    // A save can lock an already-open field. Reset during this component's render
    // so the portal closes immediately and cannot reappear when the save ends.
    if (locked && open) setOpen(false);

    return (
        <div className={fieldClass} data-field-select="">
            <Select.Root<string>
                value={value}
                items={options}
                disabled={disabled}
                readOnly={blocked}
                open={open && !locked}
                modal={false}
                onOpenChange={(nextOpen) => {
                    setOpen(nextOpen && !locked);
                }}
                onValueChange={(nextValue) => {
                    if (!locked && nextValue !== null) onChange(nextValue);
                }}
            >
                <Select.Trigger
                    id={id}
                    className={styles["trigger"]}
                    aria-label={ariaLabel}
                    aria-disabled={blocked || undefined}
                    aria-describedby={ariaDescribedBy}
                    aria-invalid={ariaInvalid}
                    data-value={value}
                    data-test-value={dataTestValue ?? value}
                >
                    {selected?.leading}
                    <Select.Value className={styles["label"]} />
                </Select.Trigger>
                <Select.Portal>
                    <Select.Positioner
                        className={styles["positioner"]}
                        alignItemWithTrigger={false}
                        sideOffset={4}
                    >
                        <Select.Popup className={styles["popup"]}>
                            <Select.List className={styles["list"]}>
                                {options.map((entry) => (
                                    <Select.Item
                                        key={entry.value}
                                        value={entry.value}
                                        className={styles["option"]}
                                        data-value={entry.value}
                                        lang={entry.optionLang}
                                    >
                                        {entry.leading}
                                        <Select.ItemText
                                            className={styles["optionText"]}
                                        >
                                            {entry.label}
                                        </Select.ItemText>
                                    </Select.Item>
                                ))}
                            </Select.List>
                        </Select.Popup>
                    </Select.Positioner>
                </Select.Portal>
            </Select.Root>
        </div>
    );
}
