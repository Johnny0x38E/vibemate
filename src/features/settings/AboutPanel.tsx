import { useEffect, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import {
    getAppInfo,
    openProjectRepository,
    type AppInfo,
} from "../../lib/desktop";
import { BrandLogo } from "../../components/BrandLogo";
import styles from "./AboutPanel.module.css";

type MetadataState =
    | { kind: "loading" }
    | { kind: "ready"; info: AppInfo }
    | { kind: "preview" }
    | { kind: "error" };

/**
 * Read build metadata when first mounted. Retain it across language changes;
 * preview and failed IPC never substitute a guessed desktop version.
 */
export function AboutPanel(): JSX.Element {
    const { t } = useTranslation();
    const [state, setState] = useState<MetadataState>({ kind: "loading" });
    const [attempt, setAttempt] = useState(0);
    const [repositoryState, setRepositoryState] = useState<
        "idle" | "opening" | "error"
    >("idle");

    useEffect(() => {
        // Each effect owns its result, including StrictMode's discarded first
        // request. Cleanup prevents late IPC from replacing the current view.
        let active = true;
        getAppInfo().then(
            (info) => {
                if (active)
                    setState(
                        info === null
                            ? { kind: "preview" }
                            : { kind: "ready", info },
                    );
            },
            () => {
                // Only bundled feedback is rendered; raw runtime errors are private.
                if (active) setState({ kind: "error" });
            },
        );
        return () => {
            active = false;
        };
    }, [attempt]);

    async function openRepository(): Promise<void> {
        setRepositoryState("opening");
        try {
            await openProjectRepository();
            setRepositoryState("idle");
        } catch {
            setRepositoryState("error");
        }
    }

    return (
        <div className={styles["card"]}>
            <header className={styles["identity"]}>
                <h2
                    className={styles["brand"]}
                    aria-label={
                        state.kind === "ready" ? state.info.name : "vibemate"
                    }
                >
                    <BrandLogo collapsed={false} className={styles["logo"]} />
                </h2>
                <p className={styles["description"]}>
                    {t("settings.about.description")}
                </p>
            </header>
            <dl className={styles["details"]}>
                <div>
                    <dt>{t("settings.about.version")}</dt>
                    <dd>
                        {state.kind === "loading" && (
                            <span role="status">
                                {t("settings.about.loading")}
                            </span>
                        )}
                        {state.kind === "ready" && state.info.version}
                        {state.kind === "preview" && (
                            <span>{t("settings.about.preview")}</span>
                        )}
                        {state.kind === "error" && (
                            <>
                                <span role="alert">
                                    {t("settings.about.failed")}
                                </span>
                                <button
                                    className={styles["retry"]}
                                    type="button"
                                    onClick={() => {
                                        setState({ kind: "loading" });
                                        setAttempt((previous) => previous + 1);
                                    }}
                                >
                                    {t("settings.about.retry")}
                                </button>
                            </>
                        )}
                    </dd>
                </div>
                <div>
                    <dt>{t("settings.about.license")}</dt>
                    <dd>MIT</dd>
                </div>
                <div>
                    <dt>{t("settings.about.repository")}</dt>
                    <dd>
                        <button
                            className={styles["repository"]}
                            type="button"
                            aria-label={t("settings.about.openRepository")}
                            title={t("settings.about.openRepository")}
                            disabled={repositoryState === "opening"}
                            onClick={() => {
                                void openRepository();
                            }}
                        >
                            {/* GitHub's Octicons mark, MIT; see assets/licenses/octicons-MIT.txt. */}
                            <svg
                                viewBox="0 0 16 16"
                                width="24"
                                height="24"
                                fill="currentColor"
                                aria-hidden="true"
                                focusable="false"
                            >
                                <path d="M6.766 11.328c-2.063-.25-3.516-1.734-3.516-3.656 0-.781.281-1.625.75-2.188-.203-.515-.172-1.609.063-2.062.625-.078 1.468.25 1.968.703.594-.187 1.219-.281 1.985-.281.765 0 1.39.094 1.953.265.484-.437 1.344-.765 1.969-.687.218.422.25 1.515.046 2.047.5.593.766 1.39.766 2.203 0 1.922-1.453 3.375-3.547 3.64.531.344.89 1.094.89 1.954v1.625c0 .468.391.734.86.547C13.781 14.359 16 11.53 16 8.03 16 3.61 12.406 0 7.984 0 3.563 0 0 3.61 0 8.031a7.88 7.88 0 0 0 5.172 7.422c.422.156.828-.125.828-.547v-1.25c-.219.094-.5.156-.75.156-1.031 0-1.64-.562-2.078-1.609-.172-.422-.36-.672-.719-.719-.187-.015-.25-.093-.25-.187 0-.188.313-.328.625-.328.453 0 .844.281 1.25.86.313.452.64.655 1.031.655s.641-.14 1-.5c.266-.265.47-.5.657-.656" />
                            </svg>
                        </button>
                        {repositoryState === "opening" && (
                            <span className={styles["feedback"]} role="status">
                                {t("settings.about.openingRepository")}
                            </span>
                        )}
                        {repositoryState === "error" && (
                            <span className={styles["feedback"]} role="alert">
                                {t("settings.about.repositoryFailed")}
                            </span>
                        )}
                    </dd>
                </div>
            </dl>
        </div>
    );
}
