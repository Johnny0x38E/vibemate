import {
    useEffect,
    useId,
    useLayoutEffect,
    useRef,
    useState,
    type JSX,
} from "react";
import { useTranslation } from "react-i18next";
import {
    AppearanceRequestError,
    getAppearancePreference,
    saveAppearancePreference,
    isAppearance,
    THEME_IDS,
    type AppearancePreference,
} from "../../lib/desktop/appearance";
import fieldStyles from "./settingsField.module.css";
import styles from "./AppearanceControl.module.css";

type ControlState =
    | { kind: "loading" | "readFailed"; preference: AppearancePreference }
    | { kind: "preview"; preference: AppearancePreference }
    | {
          kind: "desktop";
          preference: AppearancePreference;
          operation: "idle" | "saving" | "saveFailed" | "saveUnconfirmed";
      };

/**
 * Load and save brightness/palette together through Rust. Preview changes are
 * window-local. Confirmed choices update root tokens without remounting forms;
 * unknown save outcomes disable writes until the saved pair is re-read.
 */
export function AppearanceControl(): JSX.Element {
    const { t } = useTranslation();
    const id = useId();
    const [state, setState] = useState<ControlState>({
        kind: "loading",
        preference: { appearance: "system", theme: "forest" },
    });
    const [attempt, setAttempt] = useState(0);
    const pending = useRef(true);
    const alive = useRef(false);

    useEffect(() => {
        let active = true;
        alive.current = true;
        pending.current = true;
        // Each read owns its response; StrictMode's discarded request must not
        // replace a newer preference or clear the current request guard.
        getAppearancePreference().then(
            (result) => {
                if (!active) return;
                setState(
                    result.kind === "preview"
                        ? {
                              kind: "preview",
                              preference: {
                                  appearance: "system",
                                  theme: "forest",
                              },
                          }
                        : {
                              kind: "desktop",
                              preference: result.preference,
                              operation: "idle",
                          },
                );
                pending.current = false;
            },
            () => {
                if (!active) return;
                setState((previous) => ({
                    kind: "readFailed",
                    preference: previous.preference,
                }));
                pending.current = false;
            },
        );
        return () => {
            active = false;
            alive.current = false;
        };
    }, [attempt]);

    const { appearance, theme } = state.preference;
    useLayoutEffect(() => {
        // The document root is outside React's tree. Own only these attributes;
        // restoring prior values supports cleanup and StrictMode's lifecycle.
        const root = document.documentElement;
        const previousAppearance = root.dataset["appearance"];
        const previousTheme = root.dataset["theme"];
        root.dataset["appearance"] = appearance;
        root.dataset["theme"] = theme;
        return () => {
            if (previousAppearance === undefined)
                delete root.dataset["appearance"];
            else root.dataset["appearance"] = previousAppearance;
            if (previousTheme === undefined) delete root.dataset["theme"];
            else root.dataset["theme"] = previousTheme;
        };
    }, [appearance, theme]);

    async function changePreference(
        preference: AppearancePreference,
    ): Promise<void> {
        if (pending.current) return;
        if (state.kind === "preview") {
            setState({ kind: "preview", preference });
            return;
        }
        if (state.kind !== "desktop" || state.operation === "saveUnconfirmed")
            return;
        pending.current = true;
        const previous = state.preference;
        setState({
            kind: "desktop",
            preference: previous,
            operation: "saving",
        });
        try {
            const saved = await saveAppearancePreference(preference);
            if (alive.current)
                setState({
                    kind: "desktop",
                    preference: saved,
                    operation: "idle",
                });
        } catch (error: unknown) {
            if (!alive.current) return;
            const unchanged =
                error instanceof AppearanceRequestError &&
                (error.code === "write_failed" ||
                    error.code === "storage_unavailable");
            setState({
                kind: "desktop",
                preference: previous,
                operation: unchanged ? "saveFailed" : "saveUnconfirmed",
            });
        } finally {
            if (alive.current) pending.current = false;
        }
    }

    function reload(): void {
        if (pending.current) return;
        pending.current = true;
        setState({ kind: "loading", preference: state.preference });
        setAttempt((previous) => previous + 1);
    }

    const blocked =
        state.kind === "loading" ||
        state.kind === "readFailed" ||
        (state.kind === "desktop" &&
            (state.operation === "saving" ||
                state.operation === "saveUnconfirmed"));
    const message =
        state.kind === "desktop"
            ? state.operation === "idle" || state.operation === "saving"
                ? undefined
                : state.operation
            : state.kind;
    const canReload =
        state.kind === "readFailed" ||
        (state.kind === "desktop" && state.operation === "saveUnconfirmed");
    const failed =
        state.kind === "readFailed" ||
        (state.kind === "desktop" &&
            (state.operation === "saveFailed" ||
                state.operation === "saveUnconfirmed"));

    return (
        <>
            <div className={fieldStyles["row"]}>
                <label
                    className={fieldStyles["label"]}
                    htmlFor={`${id}-appearance`}
                >
                    {t("settings.appearance.label")}
                </label>
                <div className={fieldStyles["field"]}>
                    <select
                        className={fieldStyles["select"]}
                        id={`${id}-appearance`}
                        value={appearance}
                        disabled={blocked}
                        onChange={(event) => {
                            const value = event.currentTarget.value;
                            if (isAppearance(value))
                                void changePreference({
                                    appearance: value,
                                    theme,
                                });
                        }}
                    >
                        {(["system", "light", "dark"] as const).map((mode) => (
                            <option key={mode} value={mode}>
                                {t(`settings.appearance.modes.${mode}`)}
                            </option>
                        ))}
                    </select>
                </div>
            </div>
            <div className={fieldStyles["row"]}>
                <span id={`${id}-theme-label`} className={fieldStyles["label"]}>
                    {t("settings.theme.label")}
                </span>
                <div
                    className={styles["themes"]}
                    role="radiogroup"
                    aria-labelledby={`${id}-theme-label`}
                >
                    {THEME_IDS.map((palette) => (
                        <label
                            className={styles["choice"]}
                            key={palette}
                            data-palette={palette}
                            title={t(`settings.theme.names.${palette}`)}
                        >
                            {/* Native radios provide one selection and keyboard arrow
                                navigation; styling changes only the visible circle. */}
                            <input
                                className={styles["radio"]}
                                type="radio"
                                name={`${id}-theme`}
                                value={palette}
                                aria-label={t(
                                    `settings.theme.names.${palette}`,
                                )}
                                checked={theme === palette}
                                disabled={blocked}
                                onChange={() => {
                                    void changePreference({
                                        appearance,
                                        theme: palette,
                                    });
                                }}
                            />
                            <span
                                className={styles["swatch"]}
                                aria-hidden="true"
                            />
                        </label>
                    ))}
                </div>
            </div>
            {message !== undefined && (
                <div className={styles["feedback"]}>
                    <p
                        className={styles["message"]}
                        role={failed ? "alert" : "status"}
                    >
                        {t(`settings.theme.${message}`)}
                    </p>
                    {canReload && (
                        <button
                            className={styles["retry"]}
                            type="button"
                            onClick={reload}
                        >
                            {t("settings.theme.reload")}
                        </button>
                    )}
                </div>
            )}
        </>
    );
}
