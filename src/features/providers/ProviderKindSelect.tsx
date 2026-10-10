import {
    useEffect,
    useId,
    useRef,
    useState,
    type JSX,
    type KeyboardEvent,
} from "react";
import type { ProviderTemplate } from "../../lib/desktop/providers";
import { ProviderIcon } from "./ProviderIcon";
import fieldStyles from "../settings/settingsField.module.css";
import styles from "./ProviderKindSelect.module.css";

export interface ProviderKindSelectProps {
    id: string;
    templates: readonly ProviderTemplate[];
    value: string;
    blocked?: boolean;
    onChange: (kind: string) => void;
}

/**
 * Provider picker for create mode. Native `<select>` cannot show brand icons in
 * its menu, so this combobox lists each template with its official logo and name.
 */
export function ProviderKindSelect({
    id,
    templates,
    value,
    blocked = false,
    onChange,
}: ProviderKindSelectProps): JSX.Element {
    const listId = useId();
    const rootRef = useRef<HTMLDivElement>(null);
    const [open, setOpen] = useState(false);
    const [activeIndex, setActiveIndex] = useState(0);

    const selected =
        templates.find((entry) => entry.kind === value) ?? templates[0];

    useEffect(() => {
        if (!open) return;
        const index = templates.findIndex((entry) => entry.kind === value);
        setActiveIndex(index >= 0 ? index : 0);
    }, [open, templates, value]);

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

    function pick(kind: string): void {
        onChange(kind);
        setOpen(false);
    }

    function onTriggerKeyDown(event: KeyboardEvent<HTMLButtonElement>): void {
        if (blocked) return;
        switch (event.key) {
            case "ArrowDown":
            case "Enter":
            case " ":
                event.preventDefault();
                setOpen(true);
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
                    Math.min(index + 1, templates.length - 1),
                );
                break;
            case "ArrowUp":
                event.preventDefault();
                setActiveIndex((index) => Math.max(index - 1, 0));
                break;
            case "Enter":
            case " ":
                event.preventDefault();
                pick(templates[activeIndex]?.kind ?? value);
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
        <div className={fieldStyles["field"]} ref={rootRef}>
            <div className={styles["root"]}>
                <button
                    type="button"
                    id={id}
                    role="combobox"
                    className={styles["trigger"]}
                    aria-expanded={open}
                    aria-haspopup="listbox"
                    aria-controls={listId}
                    aria-disabled={blocked}
                    data-kind={value}
                    onClick={() => {
                        if (!blocked) setOpen((previous) => !previous);
                    }}
                    onKeyDown={onTriggerKeyDown}
                >
                    {selected !== undefined && (
                        <>
                            <ProviderIcon kind={selected.kind} />
                            <span className={styles["label"]}>
                                {selected.brandName}
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
                        {templates.map((entry, index) => (
                            <li
                                key={entry.kind}
                                role="option"
                                className={styles["option"]}
                                aria-selected={entry.kind === value}
                                data-active={index === activeIndex}
                                onMouseEnter={() => {
                                    setActiveIndex(index);
                                }}
                                onMouseDown={(event) => {
                                    // Keep focus on the combobox; avoid blur before click.
                                    event.preventDefault();
                                }}
                                onClick={() => {
                                    pick(entry.kind);
                                }}
                            >
                                <ProviderIcon kind={entry.kind} />
                                <span>{entry.brandName}</span>
                            </li>
                        ))}
                    </ul>
                )}
            </div>
        </div>
    );
}
