import {
    useRef,
    useState,
    type JSX,
    type KeyboardEvent,
    type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { AboutPanel } from "./AboutPanel";
import { LogSettings } from "./LogSettings";
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
    const [aboutVisited, setAboutVisited] = useState(false);
    const generalTab = useRef<HTMLButtonElement>(null);
    const aboutTab = useRef<HTMLButtonElement>(null);

    function selectTab(next: SettingsTab): void {
        setTab(next);
        if (next === "about") setAboutVisited(true);
    }

    function handleTabKey(event: KeyboardEvent<HTMLButtonElement>): void {
        let next: SettingsTab;
        switch (event.key) {
            case "ArrowLeft":
            case "ArrowRight":
                next = tab === "general" ? "about" : "general";
                break;
            case "Home":
                next = "general";
                break;
            case "End":
                next = "about";
                break;
            default:
                return;
        }
        event.preventDefault();
        selectTab(next);
        // Roving tab focus keeps one stop in the tablist; arrow keys select and
        // focus the other native button without changing preference controls.
        (next === "general" ? generalTab : aboutTab).current?.focus();
    }

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
                    onKeyDown={handleTabKey}
                    aria-selected={tab === "general"}
                    ref={generalTab}
                    id="settings-tab-general"
                    aria-controls="settings-panel-general"
                    tabIndex={tab === "general" ? 0 : -1}
                    onClick={() => {
                        selectTab("general");
                    }}
                >
                    {t("settings.tabs.general")}
                </button>
                <button
                    type="button"
                    className={styles["tab"]}
                    role="tab"
                    onKeyDown={handleTabKey}
                    aria-selected={tab === "about"}
                    ref={aboutTab}
                    id="settings-tab-about"
                    aria-controls="settings-panel-about"
                    tabIndex={tab === "about" ? 0 : -1}
                    onClick={() => {
                        selectTab("about");
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
                <LogSettings />
            </div>
            <div
                className={styles["panel"]}
                role="tabpanel"
                id="settings-panel-about"
                aria-labelledby="settings-tab-about"
                hidden={tab !== "about"}
            >
                {/* Read on first visit, then preserve the result across tab and
                    sidebar navigation without repeating metadata requests. */}
                {aboutVisited && <AboutPanel />}
            </div>
        </section>
    );
}
