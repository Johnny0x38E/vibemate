import { useId, type JSX } from "react";
import { useTranslation } from "react-i18next";
import styles from "./RelationshipOverview.module.css";

/**
 * Show the approved configuration relationships without implying live connections.
 * Every integration is planned; the reserved statistics area contains no fake data.
 */
export function RelationshipOverview(): JSX.Element {
    const { t } = useTranslation();
    const headingId = useId();
    const statisticsId = useId();
    return (
        <section aria-labelledby={headingId}>
            <div className={styles["heading"]}>
                <h1 id={headingId}>{t("desktop.overview.title")}</h1>
                <span>{t("desktop.overview.state")}</span>
            </div>
            <figure
                className={styles["relationship"]}
                aria-label={t("desktop.overview.diagram")}
            >
                <ul
                    className={styles["providers"]}
                    aria-label={t("desktop.overview.providers")}
                >
                    {["Command Code GOAT", "DeepSeek", "OpenRouter"].map(
                        (name) => (
                            <li className={styles["node"]} key={name}>
                                <span className={styles["name"]}>{name}</span>
                                <span className={styles["state"]}>
                                    {t("app.planned")}
                                </span>
                            </li>
                        ),
                    )}
                </ul>
                <svg
                    className={styles["connections"]}
                    viewBox="0 0 300 34"
                    preserveAspectRatio="none"
                    aria-hidden="true"
                >
                    <path d="M50 0V17H150V34 M150 0V34 M250 0V17H150" />
                </svg>
                <div className={styles["shared"]}>
                    <div className={styles["node"]}>
                        <span className={styles["name"]}>
                            {t("desktop.nav.skills")}
                        </span>
                        <span className={styles["state"]}>
                            {t("app.planned")}
                        </span>
                    </div>
                    <div className={styles["applicationPoint"]}>
                        {t("desktop.overview.applicationPoint")}
                    </div>
                    <div className={styles["node"]}>
                        <span className={styles["name"]}>
                            {t("desktop.nav.mcp")}
                        </span>
                        <span className={styles["state"]}>
                            {t("app.planned")}
                        </span>
                    </div>
                </div>
                <svg
                    className={styles["connections"]}
                    viewBox="0 0 300 34"
                    preserveAspectRatio="none"
                    aria-hidden="true"
                >
                    <path d="M150 0V17H80V34 M150 17H220V34" />
                </svg>
                <ul
                    className={styles["agents"]}
                    aria-label={t("desktop.overview.agents")}
                >
                    {["Pi", "Grok Build"].map((name) => (
                        <li className={styles["node"]} key={name}>
                            <span className={styles["name"]}>{name}</span>
                            <span className={styles["state"]}>
                                {t("app.planned")}
                            </span>
                        </li>
                    ))}
                </ul>
                <figcaption className={styles["caption"]}>
                    {t("desktop.overview.caption")}
                </figcaption>
            </figure>
            <section
                className={styles["statistics"]}
                aria-labelledby={statisticsId}
            >
                <div className={styles["heading"]}>
                    <h2 id={statisticsId}>
                        {t("desktop.overview.statistics")}
                    </h2>
                    <span>{t("desktop.overview.reserved")}</span>
                </div>
                <div className={styles["reserved"]}>
                    <p>{t("desktop.overview.statisticsEmpty")}</p>
                    <p>{t("desktop.overview.statisticsDetail")}</p>
                </div>
            </section>
        </section>
    );
}
