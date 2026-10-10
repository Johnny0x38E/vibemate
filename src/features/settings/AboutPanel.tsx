import { useEffect, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import { getAppInfo, type AppInfo } from "../../lib/desktop";
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

    return (
        <div className={styles["card"]}>
            <h2 className={styles["brand"]}>
                {state.kind === "ready" ? state.info.name : "vibemate"}
            </h2>
            <p className={styles["description"]}>
                {t("settings.about.description")}
            </p>
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
                    <dd className={styles["repository"]}>
                        https://github.com/Johnny0x38E/vibemate
                    </dd>
                </div>
            </dl>
        </div>
    );
}
