import { useId, type JSX } from "react";
import { useTranslation } from "react-i18next";
import { PageModuleHeader } from "../../components/PageModuleHeader";
import styles from "./OverviewView.module.css";

/**
 * The home destination. The configuration relationship diagram and the reserved
 * statistics area were removed until real overview content exists; this
 * placeholder empty state takes their place. It displays no data at all, so
 * nothing on this page can imply a connected integration or a statistic.
 */
export function OverviewView(): JSX.Element {
    const { t } = useTranslation();
    const headingId = useId();
    return (
        <section aria-labelledby={headingId}>
            <PageModuleHeader
                className={styles["pageHeader"] ?? ""}
                icon="home"
                title={t("desktop.nav.overview")}
                headingId={headingId}
                trailing={
                    <span className={styles["state"]}>
                        {t("desktop.overview.state")}
                    </span>
                }
            />
            <div className={styles["empty"]}>
                {/* A neutral placeholder drawing: a blank panel with content bars
                and one dashed accent. Decorative, so screen readers get the
                written empty state below instead. */}
                <svg
                    className={styles["illustration"]}
                    viewBox="0 0 160 120"
                    aria-hidden="true"
                >
                    <rect
                        className={styles["frame"]}
                        x="26"
                        y="18"
                        width="108"
                        height="84"
                        rx="12"
                    />
                    <circle className={styles["dot"]} cx="46" cy="40" r="5" />
                    <rect
                        className={styles["bar"]}
                        x="58"
                        y="35"
                        width="40"
                        height="10"
                        rx="5"
                    />
                    <rect
                        className={styles["bar"]}
                        x="42"
                        y="58"
                        width="76"
                        height="9"
                        rx="4.5"
                    />
                    <rect
                        className={styles["bar"]}
                        x="42"
                        y="75"
                        width="52"
                        height="9"
                        rx="4.5"
                    />
                    <circle
                        className={styles["accent"]}
                        cx="128"
                        cy="26"
                        r="11"
                    />
                </svg>
                <p className={styles["emptyTitle"]}>
                    {t("desktop.overview.emptyTitle")}
                </p>
                <p className={styles["emptyDetail"]}>
                    {t("desktop.overview.emptyDetail")}
                </p>
            </div>
        </section>
    );
}
