import { useEffect, useRef, type JSX, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { IconButton } from "../../components/IconButton";
import { SecondaryPageActionsProvider } from "../../components/SecondaryPageActions";
import styles from "./ProviderPage.module.css";

/** Inputs for {@link ProviderPage}. */
export interface ProviderPageProps {
    title: ReactNode;
    /** Return to the list; the caller restores focus there. */
    onBack: () => void;
    /** While a save is pending, leaving would hide its outcome. */
    backBlocked?: boolean;
    /** Override the back control label when the destination is not Providers. */
    backAriaLabel?: string;
    /** Read-only detail pages delegate scrolling to a single content wrapper. */
    scrollContent?: boolean;
    children: ReactNode;
}

/**
 * Shell for a secondary page: icon-only Back, left-aligned title, and optional
 * trailing actions (usually Save) published by nested editors. The title takes
 * focus when the page opens. Callers key this component by page so each page
 * change counts as a fresh open.
 */
export function ProviderPage({
    title,
    onBack,
    backBlocked = false,
    backAriaLabel,
    children,
    scrollContent = false,
}: ProviderPageProps): JSX.Element {
    const { t } = useTranslation();
    const heading = useRef<HTMLHeadingElement>(null);
    const backLabel = backAriaLabel ?? t("providers.backLabel");

    useEffect(() => {
        heading.current?.focus();
    }, []);

    return (
        <SecondaryPageActionsProvider>
            {(actions) => (
                <div className={styles["page"]}>
                    <div className={styles["header"]}>
                        <IconButton
                            icon="back"
                            label={backLabel}
                            blocked={backBlocked}
                            onClick={onBack}
                        />
                        <h1
                            className={styles["title"]}
                            ref={heading}
                            tabIndex={-1}
                            data-focus="heading"
                        >
                            {title}
                        </h1>
                        <div className={styles["actions"]}>{actions}</div>
                    </div>
                    {scrollContent ? (
                        <div className={styles["body"]}>{children}</div>
                    ) : (
                        children
                    )}
                </div>
            )}
        </SecondaryPageActionsProvider>
    );
}
