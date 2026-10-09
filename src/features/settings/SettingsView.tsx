import { useState, type JSX, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import styles from "./SettingsView.module.css";

type SettingsTab = "general" | "about";

interface SettingsViewProps {
    languageSettings: ReactNode;
    hidden?: boolean;
}

/**
 * Settings area with top tabs. Preference controls stay mounted while hidden so
 * pending saves and in-progress input survive tab switches.
 */
export function SettingsView({
    languageSettings,
    hidden = false,
}: SettingsViewProps): JSX.Element {
    const { t } = useTranslation();
    const [tab, setTab] = useState<SettingsTab>("general");

    return (
        <section className={styles["settings"]} hidden={hidden}>
            <h1 className={styles["title"]}>{t("desktop.nav.settings")}</h1>
            <div
                className={styles["tabs"]}
                role="tablist"
                aria-label={t("settings.tabs.ariaLabel")}
            >
                <button
                    type="button"
                    className={styles["tab"]}
                    role="tab"
                    aria-selected={tab === "general"}
                    id="settings-tab-general"
                    aria-controls="settings-panel-general"
                    tabIndex={tab === "general" ? 0 : -1}
                    onClick={() => {
                        setTab("general");
                    }}
                >
                    {t("settings.tabs.general")}
                </button>
                <button
                    type="button"
                    className={styles["tab"]}
                    role="tab"
                    aria-selected={tab === "about"}
                    id="settings-tab-about"
                    aria-controls="settings-panel-about"
                    tabIndex={tab === "about" ? 0 : -1}
                    onClick={() => {
                        setTab("about");
                    }}
                >
                    {t("settings.tabs.about")}
                </button>
            </div>
            <div
                className={styles["panel"]}
                role="tabpanel"
                id="settings-panel-general"
                aria-labelledby="settings-tab-general"
                hidden={tab !== "general"}
            >
                {languageSettings}
            </div>
            <div
                className={styles["panel"]}
                role="tabpanel"
                id="settings-panel-about"
                aria-labelledby="settings-tab-about"
                hidden={tab !== "about"}
            >
                <div className={styles["aboutPlaceholder"]}>
                    <h2 className={styles["aboutHeading"]}>
                        {t("desktop.plannedTitle")}
                    </h2>
                    <p className={styles["aboutDetail"]}>
                        {t("desktop.plannedDetail")}
                    </p>
                </div>
            </div>
        </section>
    );
}
