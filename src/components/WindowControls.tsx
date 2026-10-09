import { useEffect, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import {
    closeWindow,
    isWindowMaximized,
    listenWindowMaximized,
    minimizeWindow,
    toggleMaximizeWindow,
    usesCustomWindowChrome,
} from "../lib/desktop/window";
import styles from "./WindowControls.module.css";

/**
 * Platform window controls for undecorated Windows and Linux builds.
 * macOS and browser preview omit this component entirely.
 */
export function WindowControls(): JSX.Element | null {
    const { t } = useTranslation();
    const [maximized, setMaximized] = useState(false);

    useEffect(() => {
        if (!usesCustomWindowChrome()) return;
        let unlisten: (() => void) | undefined;
        let active = true;

        void listenWindowMaximized((value) => {
            if (active) setMaximized(value);
        }).then((dispose) => {
            if (!active) {
                dispose();
                return;
            }
            unlisten = dispose;
        });

        return () => {
            active = false;
            unlisten?.();
        };
    }, []);

    if (!usesCustomWindowChrome()) return null;

    const maximizeLabel = maximized
        ? t("desktop.window.restore")
        : t("desktop.window.maximize");

    return (
        <div
            className={styles["controls"]}
            role="group"
            aria-label={t("desktop.window.group")}
        >
            <button
                className={styles["control"]}
                type="button"
                aria-label={t("desktop.window.minimize")}
                title={t("desktop.window.minimize")}
                onClick={() => {
                    void minimizeWindow();
                }}
            >
                <span className={styles["iconMinimize"]} aria-hidden="true" />
            </button>
            <button
                className={styles["control"]}
                type="button"
                aria-label={maximizeLabel}
                title={maximizeLabel}
                onClick={() => {
                    void (async () => {
                        await toggleMaximizeWindow();
                        setMaximized(await isWindowMaximized());
                    })();
                }}
            >
                {maximized ? (
                    <span
                        className={styles["iconRestore"]}
                        aria-hidden="true"
                    />
                ) : (
                    <span
                        className={styles["iconMaximize"]}
                        aria-hidden="true"
                    />
                )}
            </button>
            <button
                className={[styles["control"], styles["controlClose"]]
                    .filter((value): value is string => value !== undefined)
                    .join(" ")}
                type="button"
                aria-label={t("desktop.window.close")}
                title={t("desktop.window.close")}
                onClick={() => {
                    void closeWindow();
                }}
            >
                <span className={styles["iconClose"]} aria-hidden="true" />
            </button>
        </div>
    );
}
