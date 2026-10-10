import {
    useEffect,
    useId,
    useRef,
    useState,
    type JSX,
    type KeyboardEvent,
    type ReactNode,
} from "react";
import fieldStyles from "../features/settings/settingsField.module.css";
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
    "aria-describedby"?: string | undefined;
    "aria-invalid"?: boolean | undefined;
    /** Optional test hook (for example provider kind). */
    "data-test-value"?: string;
}

function isInteractionBlocked(blocked: boolean, disabled: boolean): boolean {
    return blocked || disabled;
}

/**
 * Styled single-choice dropdown shared by settings and provider forms.
 * Native `<select>` cannot style its menu or show icons in options, so this
 * combobox reuses the settings field width and chevron while drawing its own
 * list panel.
 */
export function FieldSelect({
    id,
    value,
    options,
    onChange,
    blocked = false,
    disabled = false,
    "aria-describedby": ariaDescribedBy,
    "aria-invalid": ariaInvalid,
    "data-test-value": dataTestValue,
}: FieldSelectProps): JSX.Element {
    const listId = useId();
    const rootRef = useRef<HTMLDivElement>(null);
    const [open, setOpen] = useState(false);
    const [activeIndex, setActiveIndex] = useState(0);

    const selected =
        options.find((entry) => entry.value === value) ?? options[0];
    const locked = isInteractionBlocked(blocked, disabled);

    function showList(): void {
        const index = options.findIndex((entry) => entry.value === value);
        setActiveIndex(index >= 0 ? index : 0);
        setOpen(true);
    }

    useEffect(() => {
        if (!open) return;
        function onPointerDown(event: MouseEvent): void {
            const root = rootRef.current;
            if (root && !root.contains(event.target as Node)) setOpen(false);
        }
        document.addEventListener("mousedown", onPointerDown);
        return () => {
            document.removeEventListener("mousedown", onPointerDown);
        };
    }, [open]);

    function pick(next: string): void {
        onChange(next);
        setOpen(false);
    }

    function onTriggerKeyDown(event: KeyboardEvent<HTMLButtonElement>): void {
        if (locked) return;
        switch (event.key) {
            case "ArrowDown":
            case "Enter":
            case " ":
                event.preventDefault();
                showList();
                break;
            case "Escape":
                if (open) {
                    event.preventDefault();
                    setOpen(false);
                }
                break;
            default:
                break;
        }
    }

    function onListKeyDown(event: KeyboardEvent<HTMLUListElement>): void {
        switch (event.key) {
            case "ArrowDown":
                event.preventDefault();
                setActiveIndex((index) =>
                    Math.min(index + 1, options.length - 1),
                );
                break;
            case "ArrowUp":
                event.preventDefault();
                setActiveIndex((index) => Math.max(index - 1, 0));
                break;
            case "Enter":
            case " ":
                event.preventDefault();
                pick(options[activeIndex]?.value ?? value);
                break;
            case "Escape":
                event.preventDefault();
                setOpen(false);
                break;
            case "Tab":
                setOpen(false);
                break;
            default:
                break;
        }
    }

    return (
        <div
            className={fieldStyles["field"]}
            ref={rootRef}
            data-field-select=""
        >
            <div className={styles["root"]}>
                <button
                    type="button"
                    id={id}
                    role="combobox"
                    className={styles["trigger"]}
                    aria-expanded={open}
                    aria-haspopup="listbox"
                    aria-controls={listId}
                    aria-disabled={blocked || undefined}
                    disabled={disabled}
                    aria-describedby={ariaDescribedBy}
                    aria-invalid={ariaInvalid}
                    data-value={value}
                    data-test-value={dataTestValue ?? value}
                    onClick={() => {
                        if (locked) return;
                        if (open) setOpen(false);
                        else showList();
                    }}
                    onKeyDown={onTriggerKeyDown}
                >
                    {selected !== undefined && (
                        <>
                            {selected.leading}
                            <span className={styles["label"]}>
                                {selected.label}
                            </span>
                        </>
                    )}
                </button>
                {open && (
                    <ul
                        className={styles["list"]}
                        id={listId}
                        role="listbox"
                        aria-labelledby={id}
                        tabIndex={-1}
                        onKeyDown={onListKeyDown}
                    >
                        {options.map((entry, index) => (
                            <li
                                key={entry.value}
                                role="option"
                                className={styles["option"]}
                                aria-selected={entry.value === value}
                                data-active={index === activeIndex}
                                data-value={entry.value}
                                lang={entry.optionLang}
                                onMouseEnter={() => {
                                    setActiveIndex(index);
                                }}
                                onMouseDown={(event) => {
                                    // Keep focus on the combobox; avoid blur before click.
                                    event.preventDefault();
                                }}
                                onKeyDown={(event) => {
                                    if (
                                        event.key === "Enter" ||
                                        event.key === " "
                                    ) {
                                        event.preventDefault();
                                        pick(entry.value);
                                    }
                                }}
                                onClick={() => {
                                    pick(entry.value);
                                }}
                            >
                                {entry.leading}
                                <span>{entry.label}</span>
                            </li>
                        ))}
                    </ul>
                )}
            </div>
        </div>
    );
}
