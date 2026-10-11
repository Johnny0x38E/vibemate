import { useEffect, useRef, type JSX, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { SecondaryPageFooter } from "../../components/SecondaryPageFooter";
import styles from "./ProviderPage.module.css";

/** Inputs for {@link ProviderPage}. */
export interface ProviderPageProps {
    title: ReactNode;
    /** Return to the provider list; the caller restores focus there. */
    onBack: () => void;
    /** While a save is pending, leaving would hide its outcome. */
    backBlocked?: boolean;
    /** Override the back control `aria-label` when the destination is not Providers. */
    backAriaLabel?: string;
    /** Forms and model panels render their own footer so Back and Save share one row. */
    footerInContent?: boolean;
    /** Read-only detail pages delegate scrolling to a single content wrapper. */
    scrollContent?: boolean;
    children: ReactNode;
}

/**
 * Shell for a secondary page: left-aligned title and fixed bottom navigation.
 * The title takes focus when the page opens. Callers key this component by page
 * so each page change counts as a fresh open.
 */
export function ProviderPage({
    title,
    onBack,
    backBlocked = false,
    backAriaLabel,
    children,
    footerInContent = false,
    scrollContent = false,
}: ProviderPageProps): JSX.Element {
    const { t } = useTranslation();
    const heading = useRef<HTMLHeadingElement>(null);

    useEffect(() => {
        heading.current?.focus();
    }, []);

    return (
        <div className={styles["page"]}>
            <div className={styles["header"]}>
                <h1
                    className={styles["title"]}
                    ref={heading}
                    tabIndex={-1}
                    data-focus="heading"
                >
                    {title}
                </h1>
            </div>
            {scrollContent ? (
                <div className={styles["body"]}>{children}</div>
            ) : (
                children
            )}
            {!footerInContent && (
                <SecondaryPageFooter
                    onBack={onBack}
                    backBlocked={backBlocked}
                    backAriaLabel={backAriaLabel ?? t("providers.backLabel")}
                />
            )}
        </div>
    );
}
