import { useEffect, useId, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import {
    getProviderModelSnapshot,
    updateProviderModelAlias,
    ModelRequestError,
    type ModelErrorCode,
    type ModelSnapshot,
    type ProviderModel,
} from "../../lib/desktop/models";
import buttons from "../../components/actionButtons.module.css";
import form from "../../components/editForm.module.css";
import styles from "./ModelAliasEditor.module.css";

/** One saved model editor; the parent blocks competing operations while it is open. */
export interface ModelAliasEditorProps {
    providerId: string;
    modelId: string;
    onSaved: (model: ProviderModel) => void;
    onClose: () => void;
    onBusyChange: (busy: boolean) => void;
    /** True only while a save is in flight; closing then could hide a mutation. */
    onSavingChange: (saving: boolean) => void;
}

type Phase =
    "loading" | "ready" | "saving" | "conflict" | "uncertain" | "readError";

/**
 * Edit an alias against a Rust content snapshot. Conflicts and unknown outcomes
 * require an explicit reload; reloading deliberately replaces the local draft.
 * This component never downloads a catalog or reads a credential.
 */
export function ModelAliasEditor({
    providerId,
    modelId,
    onSaved,
    onClose,
    onBusyChange,
    onSavingChange,
}: ModelAliasEditorProps): JSX.Element {
    const { t } = useTranslation();
    const id = useId();
    const input = useRef<HTMLInputElement>(null);
    const generation = useRef(0);
    const pending = useRef(false);
    const [snapshot, setSnapshot] = useState<ModelSnapshot | null>(null);
    const [alias, setAlias] = useState("");
    const [phase, setPhase] = useState<Phase>("loading");
    const [error, setError] = useState<ModelErrorCode | null>(null);
    const [reload, setReload] = useState(0);
    const busy = phase === "loading" || phase === "saving";
    const locked = phase !== "ready";

    // Each mount/reload owns its result. Cleanup invalidates both reads and saves
    // so hidden views, identity changes and StrictMode cannot apply late replies.
    useEffect(() => {
        const current = ++generation.current;
        void getProviderModelSnapshot({ providerId, modelId })
            .then((loaded) => {
                if (current !== generation.current) return;
                setSnapshot(loaded);
                setAlias(loaded.model.alias ?? "");
                setError(null);
                setPhase("ready");
            })
            .catch((reason: unknown) => {
                if (current !== generation.current) return;
                setError(
                    reason instanceof ModelRequestError
                        ? reason.code
                        : "operation_failed",
                );
                setPhase("readError");
            });
        return () => {
            generation.current += 1;
        };
    }, [providerId, modelId, reload]);

    // The form opens from a row action; focus the input only after its snapshot
    // arrives, not on language changes or every keystroke.
    useEffect(() => {
        onBusyChange(busy);
    }, [busy, onBusyChange]);

    // The dialog can unmount this editor after a save or while Providers is
    // hidden. Releasing the parent guard on unmount prevents a permanent block.
    // The ref keeps the cleanup on the latest callback without re-running it.
    const reportBusy = useRef(onBusyChange);
    useEffect(() => {
        reportBusy.current = onBusyChange;
    }, [onBusyChange]);
    useEffect(() => {
        return () => {
            reportBusy.current(false);
        };
    }, []);

    useEffect(() => {
        if (phase === "ready") input.current?.focus();
    }, [phase]);

    async function save(): Promise<void> {
        if (locked || pending.current || snapshot === null) return;
        pending.current = true;
        const current = generation.current;
        setPhase("saving");
        onSavingChange(true);
        setError(null);
        try {
            const updated = await updateProviderModelAlias({
                providerId,
                modelId,
                expectedDigest: snapshot.contentDigest,
                alias: alias.trim() === "" ? null : alias,
            });
            if (current === generation.current) onSaved(updated.model);
        } catch (reason: unknown) {
            if (current !== generation.current) return;
            const code =
                reason instanceof ModelRequestError
                    ? reason.code
                    : "operation_failed";
            setError(code);
            setPhase(
                code === "model_changed"
                    ? "conflict"
                    : code === "operation_failed" || code === "invalid_response"
                      ? "uncertain"
                      : "ready",
            );
        } finally {
            pending.current = false;
            onSavingChange(false);
        }
    }

    return (
        <form
            className={styles["editor"]}
            aria-label={t("providers.models.aliasEditor.title", { modelId })}
            aria-busy={busy}
            onSubmit={(event) => {
                event.preventDefault();
                void save();
            }}
        >
            <p className={styles["title"]}>
                {t("providers.models.aliasEditor.title", { modelId })}
            </p>
            <label htmlFor={id}>{t("providers.models.manual.alias")}</label>
            <div className={styles["controls"]}>
                <input
                    ref={input}
                    id={id}
                    className={form["input"]}
                    value={alias}
                    readOnly={locked}
                    aria-disabled={locked}
                    autoComplete="off"
                    aria-invalid={error === "model_alias_invalid"}
                    aria-describedby={
                        error !== null ? `${id}-error` : undefined
                    }
                    onKeyDown={(event) => {
                        if (event.key === "Escape" && busy) {
                            // Base UI would close the dialog; keep it open until the
                            // in-flight read or save has a known result.
                            event.preventDefault();
                            event.stopPropagation();
                        }
                    }}
                    onChange={(event) => {
                        setAlias(event.currentTarget.value);
                        setError(null);
                    }}
                />
                <button
                    type="submit"
                    className={buttons["primary"]}
                    disabled={locked}
                >
                    {t(
                        phase === "saving"
                            ? "providers.models.saving"
                            : "providers.models.aliasEditor.save",
                    )}
                </button>
                <button
                    type="button"
                    className={buttons["secondary"]}
                    disabled={busy}
                    onClick={onClose}
                >
                    {t("providers.models.aliasEditor.cancel")}
                </button>
            </div>
            {phase === "loading" && (
                <p role="status">{t("providers.models.loading")}</p>
            )}
            {error !== null && (
                <p id={`${id}-error`} className={form["error"]} role="alert">
                    {t(
                        phase === "uncertain"
                            ? "providers.models.aliasEditor.uncertain"
                            : `providers.models.errors.${error}`,
                    )}
                </p>
            )}
            {(phase === "conflict" ||
                phase === "uncertain" ||
                phase === "readError") && (
                <button
                    type="button"
                    className={buttons["secondary"]}
                    onClick={() => {
                        setPhase("loading");
                        setError(null);
                        setReload((value) => value + 1);
                    }}
                >
                    {t("providers.models.aliasEditor.reload")}
                </button>
            )}
        </form>
    );
}
