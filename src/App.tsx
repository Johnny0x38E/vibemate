import { useId, useRef, useState, type JSX, type ReactNode } from "react";
import type { ModuleNavigationHandle } from "./components/moduleNavigation";
import { useTranslation } from "react-i18next";
import { BrandLogo } from "./components/BrandLogo";
import { Icon } from "./components/Icon";
import { NotificationProvider } from "./components/Notifications";
import { TitlebarChrome } from "./components/TitlebarChrome";
import { WindowDragRegion } from "./components/WindowDragRegion";
import { OverviewView } from "./features/overview/OverviewView";
import { AgentList } from "./features/agents/AgentList";
import { McpView } from "./features/mcp/McpView";
import { ProvidersView } from "./features/providers/ProvidersView";
import { SettingsView } from "./features/settings/SettingsView";
import { SkillLibrary } from "./features/skills/SkillLibrary";
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
 * Feature modules mount on first visit and keep cached data while hidden.
 * Sidebar activation explicitly opens the destination home through its guards,
 * rather than restoring its previously open editor or detail page.
 */
export default function App({ languageSettings }: AppProps): JSX.Element {
    const { t } = useTranslation();
    const [page, setPage] = useState<Page>("home");
    const [collapsed, setCollapsed] = useState(false);
    const [providersVisited, setProvidersVisited] = useState(false);
    const [mcpVisited, setMcpVisited] = useState(false);
    const [agentsVisited, setAgentsVisited] = useState(false);
    const [skillsVisited, setSkillsVisited] = useState(false);
    const sidebarId = useId();
    const providersNavigation = useRef<ModuleNavigationHandle>(null);
    const mcpNavigation = useRef<ModuleNavigationHandle>(null);
    const agentsNavigation = useRef<ModuleNavigationHandle>(null);
    const settingsNavigation = useRef<ModuleNavigationHandle>(null);
    const skillsNavigation = useRef<ModuleNavigationHandle>(null);

    function navigate(destination: Page): void {
        const target = {
            home: null,
            skills: skillsNavigation.current,
            providers: providersNavigation.current,
            mcp: mcpNavigation.current,
            agents: agentsNavigation.current,
            settings: settingsNavigation.current,
        }[destination];
        // Ask before revealing/resetting a cached module: its own guards may
        // reject a pending write or let the user cancel discarding model drafts.
        if (target?.openHome() === false) return;
        setPage(destination);
        if (destination === "providers") setProvidersVisited(true);
        if (destination === "mcp") setMcpVisited(true);
        if (destination === "agents") setAgentsVisited(true);
        if (destination === "skills") setSkillsVisited(true);
    }

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
                    navigate(destination);
                }}
            >
                <Icon name={destination} />
                <span className={styles["navigationText"]}>{label}</span>
            </button>
        );
    }

    return (
        // App-level so a notification survives page and view changes.
        <NotificationProvider>
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
                        aria-label={t("desktop.brandHome", {
                            name: "vibemate",
                        })}
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
                    {page === "home" && (
                        <OverviewView
                            onNavigate={(destination) => {
                                navigate(destination);
                            }}
                        />
                    )}
                    {/* Hide instead of remounting to preserve pending work and caches.
                    Sidebar re-entry asks the destination to return home safely. */}
                    {agentsVisited && (
                        <AgentList
                            hidden={page !== "agents"}
                            homeRef={agentsNavigation}
                            onOpenSkills={() => {
                                navigate("skills");
                            }}
                            onOpenMcp={() => {
                                navigate("mcp");
                            }}
                        />
                    )}
                    {skillsVisited && (
                        <SkillLibrary
                            hidden={page !== "skills"}
                            homeRef={skillsNavigation}
                        />
                    )}
                    {mcpVisited && (
                        <McpView
                            hidden={page !== "mcp"}
                            homeRef={mcpNavigation}
                        />
                    )}
                    {providersVisited && (
                        <ProvidersView
                            hidden={page !== "providers"}
                            homeRef={providersNavigation}
                        />
                    )}
                    {/* Keep the real control alive when leaving Settings. Unmounting here
                    would discard a committed choice or an unknown save outcome. */}
                    <SettingsView
                        languageSettings={languageSettings}
                        homeRef={settingsNavigation}
                        hidden={page !== "settings"}
                    />
                </main>
            </div>
        </NotificationProvider>
    );
}
