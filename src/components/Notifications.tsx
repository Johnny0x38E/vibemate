import {
    createContext,
    useCallback,
    useContext,
    useEffect,
    useRef,
    useState,
    type JSX,
    type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { Icon } from "./Icon";
import styles from "./Notifications.module.css";

/** How long a notification stays before it dismisses itself, in milliseconds. */
export const NOTIFICATION_DURATION_MS = 3500;

/** Show a short, already translated message; it replaces any current one. */
export type Notify = (message: string) => void;

interface ShownNotification {
    id: number;
    message: string;
}

// Outside a provider (isolated feature tests, previews) notifying does nothing.
const NotifyContext = createContext<Notify>(() => undefined);

/** The `notify` function of the nearest {@link NotificationProvider}. */
export function useNotify(): Notify {
    return useContext(NotifyContext);
}

/**
 * App-level host for brief, non-blocking notifications such as "Saved". It
 * outlives page and view changes, so a feature can notify and then navigate.
 *
 * The message text sits alone in a polite `role="status"` region that is
 * always mounted, so screen readers announce it without focus moving and never
 * read the close button as part of it. The card around it (and its close
 * button) is styled only while a message is shown. It dismisses itself after
 * {@link NOTIFICATION_DURATION_MS}; the timer pauses while the pointer is over
 * the card or focus is inside it, and it can be closed with its button (or
 * Escape while the button has focus). Only one notification is shown at a
 * time; a new one replaces it and restarts the full duration.
 */
export function NotificationProvider({
    children,
}: {
    children: ReactNode;
}): JSX.Element {
    const { t } = useTranslation();
    const [shown, setShown] = useState<ShownNotification | null>(null);
    const [hovered, setHovered] = useState(false);
    const [focused, setFocused] = useState(false);
    const nextId = useRef(0);
    // Time left for the notification with this id; pausing keeps it.
    const remaining = useRef({ id: 0, ms: NOTIFICATION_DURATION_MS });
    // Where focus came from when the user moved into the card, to restore it
    // on close instead of dropping focus to the document body.
    const returnFocus = useRef<HTMLElement | null>(null);
    const id = shown?.id;

    const notify = useCallback<Notify>((message) => {
        nextId.current += 1;
        remaining.current = {
            id: nextId.current,
            ms: NOTIFICATION_DURATION_MS,
        };
        setShown({ id: nextId.current, message });
    }, []);

    const dismiss = useCallback((dismissed: number) => {
        setShown((current) => (current?.id === dismissed ? null : current));
        // The empty card takes no pointer or focus, so no leave or blur
        // event would reset these for the next notification.
        setHovered(false);
        setFocused(false);
    }, []);

    useEffect(() => {
        if (id === undefined || hovered || focused) return;
        const started = Date.now();
        const timer = window.setTimeout(() => {
            dismiss(id);
        }, remaining.current.ms);
        return () => {
            window.clearTimeout(timer);
            // A newer notification has its own full duration already.
            if (remaining.current.id === id)
                remaining.current = {
                    id,
                    ms: Math.max(
                        0,
                        remaining.current.ms - (Date.now() - started),
                    ),
                };
        };
    }, [id, hovered, focused, dismiss]);

    function close(): void {
        if (id === undefined) return;
        const target = returnFocus.current;
        returnFocus.current = null;
        dismiss(id);
        if (target?.isConnected) target.focus();
    }

    return (
        <NotifyContext.Provider value={notify}>
            {children}
            <div className={styles["region"]}>
                <div
                    className={styles["card"]}
                    data-shown={shown ? "" : undefined}
                    onMouseEnter={() => {
                        if (shown) setHovered(true);
                    }}
                    onMouseLeave={() => {
                        setHovered(false);
                    }}
                    onFocus={(event) => {
                        const from = event.relatedTarget;
                        if (
                            !focused &&
                            from instanceof HTMLElement &&
                            !event.currentTarget.contains(from)
                        )
                            returnFocus.current = from;
                        setFocused(true);
                    }}
                    onBlur={(event) => {
                        const to = event.relatedTarget;
                        if (!(
                            to instanceof Node &&
                            event.currentTarget.contains(to)
                        ))
                            setFocused(false);
                    }}
                >
                    {/* Only the text is live; the button stays outside it. */}
                    <p className={styles["message"]} role="status">
                        {shown?.message}
                    </p>
                    {shown && (
                        <button
                            className={styles["close"]}
                            type="button"
                            aria-label={t("notifications.dismiss")}
                            title={t("notifications.dismiss")}
                            onClick={close}
                            // The button is the card's only focusable element.
                            onKeyDown={(event) => {
                                if (event.key === "Escape") close();
                            }}
                        >
                            <Icon name="close" />
                        </button>
                    )}
                </div>
            </div>
        </NotifyContext.Provider>
    );
}
