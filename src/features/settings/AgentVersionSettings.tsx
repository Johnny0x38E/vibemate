import { useEffect, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import { ModelCheckbox } from "../../components/ModelCheckbox";
import {
    SettingsRequestError,
    getAutoVersionProbe,
    saveAutoVersionProbe,
} from "../../lib/desktop/settings";
import styles from "./AgentVersionSettings.module.css";

type ControlState =
    | { kind: "loading" | "readFailed"; autoProbe: boolean }
    | { kind: "preview"; autoProbe: boolean }
    | {
          kind: "desktop";
          autoProbe: boolean;
          operation: "idle" | "saving" | "saveFailed" | "saveUnconfirmed";
      };

/**
 * One-time informed consent for Agent CLI `--version` probes. When enabled,
 * discovery and the detail view may probe without a per-run click. Results stay
 * in window memory; this control never displays or stores a version number.
 */
export function AgentVersionSettings(): JSX.Element {
    const { t } = useTranslation();
    const [state, setState] = useState<ControlState>({
        kind: "loading",
        autoProbe: false,
    });
    const pending = useRef(false);
    const alive = useRef(false);

    useEffect(() => {
        let active = true;
        alive.current = true;
        pending.current = true;
        // Each read owns its response; StrictMode's discarded request must not
        // replace a newer preference or clear the current request guard.
        getAutoVersionProbe().then(
            (result) => {
                if (!active) return;
                setState(
                    result.kind === "preview"
                        ? { kind: "preview", autoProbe: false }
                        : {
                              kind: "desktop",
                              autoProbe: result.preference.autoProbe,
                              operation: "idle",
                          },
                );
                pending.current = false;
            },
            () => {
                if (!active) return;
                setState((previous) => ({
                    kind: "readFailed",
                    autoProbe: previous.autoProbe,
                }));
                pending.current = false;
            },
        );
        return () => {
            active = false;
            alive.current = false;
        };
    }, []);

    async function changePreference(autoProbe: boolean): Promise<void> {
        if (pending.current) return;
        if (state.kind === "preview") {
            setState({ kind: "preview", autoProbe });
            return;
        }
        if (state.kind !== "desktop" || state.operation === "saveUnconfirmed")
            return;
        pending.current = true;
        const previous = state.autoProbe;
        setState({
            kind: "desktop",
            autoProbe: previous,
            operation: "saving",
        });
        try {
            const saved = await saveAutoVersionProbe({ autoProbe });
            if (alive.current) {
                setState({
                    kind: "desktop",
                    autoProbe: saved.autoProbe,
                    operation: "idle",
                });
                pending.current = false;
            }
        } catch (error: unknown) {
            if (!alive.current) return;
            pending.current = false;
            const code =
                error instanceof SettingsRequestError
                    ? error.code
                    : "operation_failed";
            if (code === "invalid_response") {
                // The persisted value is unknown. Block further writes until a
                // read establishes which preference is actually stored.
                setState({
                    kind: "desktop",
                    autoProbe: previous,
                    operation: "saveUnconfirmed",
                });
                return;
            }
            setState({
                kind: "desktop",
                autoProbe: previous,
                operation: "saveFailed",
            });
        }
    }

    const autoProbe = state.autoProbe;
    const saving =
        state.kind === "desktop" &&
        (state.operation === "saving" || state.operation === "saveUnconfirmed");
    return (
        <section className={styles["section"]}>
            <div className={styles["control"]}>
                <ModelCheckbox
                    checked={autoProbe}
                    disabled={saving || state.kind === "readFailed"}
                    ariaLabel={t("settings.agentVersion.enable")}
                    onCheckedChange={(next) => {
                        void changePreference(next);
                    }}
                />
                <span className={styles["label"]}>
                    {t("settings.agentVersion.enable")}
                </span>
            </div>
            {state.kind === "preview" && (
                <p className={styles["hint"]}>
                    {t("settings.agentVersion.preview")}
                </p>
            )}
            {state.kind === "readFailed" && (
                <p className={styles["error"]} role="alert">
                    {t("settings.agentVersion.readFailed")}
                </p>
            )}
            {state.kind === "desktop" && state.operation === "saveFailed" && (
                <p className={styles["error"]} role="alert">
                    {t("settings.agentVersion.saveFailed")}
                </p>
            )}
            {state.kind === "desktop" &&
                state.operation === "saveUnconfirmed" && (
                    <p className={styles["error"]} role="alert">
                        {t("settings.agentVersion.saveUnconfirmed")}
                    </p>
                )}
        </section>
    );
}
