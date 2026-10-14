import { Select } from "@base-ui/react/select";
import { useEffect, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import { EFFORT_LEVELS, type EffortLevel } from "../../lib/desktop/models";
import styles from "./EffortLevelSelect.module.css";

/** Editor order: off first, the largest effort last. */
export const EFFORT_CHOICES: readonly EffortLevel[] = [
    ...EFFORT_LEVELS,
].reverse();

/**
 * Multi-select for the reasoning levels a model declares.
 * The menu stays open after each choice so several levels can be toggled.
 * It is portaled above the model dialog, which already occupies the dialog layer.
 */
export function EffortLevelSelect({
    id,
    value,
    disabled,
    onChange,
    onOpenChange,
}: {
    id: string;
    value: readonly EffortLevel[];
    disabled: boolean;
    onChange: (levels: EffortLevel[]) => void;
    /** Reports whether the menu is open, so Escape can close it before the dialog. */
    onOpenChange: (open: boolean) => void;
}): JSX.Element {
    const { t } = useTranslation();
    const [open, setOpen] = useState(false);
    const items = EFFORT_CHOICES.map((level) => ({
        value: level,
        label: level,
    }));

    // A locked dialog must not leave the menu open. Closing during render matches
    // FieldSelect, so the portal cannot linger until an effect runs.
    if (disabled && open) setOpen(false);

    useEffect(() => {
        onOpenChange(open && !disabled);
        return () => {
            onOpenChange(false);
        };
    }, [open, disabled, onOpenChange]);

    const summary = EFFORT_CHOICES.filter((level) => value.includes(level));

    function removeLevel(level: EffortLevel): void {
        if (disabled) return;
        onChange(summary.filter((item) => item !== level));
    }

    return (
        <div className={styles["field"]} data-disabled={disabled || undefined}>
            {/* Chips sit above the trigger, not inside it: the trigger is a
                button, and a remove button cannot be nested there. Only the
                remove control takes its own clicks; the rest of a chip lets
                them fall through to the trigger, which opens the menu. */}
            {summary.map((level) => (
                <span key={level} className={styles["chip"]}>
                    <span>{level}</span>
                    <button
                        type="button"
                        className={styles["remove"]}
                        aria-label={t(
                            "providers.models.parameters.removeLevel",
                            {
                                level,
                            },
                        )}
                        disabled={disabled}
                        onClick={() => {
                            removeLevel(level);
                        }}
                    >
                        <span aria-hidden="true">×</span>
                    </button>
                </span>
            ))}
            <Select.Root<EffortLevel, true>
                multiple
                value={[...value]}
                items={items}
                modal={false}
                open={open && !disabled}
                onOpenChange={(nextOpen) => {
                    setOpen(nextOpen && !disabled);
                }}
                onValueChange={(next) => {
                    if (disabled) return;
                    onChange(
                        EFFORT_CHOICES.filter((level) => next.includes(level)),
                    );
                }}
            >
                <Select.Trigger
                    id={id}
                    className={styles["trigger"]}
                    aria-disabled={disabled || undefined}
                    data-empty={summary.length === 0 || undefined}
                >
                    {summary.length === 0 ? (
                        <span className={styles["placeholder"]}>
                            {t("providers.models.parameters.unset")}
                        </span>
                    ) : null}
                </Select.Trigger>
                <Select.Portal>
                    <Select.Positioner
                        className={styles["positioner"]}
                        alignItemWithTrigger={false}
                        sideOffset={4}
                    >
                        <Select.Popup className={styles["popup"]}>
                            <Select.List className={styles["list"]}>
                                {EFFORT_CHOICES.map((level) => (
                                    <Select.Item
                                        key={level}
                                        value={level}
                                        label={level}
                                        className={styles["option"]}
                                        data-value={level}
                                    >
                                        <Select.ItemIndicator
                                            className={styles["mark"]}
                                            keepMounted
                                            aria-hidden="true"
                                        >
                                            <svg
                                                className={styles["check"]}
                                                viewBox="0 0 24 24"
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
                                        </Select.ItemIndicator>
                                        <Select.ItemText>
                                            {level}
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
