import {
    useEffect,
    useId,
    useRef,
    useState,
    type JSX,
    type SubmitEvent,
} from "react";
import { useTranslation } from "react-i18next";
import { useNotify } from "../../components/Notifications";
import {
    ProviderRequestError,
    type ProviderErrorCode,
} from "../../lib/desktop/providers";
import {
    getProviderSecretStatus,
    replaceProviderSecret,
    type ProviderSecretStatus,
} from "../../lib/desktop/providerSecrets";
import fieldStyles from "../settings/settingsField.module.css";
import buttons from "./providerButtons.module.css";
import formStyles from "./ProviderForm.module.css";
import styles from "./ProviderKeys.module.css";

/** Inputs for {@link ProviderKeys}. */
export interface ProviderKeysProps {
    /** The saved instance whose key is shown; identified by ID only. */
    providerId: string;
    /** Reports a pending replacement so the page can block its back control. */
    onBusyChange: (busy: boolean) => void;
    /** True while the page is hidden; a typed key is cleared meanwhile. */
    hidden?: boolean;
}

/** The key status as last read from Rust. */
type ReadState =
    | { kind: "loading" | "preview" }
    | { kind: "ready"; status: ProviderSecretStatus }
    | { kind: "error"; code: ProviderErrorCode };

/**
 * One state per replacement step. `unknown` follows a replacement whose result
 * could not be confirmed; `readCode` is set when re-reading the status failed.
 */
type ActionState =
    | { kind: "idle" | "replacing" | "refreshing" | "refreshed" }
    | { kind: "failed"; code: ProviderErrorCode }
    | {
          kind: "unknown";
          code: ProviderErrorCode;
          readCode: ProviderErrorCode | null;
      };

/** Store failures concern the device, so they are shown at the top. */
const TOP_CODES: readonly ProviderErrorCode[] = [
    "credential_store_unavailable",
    "credential_store_access_denied",
    "credential_store_failed",
];

/** After these, the credential store may already hold the new key. */
const UNKNOWN_CODES: readonly ProviderErrorCode[] = [
    "secret_outcome_unknown",
    "operation_failed",
    "invalid_response",
];

/**
 * Replace codes whose generic provider wording would mislead: each of these
 * leaves the key unchanged (or, for the unknown codes, possibly changed), so
 * the text says so. `read_failed` here comes from the existence check or a
 * commit whose previous key was restored, never from reading the key.
 */
const REPLACE_ERROR_CODES = [
    "read_failed",
    "write_failed",
    "storage_unavailable",
    "invalid_request",
    "not_found",
    "operation_failed",
    "invalid_response",
    "desktop_required",
] as const;
type ReplaceErrorCode = (typeof REPLACE_ERROR_CODES)[number];

/** Status-read codes whose generic wording would suggest a write happened. */
const READ_ERROR_CODES = [
    "read_failed",
    "not_found",
    "operation_failed",
    "invalid_response",
] as const;
type ReadErrorCode = (typeof READ_ERROR_CODES)[number];

function isReplaceErrorCode(code: ProviderErrorCode): code is ReplaceErrorCode {
    return REPLACE_ERROR_CODES.some((entry) => entry === code);
}

function isReadErrorCode(code: ProviderErrorCode): code is ReadErrorCode {
    return READ_ERROR_CODES.some((entry) => entry === code);
}

function errorCode(error: unknown): ProviderErrorCode {
    return error instanceof ProviderRequestError
        ? error.code
        : "operation_failed";
}

/**
 * The "API key" group of a saved provider's detail page. It shows whether a key
 * is set (from SQLite only, so opening the page never prompts for the system
 * credential store) and when it was last saved, and replaces it. A `missing`
 * key, only possible for configurations saved before keys existed, is set with
 * the same control. There is no way to clear a key.
 *
 * This is its own form: replacing never saves the basic information and never
 * leaves the page. The typed key lives only in this component's state and is
 * cleared after every submit, while the page is hidden and on unmount. The key
 * is never shown, counted or masked; only its status is. Controls are never
 * `disabled` while work is pending: the button uses `aria-disabled` and the
 * field `readOnly`, and handlers check the state.
 */
