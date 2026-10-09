import {
    useEffect,
    useLayoutEffect,
    useRef,
    useState,
    type JSX,
    type ReactNode,
} from "react";
import type { i18n } from "i18next";
import { I18nextProvider } from "react-i18next";
import { createAppI18n, resolveSystemLocale } from "../../i18n";
import {
    getLocalePreference,
    type LocalePreferenceResult,
} from "../../lib/desktop/settings";
import en from "../../locales/en.json";
import zhCN from "../../locales/zh-CN.json";
import styles from "./LocaleStartup.module.css";

/** Validated startup inputs supplied to the ready application composition. */
export interface LocaleStartupSnapshot {
    initialPreference: LocalePreferenceResult;
    systemLanguage: string;
}

interface LocaleStartupProps {
    children: (snapshot: LocaleStartupSnapshot) => ReactNode;
    systemLanguage: string;
}

type StartupState =
    | { status: "loading" | "readFailed" | "initializationFailed" }
    | {
          status: "ready";
          preference: LocalePreferenceResult;
          instance: i18n;
      };

/**
 * Read the saved choice and prepare its translator before rendering children.
 * Call the supplied render function with the validated startup snapshot; the
 * caller decides where settings belong. Capture the system language at mount.
 * Failures block the app and offer retry without writing a default preference.
 * The ready provider stays mounted across saves so child form state is retained.
 */
export function LocaleStartup({
    children,
    systemLanguage: initialSystemLanguage,
}: LocaleStartupProps): JSX.Element {
    const [systemLanguage] = useState(initialSystemLanguage);
    const systemLocale = resolveSystemLocale(systemLanguage);
    const messages = (systemLocale === "zh-CN" ? zhCN : en).settings.startup;
    const [state, setState] = useState<StartupState>({ status: "loading" });
    const [attempt, setAttempt] = useState(0);
    const pending = useRef(true);

    useEffect(() => {
        let active = true;
        pending.current = true;

        // Re-read after every await; TypeScript's earlier narrowing cannot model
        // effect cleanup changing this closure while IPC or initialization waits.
        function isActive(): boolean {
            return active;
        }

        async function start(): Promise<void> {
            let failure: "readFailed" | "initializationFailed" = "readFailed";
            try {
                const preference = await getLocalePreference();
                if (!isActive()) return;
                const locale =
                    preference.kind === "desktop" &&
                    preference.preference !== "system"
                        ? preference.preference
                        : systemLocale;
                failure = "initializationFailed";
                const instance = await createAppI18n(locale);
                if (isActive())
                    setState({ status: "ready", preference, instance });
            } catch {
                // Never expose diagnostics or turn a failed read into a default save.
                if (isActive()) setState({ status: failure });
            } finally {
                if (isActive()) pending.current = false;
            }
        }

        // IPC is not cancellable. Each effect owns its cleanup flag so an obsolete
        // StrictMode request cannot initialize or replace the current translator.
        void start();
        return () => {
            active = false;
        };
    }, [attempt, systemLocale]);

    useLayoutEffect(() => {
        // The document element is outside React's root. Update its language before
        // paint, then follow confirmed translator changes without remounting UI.
        if (state.status !== "ready") {
            document.documentElement.lang = systemLocale;
            return;
        }
        const instance = state.instance;
        const synchronizeLanguage = () => {
            document.documentElement.lang =
                instance.resolvedLanguage ?? systemLocale;
        };
        synchronizeLanguage();
        instance.on("languageChanged", synchronizeLanguage);
        return () => {
            instance.off("languageChanged", synchronizeLanguage);
        };
    }, [state, systemLocale]);

    if (state.status !== "ready") {
        return (
            <main
                className={styles["startup"]}
                aria-busy={state.status === "loading"}
            >
                <p role={state.status === "loading" ? "status" : "alert"}>
                    {state.status === "loading"
                        ? messages.loading
                        : messages[state.status]}
                </p>
                {state.status !== "loading" && (
                    <button
                        className={styles["retry"]}
                        type="button"
                        onClick={() => {
                            // Close the gap before React removes the retry button.
                            if (pending.current) return;
                            pending.current = true;
                            setState({ status: "loading" });
                            setAttempt((previous) => previous + 1);
                        }}
                    >
                        {messages.retry}
                    </button>
                )}
            </main>
        );
    }

    return (
        <I18nextProvider i18n={state.instance}>
            {children({ initialPreference: state.preference, systemLanguage })}
        </I18nextProvider>
    );
}
