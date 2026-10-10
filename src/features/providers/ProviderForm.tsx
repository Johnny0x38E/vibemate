import {
    useEffect,
    useId,
    useRef,
    useState,
    type JSX,
    type SubmitEvent,
} from "react";
import { useTranslation } from "react-i18next";
import {
    createProvider,
    getProvider,
    ProviderRequestError,
    updateProvider,
    type ProviderErrorCode,
    type ProviderKind,
    type ProviderProtocol,
    type ProviderRecord,
    type ProviderTemplate,
} from "../../lib/desktop/providers";
import { getProviderSecretStatus } from "../../lib/desktop/providerSecrets";
import { FieldSelect } from "../../components/FieldSelect";
import { Icon } from "../../components/Icon";
import fieldStyles from "../settings/settingsField.module.css";
import { ProviderIcon } from "./ProviderIcon";
import buttons from "./providerButtons.module.css";
import styles from "./ProviderForm.module.css";

/**
 * Create a new instance (the provider is chosen in the form's first field), or
 * edit one saved instance by `id`, whose provider is then read-only.
 */
export type ProviderFormMode =
    { kind: "create" } | { kind: "edit"; record: ProviderRecord };

/** Inputs for {@link ProviderForm}. */
export interface ProviderFormProps {
    /** Rust-owned templates, used for the brand name and allowed protocols. */
    templates: readonly ProviderTemplate[];
    mode: ProviderFormMode;
    /**
     * Receives the record exactly as Rust stored it (normalized values). The
     * parent leaves this page; the form does not show the saved values itself.
     */
    onSaved: (record: ProviderRecord) => void;
    /** Receives a record re-read after a revision conflict. */
    onRecordLoaded: (record: ProviderRecord) => void;
    /**
     * Re-read the saved list after a save with an unknown outcome. Resolves
     * `true` when the list was read, so the user can check it before retrying.
     */
    onRefresh: () => Promise<boolean>;
    onCancel: () => void;
    /** Reports pending work so the page can block its back control meanwhile. */
    onBusyChange: (busy: boolean) => void;
    /**
     * True while the page is hidden (another app page is shown). The form stays
     * mounted and keeps its other fields, but a typed API key is cleared.
     */
    hidden?: boolean;
}

/**
 * The editable, non-secret fields. The API key of a new configuration is kept
 * apart, in its own state, so it is never copied with these fields.
 */
interface Fields {
    displayName: string;
    baseUrl: string;
    protocol: ProviderProtocol;
}

type FieldName = keyof Fields;

/** Fields that can show an error next to them in both create and edit modes. */
type ErrorField = FieldName | "secret";

/**
 * One state per workflow step, so "saving" and "failed" can never both hold.
 * `unknown` follows a save whose result could not be confirmed: the list is
 * re-read first, and only an explicit second action retries the save.
 */
type FormStatus =
    | { kind: "idle" | "saving" | "conflict" | "reloading" | "reloaded" }
    | { kind: "failed"; code: ProviderErrorCode }
    | { kind: "refreshing"; code: ProviderErrorCode; needsSecret: boolean }
    | {
          kind: "unknown";
          code: ProviderErrorCode;
          refreshed: boolean;
          needsSecret: boolean;
      }
    | { kind: "reloadFailed"; code: ProviderErrorCode };

/** Rust validation codes that belong next to one field rather than the form. */
const FIELD_FOR_CODE: Partial<Record<ProviderErrorCode, ErrorField>> = {
    display_name_invalid: "displayName",
    base_url_invalid: "baseUrl",
    base_url_not_https: "baseUrl",
    base_url_has_credentials: "baseUrl",
    protocol_not_supported: "protocol",
    secret_invalid: "secret",
};

/**
 * System credential store failures are about the device, not one field, so
 * they are shown at the top of the form where they are read first.
 */
const TOP_CODES: readonly ProviderErrorCode[] = [
    "credential_store_unavailable",
    "credential_store_access_denied",
    "credential_store_failed",
];

/**
 * Codes after which the save may or may not have happened. The list is re-read
 * before a retry is offered, so a second click cannot create a duplicate.
 */
const UNKNOWN_CODES: readonly ProviderErrorCode[] = [
    "operation_failed",
    "invalid_response",
    "create_outcome_unknown",
    "secret_outcome_unknown",
];

type KeyStatus =
    | { kind: "loading" }
    | { kind: "ready"; configured: boolean }
    | { kind: "error"; code: ProviderErrorCode }
    | { kind: "preview" };

