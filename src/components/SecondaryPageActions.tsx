import {
    createContext,
    useContext,
    useLayoutEffect,
    useMemo,
    useState,
    type JSX,
    type ReactNode,
} from "react";

/** Lets a secondary page host trailing header actions from nested editors. */
export interface SecondaryPageActionsHost {
    setActions: (node: ReactNode) => void;
}

const SecondaryPageActionsContext =
    createContext<SecondaryPageActionsHost | null>(null);

/**
 * Owns the trailing header slot for {@link ProviderPage}. Nested editors call
 * {@link SecondaryPageActions} while they are the active panel.
 */
export function SecondaryPageActionsProvider({
    children,
}: {
    children: (actions: ReactNode) => ReactNode;
}): JSX.Element {
    const [actions, setActions] = useState<ReactNode>(null);
    const host = useMemo(() => ({ setActions }), []);
    return (
        <SecondaryPageActionsContext.Provider value={host}>
            {children(actions)}
        </SecondaryPageActionsContext.Provider>
    );
}

/**
 * Publishes primary page actions (usually Save) into the secondary-page header.
 * Without a page host (standalone tests), actions render in place instead.
 */
export function SecondaryPageActions({
    children,
    active = true,
}: {
    children: ReactNode;
    /** False while this panel is hidden or inert (for example an inactive tab). */
    active?: boolean;
}): JSX.Element | null {
    const host = useContext(SecondaryPageActionsContext);
    useLayoutEffect(() => {
        // When inactive, do not clear here: a sibling may already own the slot.
        // Cleanup from the previous active publication removes this panel's actions.
        if (host === null || !active) return;
        host.setActions(children);
        return () => {
            host.setActions(null);
        };
    }, [host, active, children]);
    if (host === null && active) return <>{children}</>;
    return null;
}
