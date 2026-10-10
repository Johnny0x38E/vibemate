import { useEffect, useId, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import { Icon } from "../../components/Icon";
import pageStyles from "../providers/ProviderPage.module.css";
import { FieldSelect } from "../../components/FieldSelect";
import {
    McpRequestError,
    saveMcpDefinition,
    type McpErrorCode,
    type McpRecord,
    type McpSecretInput,
    type SaveMcpInput,
} from "../../lib/desktop/mcp";
import styles from "./Mcp.module.css";
import buttons from "../providers/providerButtons.module.css";

type FormState =
    | { kind: "idle" | "saving" }
    | { kind: "error"; code: McpErrorCode; uncertain: boolean };
interface FieldDraft {
    key: number;
    name: string;
    value: string;
    configured: boolean;
}
/** Editor inputs; caller keys by record/revision to make explicit reloads a new form. */
export interface McpFormProps {
    record: McpRecord | null;
    hidden: boolean;
    onSaved: (record: McpRecord) => void;
    onCancel: () => void;
    onReload: () => void;
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
}: McpFormProps): JSX.Element {
    const { t } = useTranslation();
    const prefix = useId();
    const [displayName, setDisplayName] = useState(record?.displayName ?? "");
    const [serverName, setServerName] = useState(record?.serverName ?? "");
    const [enabled, setEnabled] = useState(record?.enabled ?? true);
    const [transport, setTransport] = useState<"stdio" | "http">(
        record?.connection.type ?? "stdio",
    );
    const existing = record?.connection;
    const [command, setCommand] = useState(
        existing?.type === "stdio" ? existing.command : "",
    );
    const [args, setArgs] = useState(
        existing?.type === "stdio" ? existing.args.join("\n") : "",
    );
    const [argsEdited, setArgsEdited] = useState(false);
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
    async function submit(): Promise<void> {
        if (locked || pending.current || hidden) return;
        pending.current = true;
        const request: SaveMcpInput = {
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
                          // An untouched textarea must preserve a saved single empty
                          // argument, which renders like an empty argument list.
                          args:
                              !argsEdited && existing?.type === "stdio"
                                  ? existing.args
                                  : args === ""
                                    ? []
                                    : args.split("\n"),
                          cwd: cwd.trim() === "" ? null : cwd,
                          env: secretInputs(env),
                      }
                    : { type: "http", url, headers: secretInputs(headers) },
        };
        clearValues();
        setState({ kind: "saving" });
        try {
            const saved = await saveMcpDefinition(request);
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
    return (
        <>
            <div className={pageStyles["header"]}>
                <button
                    className={pageStyles["backButton"]}
                    type="button"
                    aria-disabled={busy}
                    aria-label={t("mcp.backLabel")}
                    onClick={() => {
                        if (!busy) {
                            clearValues();
                            if (uncertain) onReload();
                            else onCancel();
                        }
                    }}
                >
                    <Icon name="back" />
                    {t("mcp.back")}
                </button>
                <h1
                    className={pageStyles["title"]}
                    tabIndex={-1}
                    data-focus="heading"
                >
                    {t(record === null ? "mcp.create" : "mcp.edit")}
                </h1>
            </div>
            <form
                className={styles["form"]}
                aria-label={t(record === null ? "mcp.create" : "mcp.edit")}
                aria-busy={busy}
                onSubmit={(event) => {
                    event.preventDefault();
                    void submit();
                }}
            >
                <div className={styles["saveRow"]}>
                    <button
                        className={buttons["primary"]}
                        type="submit"
                        aria-disabled={locked}
                    >
                        {t("mcp.save")}
                    </button>
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
                </div>
                <p className={styles["hint"]}>{t("mcp.definitionOnly")}</p>
                <label className={styles["field"]}>
                    {t("mcp.displayName")}
                    <input
                        className={styles["input"]}
                        value={displayName}
                        readOnly={locked}
                        {...fieldError("display_name_invalid")}
                        onChange={(event) => {
                            setDisplayName(event.currentTarget.value);
                        }}
                    />
                </label>
                <label className={styles["field"]}>
                    {t("mcp.serverName")}
                    <input
                        className={styles["input"]}
                        value={serverName}
                        readOnly={locked}
                        {...fieldError(
                            code === "server_name_taken"
                                ? "server_name_taken"
                                : "server_name_invalid",
                        )}
                        autoComplete="off"
                        spellCheck={false}
                        onChange={(event) => {
                            setServerName(event.currentTarget.value);
                        }}
                    />
                    <span className={styles["hint"]}>
                        {t("mcp.serverNameHint")}
                    </span>
                </label>
                <div className={styles["field"]}>
                    <label htmlFor={`${prefix}-transport`}>
                        {t("mcp.transport")}
                    </label>
                    <FieldSelect
                        id={`${prefix}-transport`}
                        value={transport}
                        blocked={locked}
                        options={[
                            { value: "stdio", label: t("mcp.stdio") },
                            { value: "http", label: t("mcp.http") },
                        ]}
                        onChange={(value) => {
                            if (value === "stdio" || value === "http") {
                                clearValues();
                                setTransport(value);
                            }
                        }}
                    />
                </div>
                {transport === "stdio" ? (
                    <>
                        <label className={styles["field"]}>
                            {t("mcp.command")}
                            <input
                                className={styles["input"]}
                                value={command}
                                readOnly={locked}
                                {...fieldError("command_invalid")}
                                autoComplete="off"
                                onChange={(event) => {
                                    setCommand(event.currentTarget.value);
                                }}
                            />
                            <span className={styles["hint"]}>
                                {t("mcp.commandHint")}
                            </span>
                        </label>
                        <label className={styles["field"]}>
                            {t("mcp.args")}
                            <textarea
                                className={styles["input"]}
                                rows={3}
                                value={args}
                                readOnly={locked}
                                {...fieldError("args_invalid")}
                                onChange={(event) => {
                                    setArgsEdited(true);
                                    setArgs(event.currentTarget.value);
                                }}
                            />
                            <span className={styles["hint"]}>
                                {t("mcp.argsHint")}
                            </span>
                        </label>
                        <label className={styles["field"]}>
                            {t("mcp.cwd")}
                            <input
                                className={styles["input"]}
                                value={cwd}
                                readOnly={locked}
                                {...fieldError("cwd_invalid")}
                                onChange={(event) => {
                                    setCwd(event.currentTarget.value);
                                }}
                            />
                        </label>
                    </>
                ) : (
                    <label className={styles["field"]}>
                        {t("mcp.url")}
                        <input
                            className={styles["input"]}
                            value={url}
                            readOnly={locked}
                            {...fieldError("url_invalid")}
                            autoComplete="off"
                            onChange={(event) => {
                                setUrl(event.currentTarget.value);
                            }}
                        />
                        <span className={styles["hint"]}>
                            {t("mcp.urlHint")}
                        </span>
                    </label>
                )}
                <fieldset className={styles["secretGroup"]}>
                    <legend>
                        {t(transport === "stdio" ? "mcp.env" : "mcp.headers")}
                    </legend>
                    <p className={styles["hint"]}>{t("mcp.protectedValues")}</p>
                    {currentFields.map((row, index) => (
                        <div className={styles["secretRow"]} key={row.key}>
                            <label className={styles["field"]}>
                                {t("mcp.fieldName", { index: index + 1 })}
                                <input
                                    className={styles["input"]}
                                    value={row.name}
                                    readOnly={locked}
                                    {...fieldError("field_name_invalid")}
                                    spellCheck={false}
                                    autoComplete="off"
                                    onChange={(event) => {
                                        updateField(
                                            row.key,
                                            "name",
                                            event.currentTarget.value,
                                        );
                                    }}
                                />
                            </label>
                            <label className={styles["field"]}>
                                {t("mcp.fieldValue", { index: index + 1 })}
                                <input
                                    className={styles["input"]}
                                    type="password"
                                    value={row.value}
                                    readOnly={locked}
                                    aria-invalid={
                                        code === "secret_required" ||
                                        code === "secret_invalid"
                                    }
                                    aria-describedby={
                                        code === "secret_required" ||
                                        code === "secret_invalid"
                                            ? errorId
                                            : undefined
                                    }
                                    autoComplete="new-password"
                                    onChange={(event) => {
                                        updateField(
                                            row.key,
                                            "value",
                                            event.currentTarget.value,
                                        );
                                    }}
                                />
                                {row.configured && (
                                    <span className={styles["hint"]}>
                                        {t("mcp.keepValue")}
                                    </span>
                                )}
                            </label>
                            <button
                                className={buttons["text"]}
                                type="button"
                                aria-disabled={locked}
                                aria-label={t("mcp.removeField", {
                                    index: index + 1,
                                })}
                                onClick={() => {
                                    if (!locked)
                                        setFields((rows) =>
                                            rows.filter(
                                                (field) =>
                                                    field.key !== row.key,
                                            ),
                                        );
                                }}
                            >
                                {t("mcp.remove")}
                            </button>
                        </div>
                    ))}
                    <button
                        className={buttons["secondary"]}
                        type="button"
                        aria-disabled={locked || currentFields.length >= 32}
                        onClick={() => {
                            if (locked || currentFields.length >= 32) return;
                            const key = nextKey.current++;
                            setFields((rows) => [
                                ...rows,
                                { key, name: "", value: "", configured: false },
                            ]);
                        }}
                    >
                        {t("mcp.addField")}
                    </button>
                </fieldset>
                <label className={styles["enabled"]}>
                    <input
                        type="checkbox"
                        checked={enabled}
                        disabled={locked}
                        onChange={(event) => {
                            setEnabled(event.currentTarget.checked);
                        }}
                    />
                    {t("mcp.enabledDefinition")}
                </label>
                {code !== null && (
                    <p className={styles["error"]} id={errorId} role="alert">
                        {t(`mcp.errors.${code}`)}
                    </p>
                )}
                {state.kind === "error" && (
                    <p className={styles["hint"]}>{t("mcp.valuesCleared")}</p>
                )}
                {busy && <p role="status">{t("mcp.saving")}</p>}
            </form>
        </>
    );
}