function fieldsFromRecord(record: ProviderRecord): Fields {
    return {
        displayName: record.displayName,
        baseUrl: record.baseUrl,
        protocol: record.protocol,
    };
}

function errorCode(error: unknown): ProviderErrorCode {
    return error instanceof ProviderRequestError
        ? error.code
        : "operation_failed";
}

/**
 * The "Basic information" group of a provider configuration: name, base URL
 * and protocol, saved through Rust. A new configuration also takes its required
 * API key here. Editing uses the same field and save button: an empty field
 * preserves the stored key, and a supplied replacement is coordinated with settings.
 * The configured placeholder is fixed UI decoration, never a fetched secret.
 *
 * Edits send `expectedRevision`, so a concurrent change is reported instead of
 * overwritten. Rust is the only validator; its stable codes appear next to the
 * matching field. Controls are never `disabled` while work is pending, because
 * disabling the focused control drops focus to the page: buttons and the
 * select use `aria-disabled`, text fields `readOnly`, and handlers check state.
 */
export function ProviderForm({
    templates,
    mode,
    onSaved,
    onRecordLoaded,
    onRefresh,
    onCancel,
    onBusyChange,
    hidden = false,
}: ProviderFormProps): JSX.Element {
    const { t } = useTranslation();
    const id = useId();
    const editing = mode.kind === "edit" ? mode.record : null;
    // Only used when creating; an edit keeps the stored record's provider.
    const [createKind, setCreateKind] = useState<ProviderKind | undefined>(
        templates[0]?.kind,
    );
    const kind = editing ? editing.kind : createKind;
    const template = templates.find((entry) => entry.kind === kind);
    const [fields, setFields] = useState<Fields>(() =>
        editing
            ? fieldsFromRecord(editing)
            : {
                  // A brand name is a convenient start; names need not be unique.
                  displayName: template?.brandName ?? "",
                  baseUrl: template?.defaultBaseUrl ?? "",
                  protocol: template?.defaultProtocol ?? "chat_completions",
              },
    );
    // The revision this form last saw; edits must name it to be accepted.
    const [revision, setRevision] = useState(editing?.revision ?? 0);
    const [status, setStatus] = useState<FormStatus>({ kind: "idle" });
    // A new or replacement API key. It lives only in this state: it is
    // never logged, stored, put into a message or passed to the parent, and it
    // is cleared after every submit, on cancel and while the page is hidden.
    const [secret, setSecret] = useState("");
    // Set when a submit found the key empty; checked here, not by Rust.
    const [secretMissing, setSecretMissing] = useState(false);
    const editingId = editing?.id;
    const [keyStatus, setKeyStatus] = useState<KeyStatus>(
        editing ? { kind: "loading" } : { kind: "ready", configured: false },
    );
    const [keyAttempt, setKeyAttempt] = useState(0);

    useEffect(() => {
        if (editingId === undefined) return;
        // Only SQLite status is read, never the key or the OS credential store.
        // A per-effect flag rejects obsolete StrictMode reads and unmounted results.
        let active = true;
        getProviderSecretStatus(editingId).then(
            (result) => {
                if (!active) return;
                setKeyStatus(
                    result.kind === "preview"
                        ? { kind: "preview" }
                        : {
                              kind: "ready",
                              configured: result.status.state === "set",
                          },
                );
            },
            (error: unknown) => {
                if (active)
                    setKeyStatus({ kind: "error", code: errorCode(error) });
            },
        );
        return () => {
            active = false;
        };
    }, [editingId, keyAttempt]);
    // Clearing on hide is derived during render (not in an effect), so a key
    // never survives even one hidden frame.
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
        // form. StrictMode runs this setup twice, so it resets the flag too.
        return () => {
            alive.current = false;
        };
    }, []);

    // Read the mutable ref afresh after each await; cleanup can change it.
    function isAlive(): boolean {
        return alive.current;
    }

    // An edited record keeps its stored protocol even if no template lists it.
    const protocols = template?.protocols ?? [fields.protocol];
    const busy =
        status.kind === "saving" ||
        status.kind === "refreshing" ||
        status.kind === "reloading";
    const saveBlocked =
        busy ||
        status.kind === "conflict" ||
        status.kind === "reloadFailed" ||
        status.kind === "unknown" ||
        keyStatus.kind !== "ready";
    const fieldError: ErrorField | undefined = secretMissing
        ? "secret"
        : status.kind === "failed"
          ? FIELD_FOR_CODE[status.code]
          : undefined;
    const topError =
        status.kind === "failed" && TOP_CODES.includes(status.code)
            ? status.code
            : undefined;

    useEffect(() => {
        onBusyChange(busy);
        return () => {
            onBusyChange(false);
        };
    }, [busy, onBusyChange]);

    function update(name: FieldName, value: string): void {
        if (name === "protocol") {
            const protocol = protocols.find((entry) => entry === value);
            if (!protocol) return;
            setFields((previous) => ({ ...previous, protocol }));
        } else {
            setFields((previous) => ({ ...previous, [name]: value }));
        }
        // A definite failure no longer describes the edited values.
        if (status.kind === "failed") setStatus({ kind: "idle" });
    }

    /**
     * Switch the provider of a new configuration. Values that still equal the
     * previous provider's defaults take the new defaults; anything the user
     * changed is kept, except a protocol the new provider does not allow.
     */
    function changeKind(value: string): void {
        if (saveBlocked) return;
        const next = templates.find((entry) => entry.kind === value);
        if (!next) return;
        setCreateKind(next.kind);
        setFields((previous) => ({
            displayName:
                previous.displayName === template?.brandName
                    ? next.brandName
                    : previous.displayName,
            baseUrl:
                previous.baseUrl === template?.defaultBaseUrl
                    ? next.defaultBaseUrl
                    : previous.baseUrl,
            protocol:
                previous.protocol !== template?.defaultProtocol &&
                next.protocols.includes(previous.protocol)
                    ? previous.protocol
                    : next.defaultProtocol,
        }));
        if (status.kind === "failed") setStatus({ kind: "idle" });
    }

    async function save(confirmedRetry: boolean): Promise<void> {
        // Take the key out of the field before anything else: every submit
        // clears it, including one that is ignored below (a plain Enter after
        // an unknown outcome, or a second submit while saving).
        const key = secret;
        setSecret("");
        // The saved record to update, or the provider of a new configuration.
        const target = editing ?? createKind;
        if (pending.current || target === undefined) return;
        if (saveBlocked && !(confirmedRetry && status.kind === "unknown"))
            return;
        const requiresKey =
            typeof target === "string" ||
            (keyStatus.kind === "ready" && !keyStatus.configured) ||
            (confirmedRetry && status.kind === "unknown" && status.needsSecret);
        if (requiresKey && key.trim() === "") {
            // The only check made here; Rust validates everything else. An
            // unknown outcome stays in place, so its retry rules still apply.
            setSecretMissing(true);
            secretInput.current?.focus();
            return;
        }
        setSecretMissing(false);
        // The ref closes the gap before React renders the blocked state.
        pending.current = true;
        setStatus({ kind: "saving" });
        try {
            const record =
                typeof target === "string"
                    ? await createProvider({
                          kind: target,
                          ...fields,
                          extensions: {},
                          secret: key,
                      })
                    : await updateProvider({
                          id: target.id,
                          expectedRevision: revision,
                          ...fields,
                          extensions: {},
                          ...(key !== "" ? { secret: key } : {}),
                      });
            if (!isAlive()) return;
            setStatus({ kind: "idle" });
            // The parent shows Rust's normalized values and leaves this page.
            onSaved(record);
        } catch (error: unknown) {
            if (!isAlive()) return;
            const code = errorCode(error);
            if (UNKNOWN_CODES.includes(code)) {
                // The write may have happened. Re-read the list before offering a
                // retry, so a second click cannot silently create a duplicate. The
                // key was cleared, so that retry needs it typed again.
                const needsSecret = typeof target === "string" || key !== "";
                setStatus({ kind: "refreshing", code, needsSecret });
                const refreshed = await onRefresh();
                if (isAlive())
                    setStatus({
                        kind: "unknown",
                        code,
                        refreshed,
                        needsSecret,
                    });
            } else if (code === "revision_conflict") {
                setStatus({ kind: "conflict" });
            } else {
                setStatus({ kind: "failed", code });
            }
        } finally {
            pending.current = false;
        }
    }

    async function refreshAgain(): Promise<void> {
        if (pending.current || status.kind !== "unknown") return;
        pending.current = true;
        const { code, needsSecret } = status;
        setStatus({ kind: "refreshing", code, needsSecret });
        try {
            const refreshed = await onRefresh();
            if (isAlive())
                setStatus({ kind: "unknown", code, refreshed, needsSecret });
        } finally {
            pending.current = false;
        }
    }

    async function reloadLatest(): Promise<void> {
        if (!editing || pending.current) return;
        pending.current = true;
        setStatus({ kind: "reloading" });
        try {
            const record = await getProvider(editing.id);
            if (!isAlive()) return;
            // Replace the form with the newer saved values; the user reviews them
            // before saving again, so another change is never overwritten blindly.
            setFields(fieldsFromRecord(record));
            setRevision(record.revision);
            setStatus({ kind: "reloaded" });
            onRecordLoaded(record);
        } catch (error: unknown) {
            if (isAlive())
                setStatus({ kind: "reloadFailed", code: errorCode(error) });
        } finally {
            pending.current = false;
        }
    }

    function handleSubmit(event: SubmitEvent<HTMLFormElement>): void {
        event.preventDefault();
        // save handles every rejection and ignores results after unmount.
        void save(false);
    }

    function updateSecret(value: string): void {
        setSecret(value);
        setSecretMissing(false);
        if (status.kind === "failed") setStatus({ kind: "idle" });
    }

    function describedBy(
        name: ErrorField,
        hintId?: string,
    ): string | undefined {
        const ids = [hintId, fieldError === name ? `${id}-${name}-error` : ""]
            .filter((value) => value)
            .join(" ");
        return ids || undefined;
    }

    function errorText(name: ErrorField): JSX.Element | null {
        if (fieldError !== name) return null;
        let message: string;
        if (secretMissing) message = t("providers.fields.secretRequired");
        else if (status.kind === "failed")
            message = t(`providers.errors.${status.code}`);
        else return null;
        return (
            <p
                className={styles["error"]}
                id={`${id}-${name}-error`}
                role="alert"
            >
                {message}
            </p>
        );
    }

    function formMessage(): JSX.Element | null {
        switch (status.kind) {
            case "failed":
                // Field errors sit by their field, store errors at the top.
                return fieldError || topError ? null : (
                    <p className={styles["error"]} role="alert">
                        {t(`providers.errors.${status.code}`)}
                    </p>
                );
            case "saving":
                return (
                    <p className={styles["message"]} role="status">
                        {t("providers.form.saving")}
                    </p>
                );
            case "refreshing":
                return (
                    <p className={styles["message"]} role="status">
                        {t("providers.form.refreshing")}
                    </p>
                );
            case "unknown":
                return (
                    <>
                        <p className={styles["error"]} role="alert">
                            {t(`providers.errors.${status.code}`)}
                        </p>
                        <p className={styles["message"]}>
                            {!status.refreshed
                                ? t("providers.form.refreshFailed")
                                : editing
                                  ? t(
                                        status.needsSecret
                                            ? "providers.form.editUnknownSecret"
                                            : "providers.form.editUnknown",
                                    )
                                  : t("providers.form.unknownRefreshedSecret")}
                        </p>
                    </>
                );
            case "conflict":
                return (
                    <p className={styles["error"]} role="alert">
                        {t("providers.errors.revision_conflict")}
                    </p>
                );
            case "reloading":
                return (
                    <p className={styles["message"]} role="status">
                        {t("providers.form.reloading")}
                    </p>
                );
            case "reloaded":
                return (
                    <p className={styles["message"]} role="status">
                        {t("providers.form.reloaded")}
                    </p>
                );
            case "reloadFailed":
                return (
                    <p className={styles["error"]} role="alert">
                        {t(`providers.errors.${status.code}`)}
                    </p>
                );
            case "idle":
                return null;
        }
    }

    const groupId = `${id}-basic`;
    const hintId = `${id}-baseUrl-hint`;
    const secretStatusId = `${id}-secret-status`;
    const configured = keyStatus.kind === "ready" && keyStatus.configured;
    const showStoredIndicator = configured && secret === "";

    return (
        <section className={styles["group"]} aria-labelledby={groupId}>
            <h2 className={styles["groupTitle"]} id={groupId}>
                {t("providers.groups.basic")}
            </h2>
            <form
                aria-labelledby={groupId}
                aria-busy={busy}
                noValidate
                onSubmit={handleSubmit}
            >
                {topError && (
                    <div className={styles["feedback"]}>
                        <p className={styles["error"]} role="alert">
                            {t(`providers.errors.${topError}`)}
                        </p>
                    </div>
                )}
                <div className={fieldStyles["group"]}>
                    {/* The provider is part of the instance identity: chosen when
                        creating, then fixed, so an edit shows it read-only. */}
                    {editing ? (
                        <div className={fieldStyles["row"]}>
                            <span className={fieldStyles["label"]}>
                                {t("providers.fields.kind")}
                            </span>
                            <span
                                className={[
                                    styles["value"],
                                    styles["valueWithIcon"],
                                ]
                                    .filter(
                                        (value): value is string =>
                                            value !== undefined,
                                    )
                                    .join(" ")}
                            >
                                <ProviderIcon kind={editing.kind} />
                                {template?.brandName ?? editing.kind}
                            </span>
                        </div>
                    ) : (
                        <div className={fieldStyles["row"]}>
                            <label
                                className={fieldStyles["label"]}
                                htmlFor={`${id}-kind`}
                            >
                                {t("providers.fields.kind")}
                            </label>
                            <div className={styles["control"]}>
                                {createKind !== undefined && (
                                    <FieldSelect
                                        id={`${id}-kind`}
                                        value={createKind}
                                        blocked={saveBlocked}
                                        data-test-value={createKind}
                                        options={templates.map((entry) => ({
                                            value: entry.kind,
                                            label: entry.brandName,
                                            leading: (
                                                <ProviderIcon
                                                    kind={entry.kind}
                                                />
                                            ),
                                        }))}
                                        onChange={changeKind}
                                    />
                                )}
                            </div>
                        </div>
                    )}
                    {editing && (
                        <div className={fieldStyles["row"]}>
                            <span className={fieldStyles["label"]}>
                                {t("providers.fields.id")}
                            </span>
                            <span className={styles["secondaryValue"]}>
                                {editing.id}
                            </span>
                        </div>
                    )}
                    <div className={fieldStyles["row"]}>
                        <label
                            className={fieldStyles["label"]}
                            htmlFor={`${id}-displayName`}
                        >
                            {t("providers.fields.displayName")}
                        </label>
                        <div className={styles["control"]}>
                            <input
                                className={styles["input"]}
                                id={`${id}-displayName`}
                                type="text"
                                autoComplete="off"
                                value={fields.displayName}
                                readOnly={saveBlocked}
                                aria-invalid={fieldError === "displayName"}
                                aria-describedby={describedBy("displayName")}
                                onChange={(event) => {
                                    update(
                                        "displayName",
                                        event.currentTarget.value,
                                    );
                                }}
                            />
                            {errorText("displayName")}
                        </div>
                    </div>
                    <div className={fieldStyles["row"]}>
                        <label
                            className={fieldStyles["label"]}
                            htmlFor={`${id}-baseUrl`}
                        >
                            {t("providers.fields.baseUrl")}
                        </label>
                        <div className={styles["control"]}>
                            <input
                                className={styles["input"]}
                                id={`${id}-baseUrl`}
                                type="url"
                                inputMode="url"
                                autoComplete="off"
                                spellCheck={false}
                                value={fields.baseUrl}
                                readOnly={saveBlocked}
                                aria-invalid={fieldError === "baseUrl"}
                                aria-describedby={describedBy(
                                    "baseUrl",
                                    hintId,
                                )}
                                onChange={(event) => {
                                    update(
                                        "baseUrl",
                                        event.currentTarget.value,
                                    );
                                }}
                            />
                            <p className={styles["hint"]} id={hintId}>
                                {t("providers.fields.baseUrlHint")}
                            </p>
                            {errorText("baseUrl")}
                        </div>
                    </div>
                    <div className={fieldStyles["row"]}>
                        <label
                            className={fieldStyles["label"]}
                            htmlFor={`${id}-protocol`}
                        >
                            {t("providers.fields.protocol")}
                        </label>
                        <div className={styles["control"]}>
                            <FieldSelect
                                id={`${id}-protocol`}
                                value={fields.protocol}
                                blocked={saveBlocked}
                                aria-invalid={fieldError === "protocol"}
                                aria-describedby={describedBy("protocol")}
                                options={protocols.map((protocol) => ({
                                    value: protocol,
                                    label: t(`providers.protocols.${protocol}`),
                                }))}
                                onChange={(next) => {
                                    if (saveBlocked) return;
                                    update("protocol", next);
                                }}
                            />
                            {errorText("protocol")}
                        </div>
                    </div>
                    {/* The fixed placeholder and eye-off mark only indicate a
                        configured entry; neither is an input value or a reveal control. */}
                    <div className={fieldStyles["row"]}>
                        <label
                            className={fieldStyles["label"]}
                            htmlFor={`${id}-secret`}
                        >
                            {t("providers.fields.secret")}
                        </label>
                        <div className={styles["secretControl"]}>
                            <input
                                className={styles["input"]}
                                id={`${id}-secret`}
                                ref={secretInput}
                                type="password"
                                // "off" is ignored for passwords; this keeps
                                // saved passwords from being filled in.
                                autoComplete="new-password"
                                autoCapitalize="none"
                                autoCorrect="off"
                                spellCheck={false}
                                required={!configured}
                                placeholder={
                                    showStoredIndicator ? "••••••••" : undefined
                                }
                                value={secret}
                                // Editable after an unknown outcome, so the
                                // key can be typed again for a retry.
                                readOnly={busy}
                                aria-invalid={fieldError === "secret"}
                                aria-describedby={describedBy(
                                    "secret",
                                    showStoredIndicator
                                        ? secretStatusId
                                        : undefined,
                                )}
                                onChange={(event) => {
                                    updateSecret(event.currentTarget.value);
                                }}
                            />
                            {showStoredIndicator && (
                                <span
                                    className={styles["keyIndicator"]}
                                    title={t(
                                        "providers.fields.secretConfigured",
                                    )}
                                >
                                    <Icon name="eyeOff" />
                                    <span
                                        className={styles["visuallyHidden"]}
                                        id={secretStatusId}
                                    >
                                        {t("providers.fields.secretConfigured")}
                                    </span>
                                </span>
                            )}
                            {keyStatus.kind === "loading" && (
                                <p className={styles["message"]} role="status">
                                    {t("providers.fields.secretLoading")}
                                </p>
                            )}
                            {keyStatus.kind === "error" && (
                                <>
                                    <p className={styles["error"]} role="alert">
                                        {t(
                                            `providers.errors.${keyStatus.code}`,
                                        )}
                                    </p>
                                    <button
                                        type="button"
                                        className={buttons["secondary"]}
                                        onClick={() => {
                                            // The retry button disappears; keep focus in the key field.
                                            secretInput.current?.focus();
                                            setKeyStatus({ kind: "loading" });
                                            setKeyAttempt(
                                                (attempt) => attempt + 1,
                                            );
                                        }}
                                    >
                                        {t("providers.fields.secretRetry")}
                                    </button>
                                </>
                            )}
                            {keyStatus.kind === "preview" && (
                                <p className={styles["message"]}>
                                    {t("providers.errors.desktop_required")}
                                </p>
                            )}
                            {errorText("secret")}
                        </div>
                    </div>
                </div>
                <div className={styles["feedback"]}>{formMessage()}</div>
                <div className={styles["actions"]}>
                    {/* The one primary action. Never `disabled`: focus stays here
                        while saving, and save() checks the state itself. */}
                    <button
                        className={buttons["primary"]}
                        type="submit"
                        ref={submit}
                        aria-disabled={saveBlocked}
                    >
                        {t("providers.form.save")}
                    </button>
                    {status.kind === "unknown" && status.refreshed && (
                        <button
                            className={buttons["secondary"]}
                            type="button"
                            onClick={() => {
                                // This button disappears; keep focus inside the form.
                                submit.current?.focus();
                                // An explicit second action after the list was re-read.
                                void save(true);
                            }}
                        >
                            {t("providers.form.saveAgain")}
                        </button>
                    )}
                    {status.kind === "unknown" && !status.refreshed && (
                        <button
                            className={buttons["secondary"]}
                            type="button"
                            onClick={() => {
                                submit.current?.focus();
                                void refreshAgain();
                            }}
                        >
                            {t("providers.form.refreshList")}
                        </button>
                    )}
                    {(status.kind === "conflict" ||
                        status.kind === "reloadFailed") && (
                        <button
                            className={buttons["secondary"]}
                            type="button"
                            onClick={() => {
                                submit.current?.focus();
                                void reloadLatest();
                            }}
                        >
                            {t("providers.form.reloadLatest")}
                        </button>
                    )}
                    <button
                        className={buttons["secondary"]}
                        type="button"
                        aria-disabled={busy}
                        onClick={() => {
                            if (busy) return;
                            setSecret("");
                            onCancel();
                        }}
                    >
                        {t("providers.form.cancel")}
                    </button>
                </div>
            </form>
        </section>
    );
}
