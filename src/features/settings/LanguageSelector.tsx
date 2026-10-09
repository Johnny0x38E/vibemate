import {
    useEffect,
    useId,
    useRef,
    useState,
    type JSX,
    type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { resolveSystemLocale } from "../../i18n";
import {
    getLocalePreference,
    saveLocalePreference,
    SettingsRequestError,
    type LocalePreference,
    type LocalePreferenceResult,
} from "../../lib/desktop/settings";
import fieldStyles from "./settingsField.module.css";
import styles from "./LanguageSelector.module.css";

/** Startup supplies a validated choice and the system language before mounting. */
export interface LanguageSelectorProps {
    /** Startup snapshot, or preview mode. Parent rerenders do not reset local choices. */
    initialPreference: LocalePreferenceResult;
    /** Used only when the confirmed choice is "system"; explicit choices take priority. */
    systemLanguage: string;
    /** Additional preference rows rendered inside the grouped settings card. */
    footer?: ReactNode;
}

type Status =
    | "idle"
    | "saving"
    | "saveFailed"
    | "saveUnconfirmed"
    | "reloading"
    | "reloadFailed"
    | "applyFailed"
    | "preview";

const ERROR_MESSAGE_KEY: Partial<
    Record<
        Status,
        | "settings.language.saveFailed"
        | "settings.language.saveUnconfirmed"
        | "settings.language.reloadFailed"
        | "settings.language.applyFailed"
    >
> = {
    saveFailed: "settings.language.saveFailed",
    saveUnconfirmed: "settings.language.saveUnconfirmed",
    reloadFailed: "settings.language.reloadFailed",
    applyFailed: "settings.language.applyFailed",
};

/**
 * Save a language choice through Rust before changing the shared React translator.
 * Local state changes do not remount sibling forms. Startup and HTML language
 * synchronization belong to the parent, not this selector.
 */
export function LanguageSelector({
    initialPreference,
    systemLanguage,
    footer,
}: LanguageSelectorProps): JSX.Element {
    const { t, i18n } = useTranslation();
    const id = useId();
    const [preference, setPreference] = useState<LocalePreference>(
        initialPreference.kind === "desktop"
            ? initialPreference.preference
            : "system",
    );
    const [status, setStatus] = useState<Status>("idle");
    const active = useRef(true);
    const pending = useRef(false);
    const preview =
        initialPreference.kind === "preview" || status === "preview";
    const busy = status === "saving" || status === "reloading";
    const needsReload =
        status === "saveUnconfirmed" ||
        status === "reloadFailed" ||
        status === "applyFailed";

    useEffect(() => {
        active.current = true;
        // An IPC write cannot be cancelled, but its late result must not change the
        // translator or UI after this selector unmounts. StrictMode reruns setup.
        return () => {
            active.current = false;
        };
    }, []);

    // Read the mutable ref afresh after every await. TypeScript otherwise keeps
    // an earlier property narrowing even though effect cleanup can change it.
    function isActive(): boolean {
        return active.current;
    }

    async function applyConfirmed(confirmed: LocalePreference): Promise<void> {
        if (!isActive()) return;
        // The persisted choice is known even if changing the translator fails.
        // Never label an already committed write as a failed save.
        setPreference(confirmed);
        try {
            await i18n.changeLanguage(
                confirmed === "system"
                    ? resolveSystemLocale(systemLanguage)
                    : confirmed,
            );
            if (isActive()) setStatus("idle");
        } catch {
            if (isActive()) setStatus("applyFailed");
        }
    }

    async function save(next: LocalePreference): Promise<void> {
        if (preview || pending.current || needsReload || next === preference)
            return;
        // A ref closes the brief gap before React renders the disabled control.
        pending.current = true;
        setStatus("saving");
        try {
            const saved = await saveLocalePreference(next);
            await applyConfirmed(saved);
        } catch (error: unknown) {
            if (!active.current) return;
            if (
                error instanceof SettingsRequestError &&
                error.code === "desktop_required"
            ) {
                setStatus("preview");
            } else if (
                error instanceof SettingsRequestError &&
                (error.code === "write_failed" ||
                    error.code === "storage_unavailable")
            ) {
                setStatus("saveFailed");
            } else {
                // Transport/task failures may follow a committed write. Stop new writes
                // until an explicit read reconciles the displayed and persisted choice.
                setStatus("saveUnconfirmed");
            }
        } finally {
            pending.current = false;
        }
    }

    async function reload(): Promise<void> {
        if (preview || pending.current || !needsReload) return;
        pending.current = true;
        setStatus("reloading");
        try {
            const result = await getLocalePreference();
            if (!active.current) return;
            if (result.kind === "preview") {
                setStatus("preview");
                return;
            }
            await applyConfirmed(result.preference);
        } catch {
            if (active.current) setStatus("reloadFailed");
        } finally {
            pending.current = false;
        }
    }

    const errorKey = ERROR_MESSAGE_KEY[status];
    const statusMessage = preview
        ? t("settings.language.preview")
        : errorKey
          ? t(errorKey)
          : null;

    return (
        <div className={styles["selector"]}>
            <div className={fieldStyles["group"]}>
                <div className={fieldStyles["row"]}>
                    <label className={fieldStyles["label"]} htmlFor={id}>
                        {t("settings.language.label")}
                    </label>
                    {/* The wrapper draws the arrow: WebKit ignores height on a native select
                    unless its appearance is removed, and CSP forbids data: images. */}
                    <div className={fieldStyles["field"]}>
                        <select
                            className={fieldStyles["select"]}
                            id={id}
                            value={preference}
                            disabled={preview || busy || needsReload}
                            aria-describedby={
                                statusMessage ? `${id}-status` : undefined
                            }
                            onChange={(event) => {
                                const value = event.currentTarget.value;
                                if (
                                    value === "system" ||
                                    value === "zh-CN" ||
                                    value === "en"
                                ) {
                                    // save handles both rejection and lifecycle cleanup internally.
                                    void save(value);
                                }
                            }}
                        >
                            <option value="system">
                                {t("settings.language.system")}
                            </option>
                            <option value="zh-CN" lang="zh-CN">
                                {t("settings.language.zhCN")}
                            </option>
                            <option value="en" lang="en">
                                {t("settings.language.en")}
                            </option>
                        </select>
                    </div>
                </div>
                {footer}
            </div>
            {statusMessage ? (
                <p
                    className={styles["message"]}
                    id={`${id}-status`}
                    role="status"
                >
                    {statusMessage}
                </p>
            ) : null}
            {(needsReload || status === "reloading") && (
                <button
                    className={styles["retry"]}
                    type="button"
                    disabled={busy}
                    onClick={() => {
                        // reload also handles rejection and ignores results after cleanup.
                        void reload();
                    }}
                >
                    {t("settings.language.reload")}
                </button>
            )}
        </div>
    );
}
