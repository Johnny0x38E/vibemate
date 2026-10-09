import { useId, useLayoutEffect, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import { Icon } from "../../components/Icon";
import styles from "./AppearanceControl.module.css";

type Appearance = "system" | "light" | "dark";
const nextAppearance: Record<Appearance, Appearance> = {
    system: "light",
    light: "dark",
    dark: "system",
};

/**
 * Cycle this window's appearance without saving a preference or remounting UI.
 * CSS follows OS changes in system mode; the icon describes the chosen mode.
 * Own the document attribute only while mounted and restore it during cleanup.
 */
export function AppearanceControl(): JSX.Element {
    const { t } = useTranslation();
    const [appearance, setAppearance] = useState<Appearance>("system");
    const statusId = useId();
    const current = t(`settings.appearance.modes.${appearance}`);
    const next = t(`settings.appearance.modes.${nextAppearance[appearance]}`);
    const action = t("settings.appearance.cycle", { current, next });

    useLayoutEffect(() => {
        // The document element is outside React's root. Apply before paint; restore
        // the previous owner on cleanup, including StrictMode's development check.
        const root = document.documentElement;
        const previous = root.dataset["appearance"];
        root.dataset["appearance"] = appearance;
        return () => {
            if (previous === undefined) delete root.dataset["appearance"];
            else root.dataset["appearance"] = previous;
        };
    }, [appearance]);

    return (
        <div className={styles["row"]}>
            <div>
                <h2 className={styles["label"]}>
                    {t("settings.appearance.label")}
                </h2>
                <p
                    className={styles["description"]}
                    id={statusId}
                    role="status"
                >
                    {t("settings.appearance.current", { mode: current })}
                </p>
                <p className={styles["description"]}>
                    {t("settings.appearance.temporary")}
                </p>
            </div>
            <button
                className={styles["button"]}
                type="button"
                aria-label={action}
                aria-describedby={statusId}
                title={action}
                onClick={() => {
                    setAppearance((previous) => nextAppearance[previous]);
                }}
            >
                <Icon name={appearance} />
            </button>
        </div>
    );
}
