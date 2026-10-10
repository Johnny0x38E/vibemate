import { useEffect, useId, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import {
    addManualProviderModel,
    listProviderModels,
    ModelRequestError,
    type ModelErrorCode,
    type ProviderModel,
} from "../../lib/desktop/models";
import styles from "./ProviderForm.module.css";
import buttons from "./providerButtons.module.css";

type SaveState =
    | { kind: "idle" | "saving" | "checking" | "uncertain" }
    | { kind: "error"; code: ModelErrorCode };

/** Inputs for a manual model creation, separate from checkbox drafts. */
export interface ManualModelFormProps {
    providerId: string;
    blocked: boolean;
    onSaved: (model: ProviderModel) => void;
    onBusyChange: (busy: boolean) => void;
}

/**
 * Save a selected manual model through Rust. Unknown write outcomes block a
 * second creation until a read checks for the exact model ID. Unmounting ignores
 * late replies; the parent reloads its selected list on the next visit.
 */
export function ManualModelForm({
    providerId,
    blocked,
    onSaved,
    onBusyChange,
}: ManualModelFormProps): JSX.Element {
    const { t } = useTranslation();
    const id = useId();
    const [modelId, setModelId] = useState("");
    const [alias, setAlias] = useState("");
    const [state, setState] = useState<SaveState>({ kind: "idle" });
    const active = useRef(true);
    const pending = useRef(false);
    const busy = state.kind === "saving" || state.kind === "checking";
    const uncertain = state.kind === "uncertain" || state.kind === "checking";
    const locked = blocked || busy || uncertain;
    const fieldError = state.kind === "error" ? state.code : null;
    const idError =
        fieldError === "model_id_invalid" ||
        fieldError === "model_already_exists";
    const aliasError = fieldError === "model_alias_invalid";

    useEffect(() => {
        active.current = true;
        return () => {
            active.current = false;
        };
    }, []);

    // Report pending writes to the detail-page navigation guard. A hidden parent
    // unmounts this form, so cleanup must release that guard as well.
    useEffect(() => {
        onBusyChange(busy);
        return () => {
            onBusyChange(false);
        };
    }, [busy, onBusyChange]);

    function saved(model: ProviderModel): void {
        setState({ kind: "idle" });
        setModelId("");
        setAlias("");
        onSaved(model);
    }

    async function submit(): Promise<void> {
        if (locked || pending.current) return;
        pending.current = true;
        setState({ kind: "saving" });
        try {
            const model = await addManualProviderModel({
                providerId,
                modelId,
                alias: alias.trim() === "" ? null : alias,
            });
            if (active.current) saved(model);
        } catch (error: unknown) {
            if (!active.current) return;
            const code =
                error instanceof ModelRequestError
                    ? error.code
                    : "operation_failed";
            setState(
                code === "operation_failed" || code === "invalid_response"
                    ? { kind: "uncertain" }
                    : { kind: "error", code },
            );
        } finally {
            pending.current = false;
        }
    }

    async function checkResult(): Promise<void> {
        if (blocked || pending.current) return;
        pending.current = true;
        setState({ kind: "checking" });
        try {
            let after: string | null = null;
            for (;;) {
                const result = await listProviderModels({
                    providerId,
                    after,
                    limit: 200,
                    filter: "all",
                });
                if (!active.current) return;
                if (result.kind === "preview") {
                    setState({ kind: "uncertain" });
                    return;
                }
                // Read cursor pages rather than fuzzy search: a valid ID can be
                // longer than the search limit, and similar IDs prove nothing.
                const found = result.page.items.find(
                    (model) => model.modelId === modelId.trim(),
                );
                if (found !== undefined) {
                    if (found.source === "manual" && found.selected)
                        saved(found);
                    else
                        setState({
                            kind: "error",
                            code: "model_already_exists",
                        });
                    return;
                }
                after = result.page.nextCursor;
                if (after === null) break;
            }
            setState({ kind: "idle" });
        } catch {
            if (active.current) setState({ kind: "uncertain" });
        } finally {
            pending.current = false;
        }
    }

    return (
        <form
            aria-labelledby={`${id}-title`}
            aria-busy={busy}
            onSubmit={(event) => {
                event.preventDefault();
                void submit();
            }}
        >
            <h3 className={styles["groupTitle"]} id={`${id}-title`}>
                {t("providers.models.manual.title")}
            </h3>
            <p className={styles["hint"]} id={`${id}-hint`}>
                {t("providers.models.manual.hint")}
            </p>
            <div className={styles["control"]}>
                <label htmlFor={`${id}-model`}>
                    {t("providers.models.manual.modelId")}
                </label>
                <input
                    className={styles["input"]}
                    id={`${id}-model`}
                    value={modelId}
                    readOnly={locked}
                    aria-disabled={locked}
                    aria-invalid={idError}
                    aria-describedby={idError ? `${id}-error` : `${id}-hint`}
                    autoComplete="off"
                    spellCheck={false}
                    onChange={(event) => {
                        setModelId(event.currentTarget.value);
                        setState({ kind: "idle" });
                    }}
                />
                <label htmlFor={`${id}-alias`}>
                    {t("providers.models.manual.alias")}
                </label>
                <input
                    className={styles["input"]}
                    id={`${id}-alias`}
                    value={alias}
                    readOnly={locked}
                    aria-disabled={locked}
                    aria-invalid={aliasError}
                    aria-describedby={aliasError ? `${id}-error` : undefined}
                    autoComplete="off"
                    onChange={(event) => {
                        setAlias(event.currentTarget.value);
                        setState({ kind: "idle" });
                    }}
                />
            </div>
            <div className={styles["feedback"]}>
                {fieldError !== null && (
                    <p
                        id={`${id}-error`}
                        className={styles["error"]}
                        role="alert"
                    >
                        {t(`providers.models.errors.${fieldError}`)}
                    </p>
                )}
                {uncertain && (
                    <p className={styles["error"]} role="alert">
                        {t("providers.models.manual.uncertain")}
                    </p>
                )}
                {state.kind === "saving" && (
                    <p role="status" className={styles["message"]}>
                        {t("providers.models.saving")}
                    </p>
                )}
            </div>
            <div className={styles["actions"]}>
                <button
                    className={buttons["secondary"]}
                    type="submit"
                    aria-disabled={locked}
                >
                    {t("providers.models.manual.add")}
                </button>
                {uncertain && (
                    <button
                        className={buttons["secondary"]}
                        type="button"
                        aria-disabled={blocked || busy}
                        onClick={() => {
                            void checkResult();
                        }}
                    >
                        {t("providers.models.manual.check")}
                    </button>
                )}
            </div>
        </form>
    );
}