export function ProviderKeys({
    providerId,
    onBusyChange,
    hidden = false,
}: ProviderKeysProps): JSX.Element {
    const { t, i18n } = useTranslation();
    const notify = useNotify();
    const id = useId();
    const [read, setRead] = useState<ReadState>({ kind: "loading" });
    const [readAttempt, setReadAttempt] = useState(0);
    const [action, setAction] = useState<ActionState>({ kind: "idle" });
    const [secret, setSecret] = useState("");
    const [secretMissing, setSecretMissing] = useState(false);
    // Clear on hide during render, so a key never survives a hidden frame.
    const [wasHidden, setWasHidden] = useState(hidden);
    if (hidden !== wasHidden) {
        setWasHidden(hidden);
        if (hidden) {
            setSecret("");
            setSecretMissing(false);
        }
    }
    const alive = useRef(true);
    const pending = useRef(false);
    const submit = useRef<HTMLButtonElement>(null);
    const secretInput = useRef<HTMLInputElement>(null);

    useEffect(() => {
        alive.current = true;
        // IPC cannot be cancelled; late results must not update an unmounted
        // group. StrictMode runs this setup twice, so it resets the flag too.
        return () => {
            alive.current = false;
        };
    }, []);

    // Read the mutable ref afresh after each await; cleanup can change it.
    function isAlive(): boolean {
        return alive.current;
    }

    useEffect(() => {
        let active = true;
        getProviderSecretStatus(providerId).then(
            (result) => {
                if (!active) return;
                setRead(
                    result.kind === "preview"
                        ? { kind: "preview" }
                        : { kind: "ready", status: result.status },
                );
            },
            (error: unknown) => {
                if (active) setRead({ kind: "error", code: errorCode(error) });
            },
        );
        return () => {
            active = false;
        };
    }, [providerId, readAttempt]);

    const busy = action.kind === "replacing";
    const working = busy || action.kind === "refreshing";
    const blocked = read.kind !== "ready" || working;
    const unknown = action.kind === "unknown" || action.kind === "refreshing";
    const missing =
        !unknown && read.kind === "ready" && read.status.state === "missing";
    const fieldError =
        secretMissing ||
        (action.kind === "failed" && action.code === "secret_invalid");
    const topError =
        action.kind === "failed" && TOP_CODES.includes(action.code)
            ? action.code
            : undefined;

    useEffect(() => {
        onBusyChange(busy);
        return () => {
            onBusyChange(false);
        };
    }, [busy, onBusyChange]);

    /** Text for a failed replacement; it always says whether the key changed. */
    function replaceError(code: ProviderErrorCode): string {
        return isReplaceErrorCode(code)
            ? t(`providers.keys.replaceErrors.${code}`)
            : t(`providers.errors.${code}`);
    }

    /** Text for a failed status read, which never changes anything. */
    function readError(code: ProviderErrorCode): string {
        return isReadErrorCode(code)
            ? t(`providers.keys.readErrors.${code}`)
            : t(`providers.errors.${code}`);
    }

    async function replace(): Promise<void> {
        // Every submit clears the field, whatever the result, including one
        // that is ignored because the status is loading or being refreshed.
        const key = secret;
        setSecret("");
        if (pending.current || working || read.kind !== "ready") return;
        const wasMissing = read.status.state === "missing";
        if (key.trim() === "") {
            // The only check made here; Rust validates everything else.
            setSecretMissing(true);
            secretInput.current?.focus();
            return;
        }
        setSecretMissing(false);
        pending.current = true;
        setAction({ kind: "replacing" });
        try {
            const status = await replaceProviderSecret({
                providerId,
                secret: key,
            });
            if (!isAlive()) return;
            setRead({ kind: "ready", status });
            setAction({ kind: "idle" });
            // Announced without moving focus; the page stays open.
            notify(
                wasMissing
                    ? t("providers.keys.keySet")
                    : t("providers.keys.replaced"),
            );
        } catch (error: unknown) {
            if (!isAlive()) return;
            const code = errorCode(error);
            setAction(
                UNKNOWN_CODES.includes(code)
                    ? { kind: "unknown", code, readCode: null }
                    : { kind: "failed", code },
            );
        } finally {
            pending.current = false;
        }
    }

    /** Re-read the status after an unknown outcome. */
    async function refresh(): Promise<void> {
        if (pending.current || action.kind !== "unknown") return;
        const { code } = action;
        pending.current = true;
        setAction({ kind: "refreshing" });
        try {
            const result = await getProviderSecretStatus(providerId);
            if (!isAlive()) return;
            if (result.kind === "preview") {
                setRead({ kind: "preview" });
                setAction({ kind: "idle" });
            } else {
                setRead({ kind: "ready", status: result.status });
                // "Set" does not prove which key was saved; the text says so.
                setAction({ kind: "refreshed" });
            }
        } catch (error: unknown) {
            if (isAlive())
                setAction({
                    kind: "unknown",
                    code,
                    readCode: errorCode(error),
                });
        } finally {
            pending.current = false;
        }
    }

    function handleSubmit(event: SubmitEvent<HTMLFormElement>): void {
        event.preventDefault();
        void replace();
    }

    function stateText(): string {
        if (unknown) return t("providers.keys.unknown");
        switch (read.kind) {
            case "loading":
                return t("providers.keys.loading");
            case "preview":
                return t("providers.keys.preview");
            case "error":
                return t("providers.keys.unknown");
            case "ready":
                return read.status.updatedAtMs === null
                    ? t("providers.keys.missing")
                    : t("providers.keys.set", {
                          time: new Intl.DateTimeFormat(i18n.language, {
                              dateStyle: "medium",
                              timeStyle: "short",
                          }).format(read.status.updatedAtMs),
                      });
        }
    }

    function message(): JSX.Element | null {
        switch (action.kind) {
            case "replacing":
                return (
                    <p className={formStyles["message"]} role="status">
                        {missing
                            ? t("providers.keys.setting")
                            : t("providers.keys.replacing")}
                    </p>
                );
            case "refreshing":
                return (
                    <p className={formStyles["message"]} role="status">
                        {t("providers.keys.refreshing")}
                    </p>
                );
            case "refreshed":
                return (
                    <p className={formStyles["message"]} role="status">
                        {t("providers.keys.refreshed")}
                    </p>
                );
            case "unknown":
                return (
                    <>
                        <p className={formStyles["error"]} role="alert">
                            {replaceError(action.code)}
                        </p>
                        {action.readCode && (
                            <p className={formStyles["error"]} role="alert">
                                {readError(action.readCode)}
                            </p>
                        )}
                    </>
                );
            case "failed":
                return fieldError || topError ? null : (
                    <p className={formStyles["error"]} role="alert">
                        {replaceError(action.code)}
                    </p>
                );
            case "idle":
                return read.kind === "error" ? (
                    <p className={formStyles["error"]} role="alert">
                        {readError(read.code)}
                    </p>
                ) : null;
        }
    }

    const groupId = `${id}-keys`;
    const hintId = `${id}-secret-hint`;
    const errorId = `${id}-secret-error`;

    return (
        <section className={formStyles["group"]} aria-labelledby={groupId}>
            <h2 className={formStyles["groupTitle"]} id={groupId}>
                {t("providers.groups.keys")}
            </h2>
            <form
                aria-labelledby={groupId}
                aria-busy={working}
                noValidate
                onSubmit={handleSubmit}
            >
                {topError && (
                    <div className={formStyles["feedback"]}>
                        <p className={formStyles["error"]} role="alert">
                            {t(`providers.errors.${topError}`)}
                        </p>
                    </div>
                )}
                <div className={fieldStyles["group"]}>
                    <div className={fieldStyles["row"]}>
                        <span className={fieldStyles["label"]}>
                            {t("providers.keys.current")}
                        </span>
                        {/* A live region: loading, set, missing and unknown are
                            announced as they change, without moving focus. */}
                        <span
                            className={[formStyles["value"], styles["state"]]
                                .filter(
                                    (value): value is string =>
                                        value !== undefined,
                                )
                                .join(" ")}
                            role="status"
                        >
                            {stateText()}
                        </span>
                    </div>
                    <div className={fieldStyles["row"]}>
                        <label
                            className={fieldStyles["label"]}
                            htmlFor={`${id}-secret`}
                        >
                            {missing
                                ? t("providers.keys.secret")
                                : t("providers.keys.newSecret")}
                        </label>
                        <div className={formStyles["control"]}>
                            <input
                                className={formStyles["input"]}
                                id={`${id}-secret`}
                                ref={secretInput}
                                type="password"
                                // "off" is ignored for passwords; this keeps
                                // saved passwords from being filled in.
                                autoComplete="new-password"
                                autoCapitalize="none"
                                autoCorrect="off"
                                spellCheck={false}
                                value={secret}
                                readOnly={blocked}
                                aria-invalid={fieldError}
                                aria-describedby={
                                    fieldError ? `${hintId} ${errorId}` : hintId
                                }
                                onChange={(event) => {
                                    setSecret(event.currentTarget.value);
                                    setSecretMissing(false);
                                    if (action.kind === "failed")
                                        setAction({ kind: "idle" });
                                }}
                            />
                            <p className={formStyles["hint"]} id={hintId}>
                                {t("providers.keys.hint")}
                            </p>
                            {fieldError && (
                                <p
                                    className={formStyles["error"]}
                                    id={errorId}
                                    role="alert"
                                >
                                    {secretMissing
                                        ? t("providers.fields.secretRequired")
                                        : t("providers.errors.secret_invalid")}
                                </p>
                            )}
                        </div>
                    </div>
                </div>
                <div className={formStyles["feedback"]}>{message()}</div>
                <div className={formStyles["actions"]}>
                    {/* Secondary: the page's one primary action is the basic
                        information Save. Never `disabled`, so focus stays. */}
                    <button
                        className={buttons["secondary"]}
                        type="submit"
                        ref={submit}
                        aria-disabled={blocked}
                    >
                        {missing
                            ? t("providers.keys.setKey")
                            : t("providers.keys.replace")}
                    </button>
                    {unknown && (
                        <button
                            className={buttons["secondary"]}
                            type="button"
                            aria-disabled={working}
                            onClick={() => {
                                // This button disappears; keep focus in the group.
                                submit.current?.focus();
                                void refresh();
                            }}
                        >
                            {t("providers.keys.refresh")}
                        </button>
                    )}
                    {read.kind === "error" && (
                        <button
                            className={buttons["secondary"]}
                            type="button"
                            onClick={() => {
                                submit.current?.focus();
                                setRead({ kind: "loading" });
                                setReadAttempt((previous) => previous + 1);
                            }}
                        >
                            {t("providers.keys.retry")}
                        </button>
                    )}
                </div>
            </form>
        </section>
    );
}
