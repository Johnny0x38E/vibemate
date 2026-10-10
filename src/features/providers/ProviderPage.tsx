import { useEffect, useRef, type JSX, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Icon } from "../../components/Icon";
import buttons from "./providerButtons.module.css";
import styles from "./ProviderPage.module.css";

/** Inputs for {@link ProviderPage}. */
export interface ProviderPageProps {
    title: ReactNode;
    /** Return to the provider list; the caller restores focus there. */
    onBack: () => void;
    /** While a save is pending, leaving would hide its outcome. */
    backBlocked?: boolean;
    children: ReactNode;
}

/**
 * Shell for a secondary Providers page (new, edit): a back
 * control and the page title on top. The title takes focus when the page
 * opens, so keyboard and screen reader users land at its start. Callers key
 * this component by page so each page change counts as a fresh open.
 */
export function ProviderPage({
    title,
    onBack,
    backBlocked = false,
    children,
}: ProviderPageProps): JSX.Element {
    const { t } = useTranslation();
    const heading = useRef<HTMLHeadingElement>(null);

    useEffect(() => {
        heading.current?.focus();
    }, []);

    return (
        <div className={styles["page"]}>
            <div className={styles["header"]}>
                <button
                    className={[
                        buttons["secondary"],
                        buttons["withIcon"],
                    ]
                        .filter((value): value is string => value !== undefined)
                        .join(" ")}
                    type="button"
                    aria-disabled={backBlocked}
                    // The fuller name starts with the visible text (label in name).
                    aria-label={t("providers.backLabel")}
                    onClick={() => {
                        if (!backBlocked) onBack();
                    }}
                >
                    {/* Decorative: the label carries the meaning. */}
                    <Icon name="back" />
                    {t("providers.back")}
                </button>
                <h1 className={styles["title"]} ref={heading} tabIndex={-1}>
                    {title}
                </h1>
            </div>
            {children}
        </div>
    );
}
