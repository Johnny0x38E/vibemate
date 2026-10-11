import type { JSX, ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Icon } from "./Icon";
import styles from "./SecondaryPageFooter.module.css";

/** Page-level navigation, feedback and actions, separate from scrolling content. */
export interface SecondaryPageFooterProps {
    onBack?: (() => void) | undefined;
    backBlocked?: boolean;
    backAriaLabel?: string | undefined;
    feedback?: ReactNode;
    children?: ReactNode;
}

/** Keep Back at the start and submission actions at the end without replacing focusable controls. */
export function SecondaryPageFooter({
    onBack,
    backBlocked = false,
    backAriaLabel,
    feedback,
    children,
}: SecondaryPageFooterProps): JSX.Element {
    const { t } = useTranslation();
    return (
        <footer className={styles["footer"]}>
            {feedback && <div className={styles["feedback"]}>{feedback}</div>}
            <div className={styles["row"]}>
                {onBack && (
                    <button
                        className={styles["back"]}
                        type="button"
                        aria-disabled={backBlocked}
                        aria-label={backAriaLabel ?? t("providers.backLabel")}
                        onClick={() => {
                            if (!backBlocked) onBack();
                        }}
                    >
                        <Icon name="back" />
                        {t("providers.back")}
                    </button>
                )}
                <div className={styles["actions"]}>{children}</div>
            </div>
        </footer>
    );
}
