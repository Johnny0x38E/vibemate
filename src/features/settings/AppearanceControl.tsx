import { useId, useLayoutEffect, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import fieldStyles from "./settingsField.module.css";

type Appearance = "system" | "light" | "dark";

const APPEARANCE_MODES: Appearance[] = ["system", "light", "dark"];

/**
 * Pick this window's appearance without saving a preference or remounting UI.
 * CSS follows OS changes in system mode. Own the document attribute only while
 * mounted and restore it during cleanup.
 */
export function AppearanceControl(): JSX.Element {
    const { t } = useTranslation();
    const [appearance, setAppearance] = useState<Appearance>("system");
    const id = useId();

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
        <div className={fieldStyles["row"]}>
            <label className={fieldStyles["label"]} htmlFor={id}>
                {t("settings.appearance.label")}
            </label>
            <div className={fieldStyles["field"]}>
                <select
                    className={fieldStyles["select"]}
                    id={id}
                    value={appearance}
                    onChange={(event) => {
                        const value = event.currentTarget.value;
                        if (
                            value === "system" ||
                            value === "light" ||
                            value === "dark"
                        ) {
                            setAppearance(value);
                        }
                    }}
                >
                    {APPEARANCE_MODES.map((mode) => (
                        <option key={mode} value={mode}>
                            {t(`settings.appearance.modes.${mode}`)}
                        </option>
                    ))}
                </select>
            </div>
        </div>
    );
}
