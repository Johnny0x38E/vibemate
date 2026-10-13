import { Dialog } from "@base-ui/react/dialog";
import { useEffect, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import { IconButton } from "../../components/IconButton";
import { useNotify } from "../../components/Notifications";
import {
    deleteProvider,
    ProviderRequestError,
    type DeletedProvider,
    type ProviderErrorCode,
    type ProviderRecord,
} from "../../lib/desktop/providers";
import buttons from "../../components/actionButtons.module.css";
import styles from "./ProviderDeleteAction.module.css";

/** One destructive list action: confirm, delete, then report the outcome. */
export interface ProviderDeleteActionProps {
    record: ProviderRecord;
    /** True while the list or a form blocks mutations. */
    blocked: boolean;
    onDeleted: (deleted: DeletedProvider) => void;
    /** Re-read the first page after a conflict or unknown outcome. */
    onReload: () => void;
}

type DeleteState =
    | { kind: "idle" }
    | { kind: "deleting" }
    | { kind: "error"; code: ProviderErrorCode };

/**
 * Delete one saved configuration after an explicit confirmation that names the
 * provider and its saved-model count. The dialog traps focus and returns it to
 * the row action. A conflict or unknown outcome reloads the list instead of
 * pretending the delete succeeded.
 */
export function ProviderDeleteAction({
    record,
    blocked,
    onDeleted,
    onReload,
}: ProviderDeleteActionProps): JSX.Element {
    const { t } = useTranslation();
    const notify = useNotify();
    const [open, setOpen] = useState(false);
    const [state, setState] = useState<DeleteState>({ kind: "idle" });
    const trigger = useRef<HTMLElement | null>(null);
    const cancelButton = useRef<HTMLButtonElement | null>(null);
    const deleting = state.kind === "deleting";

    // Hiding or leaving the list unmounts this action; the reference must not
    // outlive the row it points at.
    useEffect(() => {
        return () => {
            trigger.current = null;
        };
    }, []);

    async function confirm(): Promise<void> {
        if (deleting) return;
        setState({ kind: "deleting" });
        try {
            const deleted = await deleteProvider({
                id: record.id,
                expectedRevision: record.revision,
            });
            setOpen(false);
            setState({ kind: "idle" });
            onDeleted(deleted);
            notify(
                t("providers.delete.done", {
                    name: deleted.displayName,
                }),
            );
        } catch (error: unknown) {
            const code =
                error instanceof ProviderRequestError
                    ? error.code
                    : "operation_failed";
            // The list must not keep showing a row whose state is unknown.
            onReload();
            setState({ kind: "error", code });
            // Conflicts and unknown outcomes close the dialog and explain the
            // state through the notification; definite failures stay in view.
            if (
                code === "not_found" ||
                code === "delete_outcome_unknown" ||
                code === "operation_failed" ||
                code === "invalid_response"
            ) {
                setOpen(false);
                notify({
                    tone: "error",
                    message: t(`providers.errors.${code}`),
                });
            }
        }
    }

    return (
        <Dialog.Root
            open={open}
            onOpenChange={(nextOpen) => {
                // A delete in flight must finish; nothing else blocks closing.
                if (!nextOpen && deleting) return;
                setOpen(nextOpen);
                if (!nextOpen) setState({ kind: "idle" });
            }}
        >
            <IconButton
                icon="trash"
                destructive
                label={t("providers.list.deleteLabel", {
                    name: record.displayName,
                })}
                blocked={blocked}
                onClick={() => {
                    if (blocked) return;
                    trigger.current =
                        document.activeElement instanceof HTMLElement
                            ? document.activeElement
                            : null;
                    setOpen(true);
                }}
            />
            <Dialog.Portal>
                <Dialog.Backdrop className={styles["backdrop"]} />
                <Dialog.Popup
                    className={styles["popup"]}
                    // Start on the safe action; the user must move to Delete on
                    // purpose. Base UI falls back to the popup when the ref is not
                    // ready yet, so no custom focus code is needed.
                    initialFocus={cancelButton}
                    finalFocus={() => trigger.current}
                >
                    <Dialog.Title className={styles["title"]}>
                        {t("providers.delete.title")}
                    </Dialog.Title>
                    <Dialog.Description className={styles["body"]}>
                        {t("providers.delete.body", {
                            name: record.displayName,
                            count: record.selectedModelCount,
                        })}
                    </Dialog.Description>
                    {state.kind === "error" && (
                        <p className={styles["error"]} role="alert">
                            {t(`providers.errors.${state.code}`)}
                        </p>
                    )}
                    <div className={styles["actions"]}>
                        <button
                            ref={cancelButton}
                            type="button"
                            className={buttons["secondary"]}
                            disabled={deleting}
                            onClick={() => {
                                setOpen(false);
                            }}
                        >
                            {t("providers.delete.cancel")}
                        </button>
                        <button
                            type="button"
                            className={buttons["dangerSecondary"]}
                            aria-disabled={deleting}
                            onClick={() => {
                                if (deleting) return;
                                void confirm();
                            }}
                        >
                            {t("providers.delete.confirm")}
                        </button>
                    </div>
                </Dialog.Popup>
            </Dialog.Portal>
        </Dialog.Root>
    );
}
