import { useId, useState, type JSX, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { BrandLogo } from "./components/BrandLogo";
import { Icon } from "./components/Icon";
import { TitlebarChrome } from "./components/TitlebarChrome";
import { WindowDragRegion } from "./components/WindowDragRegion";
import { RelationshipOverview } from "./features/overview/RelationshipOverview";
import { SettingsView } from "./features/settings/SettingsView";
import styles from "./App.module.css";
import "./App.css";

type Page = "home" | "providers" | "agents" | "skills" | "mcp" | "settings";

interface AppProps {
    languageSettings: ReactNode;
}

/**
 * Compose the approved desktop shell around a caller-supplied language control.
 * Navigation is window-local; Rust persists appearance. Settings stays mounted while hidden
 * so pending saves, uncertain outcomes, and unsubmitted input survive navigation.
 */
export default function App({ languageSettings }: AppProps): JSX.Element {
    const { t } = useTranslation();
    const [page, setPage] = useState<Page>("home");
    const [collapsed, setCollapsed] = useState(false);
    const sidebarId = useId();

    const pageLabel =
        page === "home" ? t("desktop.home") : t(`desktop.nav.${page}`);
    const collapseAction = t(collapsed ? "desktop.expand" : "desktop.collapse");

    function navigationButton(destination: Page): JSX.Element {
        const label =
            destination === "home"
                ? t("desktop.nav.overview")
                : t(`desktop.nav.${destination}`);
        return (
            <button
                className={styles["navigationButton"]}
                type="button"
                key={destination}
                aria-label={label}
                aria-current={page === destination ? "page" : undefined}
                title={label}
                onClick={() => {
                    setPage(destination);
                }}
            >
                <Icon name={destination} />
                <span className={styles["navigationText"]}>{label}</span>
            </button>
        );
    }

    return (
        <div className={styles["shell"]} data-collapsed={collapsed}>
            <aside
                id={sidebarId}
                className={styles["sidebar"]}
                aria-label={t("desktop.sidebar")}
            >
                {/* On macOS the native traffic lights sit in this reserved strip. It is
                    also the sidebar's drag area, so it must never hold a control. */}
                <WindowDragRegion className={styles["windowSpace"]} />
                <button
                    className={styles["brand"]}
                    type="button"
                    aria-label={t("desktop.brandHome", { name: "vibemate" })}
                    title={t("desktop.brandHome", { name: "vibemate" })}
                    onClick={() => {
                        setPage("home");
                    }}
                >
                    {/* The logo is decorative; the label starts with the visible brand
                        name so voice control can target the button by what it shows. */}
                    <BrandLogo
                        collapsed={collapsed}
                        className={styles["brandLogo"]}
                    />
                </button>
                <nav
                    className={styles["navigation"]}
                    aria-label={t("desktop.navigation")}
                >
                    {(
                        [
                            "home",
                            "providers",
                            "agents",
                            "skills",
                            "mcp",
                        ] as const
                    ).map(navigationButton)}
                </nav>
                {/* Settings and the toggle share one row while expanded. The collapsed
                    rail stacks them, showing the toggle above Settings. */}
                <div className={styles["sidebarBottom"]}>
                    {navigationButton("settings")}
                    <button
                        className={styles["collapse"]}
                        type="button"
                        aria-label={collapseAction}
                        title={collapseAction}
                        aria-expanded={!collapsed}
                        aria-controls={sidebarId}
                        onClick={() => {
                            setCollapsed((previous) => !previous);
                        }}
                    >
                        <Icon name="collapse" />
                    </button>
                </div>
            </aside>
            {/* A narrow unpainted strip above the scrolling content keeps the top edge
                draggable without a visible title bar. It stays outside <main> so
                scrolled content can never slide underneath it. */}
            <TitlebarChrome
                className={styles["titlebarRow"]}
                dragClassName={styles["titlebarDrag"]}
            />
            <main className={styles["content"]} aria-label={pageLabel}>
                {page === "home" && <RelationshipOverview />}
                {page !== "home" && page !== "settings" && (
                    <section>
                        <h1 className={styles["pageHeading"]}>{pageLabel}</h1>
                        <div className={styles["planned"]}>
                            <h2>{t("desktop.plannedTitle")}</h2>
                            <p>{t("desktop.plannedDetail")}</p>
                        </div>
                    </section>
                )}
                {/* Keep the real control alive when leaving Settings. Unmounting here
                    would discard a committed choice or an unknown save outcome. */}
                <SettingsView
                    languageSettings={languageSettings}
                    hidden={page !== "settings"}
                />
            </main>
        </div>
    );
}
