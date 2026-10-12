import {
    useEffect,
    useId,
    useRef,
    useState,
    type JSX,
    type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { EditFormRow } from "../../components/EditFormRow";
import { FieldSelect } from "../../components/FieldSelect";
import { ModelCheckbox } from "../../components/ModelCheckbox";
import { IconButton } from "../../components/IconButton";
import editForm from "../../components/editForm.module.css";
import { ProviderPage } from "../providers/ProviderPage";
import {
    McpRequestError,
    saveMcpDefinition,
    type McpErrorCode,
    type McpRecord,
    type McpSecretInput,
    type SaveMcpInput,
} from "../../lib/desktop/mcp";
import styles from "./Mcp.module.css";
import { SecondaryPageActions } from "../../components/SecondaryPageActions";
import buttons from "../../components/actionButtons.module.css";
import { deriveServerName, serverNameForAttempt } from "./deriveServerName";

type FormState =
    | { kind: "idle" | "saving" }
    | { kind: "error"; code: McpErrorCode; uncertain: boolean };
interface FieldDraft {
    key: number;
    name: string;
    value: string;
    configured: boolean;
}
interface ArgDraft {
    key: number;
    value: string;
}
function argRows(values: string[]): ArgDraft[] {
    return values.map((value, key) => ({ key, value }));
}
/** Editor inputs; caller keys by record/revision to make explicit reloads a new form. */
export interface McpFormProps {
    record: McpRecord | null;
    hidden: boolean;
    onSaved: (record: McpRecord) => void;
    onCancel: () => void;
    onReload: () => void;
    /** Protect sidebar home navigation during writes or unresolved outcomes. */
    onNavigationBlockedChange?: (blocked: boolean) => void;
}
function fields(names: string[]): FieldDraft[] {
    return names.map((name, key) => ({
        key,
        name,
        value: "",
        configured: true,
    }));
}
/**
 * Central definition editor. Secret inputs are transient and cleared after every
 * submit or hide. Stored values never enter React; blank existing values retain
 * references. An unknown result blocks resubmission until the caller reloads.
 */
export function McpForm({
    record,
    hidden,
    onSaved,
    onCancel,
    onReload,
    onNavigationBlockedChange,
}: McpFormProps): JSX.Element {
    const { t } = useTranslation();
    const prefix = useId();
    const [displayName, setDisplayName] = useState(record?.displayName ?? "");
    const [enabled, setEnabled] = useState(record?.enabled ?? true);
    const [transport, setTransport] = useState<"stdio" | "http">(
        record?.connection.type ?? "stdio",
    );
    const existing = record?.connection;
    const [command, setCommand] = useState(
        existing?.type === "stdio" ? existing.command : "",
    );
    const [args, setArgs] = useState<ArgDraft[]>(() =>
        existing?.type === "stdio" ? argRows(existing.args) : [],
    );
    const [cwd, setCwd] = useState(
        existing?.type === "stdio" ? (existing.cwd ?? "") : "",
    );
    const [url, setUrl] = useState(
        existing?.type === "http" ? existing.url : "",
    );
    const [env, setEnv] = useState<FieldDraft[]>(() =>
        fields(existing?.type === "stdio" ? existing.env : []),
    );
    const [headers, setHeaders] = useState<FieldDraft[]>(() =>
        fields(existing?.type === "http" ? existing.headers : []),
    );
    const nextKey = useRef(32);
    const pending = useRef(false);
    const active = useRef(true);
    const [state, setState] = useState<FormState>({ kind: "idle" });
    const busy = state.kind === "saving";
    const uncertain = state.kind === "error" && state.uncertain;
    const locked = busy || uncertain;
    const code = state.kind === "error" ? state.code : null;
    const errorId = `${prefix}-error`;
    const currentFields = transport === "stdio" ? env : headers;
    const setFields = transport === "stdio" ? setEnv : setHeaders;
    // Like page Back, sidebar home must not abandon a pending/uncertain save.
    useEffect(() => {
        onNavigationBlockedChange?.(locked);
        return () => {
            onNavigationBlockedChange?.(false);
        };
    }, [locked, onNavigationBlockedChange]);
    useEffect(() => {
        active.current = true;
        return () => {
            active.current = false;
        };
    }, []);
    // Match ProviderForm's render-time hide boundary: even a hide/reveal in one
    // event cycle must clear typed secrets before a hidden frame is committed.
    const [wasHidden, setWasHidden] = useState(hidden);
    if (hidden !== wasHidden) {
        setWasHidden(hidden);
        if (hidden) {
            setEnv((rows) => rows.map((row) => ({ ...row, value: "" })));
            setHeaders((rows) => rows.map((row) => ({ ...row, value: "" })));
        }
    }
    function clearValues(): void {
        setEnv((rows) => rows.map((row) => ({ ...row, value: "" })));
        setHeaders((rows) => rows.map((row) => ({ ...row, value: "" })));
    }
    function secretInputs(rows: FieldDraft[]): McpSecretInput[] {
        return rows.map((row) => ({
            name: row.name,
            value: row.value === "" ? null : row.value,
        }));
    }
    function fieldError(expected: McpErrorCode): {
        "aria-invalid": boolean;
        "aria-describedby": string | undefined;
    } {
        return {
            "aria-invalid": code === expected,
            "aria-describedby": code === expected ? errorId : undefined,
        };
    }
    function displayNameInputA11y(): {
        "aria-invalid": boolean;
        "aria-describedby": string | undefined;
    } {
        if (code === null) {
            return { "aria-invalid": false, "aria-describedby": undefined };
        }
        if (code === "display_name_invalid") {
            return fieldError("display_name_invalid");
        }
        if (
            record === null &&
            (code === "server_name_taken" || code === "server_name_invalid")
        ) {
            return { "aria-invalid": true, "aria-describedby": errorId };
        }
        return { "aria-invalid": false, "aria-describedby": undefined };
    }
    function buildRequest(serverName: string): SaveMcpInput {
        return {
            id: record?.id ?? null,
            expectedRevision: record?.revision ?? null,
            displayName,
            serverName,
            enabled,
            connection:
                transport === "stdio"
                    ? {
                          type: "stdio",
                          command,
                          args: args.map((row) => row.value),
                          cwd: cwd.trim() === "" ? null : cwd,
                          env: secretInputs(env),
                      }
                    : { type: "http", url, headers: secretInputs(headers) },
        };
    }
    async function submit(): Promise<void> {
        if (locked || pending.current || hidden) return;
        pending.current = true;
        clearValues();
        setState({ kind: "saving" });
        const creating = record === null;
        const baseId = creating
            ? deriveServerName(displayName)
            : record.serverName;
        try {
            let saved: McpRecord | null = null;
            for (let attempt = 0; attempt < 32; attempt += 1) {
                const serverName = creating
                    ? serverNameForAttempt(baseId, attempt)
                    : record.serverName;
                try {
                    saved = await saveMcpDefinition(buildRequest(serverName));
                    break;
                } catch (error: unknown) {
                    if (
                        creating &&
                        error instanceof McpRequestError &&
                        error.code === "server_name_taken"
                    ) {
                        continue;
                    }
                    throw error;
                }
            }
            if (saved === null) {
                throw new McpRequestError("server_name_taken");
            }
            if (active.current) onSaved(saved);
        } catch (error: unknown) {
            if (!active.current) return;
            const safe =
                error instanceof McpRequestError
                    ? error.code
                    : "operation_failed";
            setState({
                kind: "error",
                code: safe,
                uncertain: [
                    "outcome_unknown",
                    "operation_failed",
                    "invalid_response",
                    "revision_conflict",
                ].includes(safe),
            });
        } finally {
            pending.current = false;
        }
    }
    function updateField(
        key: number,
        property: "name" | "value",
        value: string,
    ): void {
        if (locked) return;
        setFields((rows) =>
            rows.map((row) =>
                row.key === key
                    ? {
                          ...row,
                          [property]: value,
                          configured:
                              property === "name" ? false : row.configured,
                      }
                    : row,
            ),
        );
    }
    function leavePage(): void {
        clearValues();
        if (uncertain) onReload();
        else onCancel();
    }
    function uniformRow(
        label: ReactNode,
        control: ReactNode,
        options?: { section?: boolean },
    ): JSX.Element {
        return (
            <EditFormRow
                label={label}
                {...(options?.section
                    ? { className: styles["rowSection"] }
                    : {})}
            >
                {control}
            </EditFormRow>
        );
    }
    const pageTitle =
        record === null
            ? t("mcp.create")
            : displayName.trim() === ""
              ? t("mcp.edit")
              : displayName;
    const formId = `${prefix}-form`;
    return (
        <div className={styles["editor"]}>
            <ProviderPage
                title={pageTitle}
                onBack={leavePage}
                backBlocked={busy}
                backAriaLabel={t("mcp.backLabel")}
            >
                <section className={styles["editorGroup"]} data-layout="page">
                    <form
                        id={formId}
                        className={styles["editorForm"]}
                        aria-label={pageTitle}
                        aria-busy={busy}
                        noValidate
                        onSubmit={(event) => {
                            event.preventDefault();
                            void submit();
                        }}
                    >
                        <div className={styles["editorContent"]}>
                            <div
                                className={[
                                    editForm["group"],
                                    editForm["groupFlush"],
                                ].join(" ")}
                            >
                                {uniformRow(
                                    <label
                                        className={editForm["label"]}
                                        htmlFor={`${prefix}-display-name`}
                                    >
                                        {t("mcp.displayName")}
                                    </label>,
                                    <input
                                        id={`${prefix}-display-name`}
                                        className={editForm["input"]}
                                        value={displayName}
                                        readOnly={locked}
                                        autoComplete="off"
                                        {...displayNameInputA11y()}
                                        onChange={(event) => {
                                            setDisplayName(
                                                event.currentTarget.value,
                                            );
                                        }}
                                    />,
                                )}
                                {uniformRow(
                                    <label
                                        className={editForm["label"]}
                                        htmlFor={`${prefix}-transport`}
                                    >
                                        {t("mcp.transport")}
                                    </label>,
                                    <FieldSelect
                                        id={`${prefix}-transport`}
                                        value={transport}
                                        blocked={locked}
                                        options={[
                                            {
                                                value: "stdio",
                                                label: t("mcp.stdio"),
                                            },
                                            {
                                                value: "http",
                                                label: t("mcp.http"),
                                            },
                                        ]}
                                        onChange={(value) => {
                                            if (
                                                value === "stdio" ||
                                                value === "http"
                                            ) {
                                                clearValues();
                                                setTransport(value);
                                            }
                                        }}
                                    />,
                                )}
                                {transport === "stdio" ? (
                                    <>
                                        {uniformRow(
                                            <label
                                                className={editForm["label"]}
                                                htmlFor={`${prefix}-command`}
                                            >
                                                {t("mcp.command")}
                                            </label>,
                                            <input
                                                id={`${prefix}-command`}
                                                className={editForm["input"]}
                                                value={command}
                                                readOnly={locked}
                                                {...fieldError(
                                                    "command_invalid",
                                                )}
                                                autoComplete="off"
                                                onChange={(event) => {
                                                    setCommand(
                                                        event.currentTarget
                                                            .value,
                                                    );
                                                }}
                                            />,
                                        )}
                                        {uniformRow(
                                            t("mcp.args"),
                                            <div
                                                className={
                                                    styles["sectionControl"]
                                                }
                                            >
                                                {args.map((row, index) => (
                                                    <div
                                                        className={
                                                            styles["argRow"]
                                                        }
                                                        key={row.key}
                                                    >
                                                        <label
                                                            className={
                                                                editForm[
                                                                    "label"
                                                                ]
                                                            }
                                                            htmlFor={`${prefix}-arg-${String(row.key)}`}
                                                        >
                                                            {t(
                                                                "mcp.arg_value",
                                                                {
                                                                    index:
                                                                        index +
                                                                        1,
                                                                },
                                                            )}
                                                        </label>
                                                        <input
                                                            id={`${prefix}-arg-${String(row.key)}`}
                                                            className={
                                                                editForm[
                                                                    "input"
                                                                ]
                                                            }
                                                            value={row.value}
                                                            readOnly={locked}
                                                            {...fieldError(
                                                                "args_invalid",
                                                            )}
                                                            autoComplete="off"
                                                            onChange={(
                                                                event,
                                                            ) => {
                                                                const value =
                                                                    event
                                                                        .currentTarget
                                                                        .value;
                                                                setArgs(
                                                                    (rows) =>
                                                                        rows.map(
                                                                            (
                                                                                item,
                                                                            ) =>
                                                                                item.key ===
                                                                                row.key
                                                                                    ? {
                                                                                          ...item,
                                                                                          value,
                                                                                      }
                                                                                    : item,
                                                                        ),
                                                                );
                                                            }}
                                                        />
                                                        <IconButton
                                                            icon="trash"
                                                            destructive
                                                            blocked={locked}
                                                            label={t(
                                                                "mcp.remove_argument",
                                                                {
                                                                    index:
                                                                        index +
                                                                        1,
                                                                },
                                                            )}
                                                            onClick={() => {
                                                                if (!locked) {
                                                                    setArgs(
                                                                        (
                                                                            rows,
                                                                        ) =>
                                                                            rows.filter(
                                                                                (
                                                                                    item,
                                                                                ) =>
                                                                                    item.key !==
                                                                                    row.key,
                                                                            ),
                                                                    );
                                                                }
                                                            }}
                                                        />
                                                    </div>
                                                ))}
                                                <div
                                                    className={
                                                        styles["sectionAddArg"]
                                                    }
                                                >
                                                    <IconButton
                                                        icon="plus"
                                                        label={t(
                                                            "mcp.add_argument",
                                                        )}
                                                        blocked={
                                                            locked ||
                                                            args.length >= 64
                                                        }
                                                        onClick={() => {
                                                            if (
                                                                locked ||
                                                                args.length >=
                                                                    64
                                                            )
                                                                return;
                                                            const key =
                                                                nextKey.current++;
                                                            setArgs((rows) => [
                                                                ...rows,
                                                                {
                                                                    key,
                                                                    value: "",
                                                                },
                                                            ]);
                                                        }}
                                                    />
                                                </div>
                                            </div>,
                                            { section: true },
                                        )}
                                        {uniformRow(
                                            <label
                                                className={editForm["label"]}
                                                htmlFor={`${prefix}-cwd`}
                                            >
                                                {t("mcp.cwd")}
                                            </label>,
                                            <input
                                                id={`${prefix}-cwd`}
                                                className={editForm["input"]}
                                                value={cwd}
                                                readOnly={locked}
                                                {...fieldError("cwd_invalid")}
                                                autoComplete="off"
                                                onChange={(event) => {
                                                    setCwd(
                                                        event.currentTarget
                                                            .value,
                                                    );
                                                }}
                                            />,
                                        )}
                                    </>
                                ) : (
                                    uniformRow(
                                        <label
                                            className={editForm["label"]}
                                            htmlFor={`${prefix}-url`}
                                        >
                                            {t("mcp.url")}
                                        </label>,
                                        <>
                                            <input
                                                id={`${prefix}-url`}
                                                className={editForm["input"]}
                                                value={url}
                                                readOnly={locked}
                                                {...fieldError("url_invalid")}
                                                autoComplete="off"
                                                onChange={(event) => {
                                                    setUrl(
                                                        event.currentTarget
                                                            .value,
                                                    );
                                                }}
                                            />
                                            <p className={editForm["hint"]}>
                                                {t("mcp.urlHint")}
                                            </p>
                                        </>,
                                    )
                                )}
                                {uniformRow(
                                    t(
                                        transport === "stdio"
                                            ? "mcp.env"
                                            : "mcp.headers",
                                    ),
                                    <div className={styles["sectionControl"]}>
                                        {currentFields.map((row, index) => (
                                            <div
                                                className={styles["secretRow"]}
                                                key={row.key}
                                            >
                                                <label
                                                    className={
                                                        editForm["label"]
                                                    }
                                                    htmlFor={`${prefix}-secret-name-${String(row.key)}`}
                                                >
                                                    {t("mcp.fieldName", {
                                                        index: index + 1,
                                                    })}
                                                </label>
                                                <label
                                                    className={
                                                        editForm["label"]
                                                    }
                                                    htmlFor={`${prefix}-secret-value-${String(row.key)}`}
                                                >
                                                    {t("mcp.fieldValue", {
                                                        index: index + 1,
                                                    })}
                                                </label>
                                                <span
                                                    className={
                                                        styles[
                                                            "secretRemoveSlot"
                                                        ]
                                                    }
                                                    aria-hidden="true"
                                                />
                                                <input
                                                    id={`${prefix}-secret-name-${String(row.key)}`}
                                                    className={
                                                        editForm["input"]
                                                    }
                                                    value={row.name}
                                                    readOnly={locked}
                                                    {...fieldError(
                                                        "field_name_invalid",
                                                    )}
                                                    spellCheck={false}
                                                    autoComplete="off"
                                                    onChange={(event) => {
                                                        updateField(
                                                            row.key,
                                                            "name",
                                                            event.currentTarget
                                                                .value,
                                                        );
                                                    }}
                                                />
                                                <input
                                                    id={`${prefix}-secret-value-${String(row.key)}`}
                                                    className={
                                                        editForm["input"]
                                                    }
                                                    type="password"
                                                    value={row.value}
                                                    readOnly={locked}
                                                    aria-invalid={
                                                        code ===
                                                            "secret_required" ||
                                                        code ===
                                                            "secret_invalid"
                                                    }
                                                    aria-describedby={
                                                        code ===
                                                            "secret_required" ||
                                                        code ===
                                                            "secret_invalid"
                                                            ? errorId
                                                            : undefined
                                                    }
                                                    autoComplete="new-password"
                                                    onChange={(event) => {
                                                        updateField(
                                                            row.key,
                                                            "value",
                                                            event.currentTarget
                                                                .value,
                                                        );
                                                    }}
                                                />
                                                <IconButton
                                                    icon="trash"
                                                    destructive
                                                    blocked={locked}
                                                    label={t(
                                                        "mcp.removeField",
                                                        {
                                                            index: index + 1,
                                                        },
                                                    )}
                                                    onClick={() => {
                                                        if (!locked)
                                                            setFields((rows) =>
                                                                rows.filter(
                                                                    (field) =>
                                                                        field.key !==
                                                                        row.key,
                                                                ),
                                                            );
                                                    }}
                                                />
                                                {row.configured && (
                                                    <p
                                                        className={[
                                                            editForm["hint"],
                                                            styles[
                                                                "secretKeepHint"
                                                            ],
                                                        ].join(" ")}
                                                    >
                                                        {t("mcp.keepValue")}
                                                    </p>
                                                )}
                                            </div>
                                        ))}
                                        <div
                                            className={
                                                styles["sectionAddSecret"]
                                            }
                                        >
                                            <IconButton
                                                icon="plus"
                                                label={t(
                                                    transport === "stdio"
                                                        ? "mcp.addField"
                                                        : "mcp.addHeader",
                                                )}
                                                blocked={
                                                    locked ||
                                                    currentFields.length >= 32
                                                }
                                                onClick={() => {
                                                    if (
                                                        locked ||
                                                        currentFields.length >=
                                                            32
                                                    )
                                                        return;
                                                    const key =
                                                        nextKey.current++;
                                                    setFields((rows) => [
                                                        ...rows,
                                                        {
                                                            key,
                                                            name: "",
                                                            value: "",
                                                            configured: false,
                                                        },
                                                    ]);
                                                }}
                                            />
                                        </div>
                                    </div>,
                                    { section: true },
                                )}
                                {uniformRow(
                                    t("mcp.enabledDefinition"),
                                    <div className={styles["enabledControl"]}>
                                        <ModelCheckbox
                                            checked={enabled}
                                            disabled={locked}
                                            ariaLabel={t(
                                                "mcp.enabledDefinition",
                                            )}
                                            onCheckedChange={setEnabled}
                                        />
                                    </div>,
                                )}
                            </div>
                        </div>
                        <div className={styles["editorFeedback"]}>
                            {code !== null && (
                                <p
                                    className={editForm["error"]}
                                    id={errorId}
                                    role="alert"
                                >
                                    {t(`mcp.errors.${code}`)}
                                </p>
                            )}
                            {state.kind === "error" && (
                                <p className={editForm["hint"]}>
                                    {t("mcp.valuesCleared")}
                                </p>
                            )}
                            {busy && (
                                <p
                                    className={editForm["message"]}
                                    role="status"
                                >
                                    {t("mcp.saving")}
                                </p>
                            )}
                        </div>
                        <SecondaryPageActions active={!hidden}>
                            <div className={styles["editorActions"]}>
                                {uncertain && (
                                    <button
                                        className={buttons["secondary"]}
                                        type="button"
                                        onClick={() => {
                                            clearValues();
                                            onReload();
                                        }}
                                    >
                                        {t("mcp.reload")}
                                    </button>
                                )}
                                <button
                                    className={buttons["primary"]}
                                    type="submit"
                                    form={formId}
                                    aria-disabled={locked}
                                >
                                    {busy ? t("mcp.saving") : t("mcp.save")}
                                </button>
                            </div>
                        </SecondaryPageActions>
                    </form>
                </section>
            </ProviderPage>
        </div>
    );
}
