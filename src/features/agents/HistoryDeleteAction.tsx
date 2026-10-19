import { Dialog } from "@base-ui/react/dialog";
import { useEffect, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import { IconButton } from "../../components/IconButton";
import buttons from "../../components/actionButtons.module.css";
import {
    deleteHistoryEntry,
    HistoryDeleteRequestError,
    type HistoryDeleteErrorCode,
    type HistoryDeleteLeftover,
    type HistoryDeleteOutcome,
} from "../../lib/desktop/historyDelete";
import type { ConfigurableAgent } from "../../lib/desktop/agents";
import styles from "./HistoryDeleteAction.module.css";

/** Why the trash control is disabled before calling IPC. */
export type HistoryDeleteBlockCode =
    "history_active" | "history_unresolved" | "history_referenced";

type DialogState =
    | { kind: "idle" }
    | { kind: "deleting" }
    | { kind: "error"; code: HistoryDeleteErrorCode }
    | { kind: "failed"; leftovers: HistoryDeleteLeftover[] };

/**
 * Row-end delete for one history entry: confirm, call H3 IPC, surface leftovers.
 *
 * A blocked reason disables the trash and names the refusal. Failed leftovers
 * stay in the dialog with a retry that repeats the same entry key.
 */
export function HistoryDeleteAction({
    agent,
    entryKey,
    blocked,
    blockReason,
    onDeleted,
    onBusyChange,
}: {
    agent: ConfigurableAgent;
    entryKey: string;
    /** Parent busy or another mutation in flight. */
    blocked: boolean;
    /** UI-known refusal; trash stays focusable with this reason as its label. */
    blockReason: HistoryDeleteBlockCode | null;
    /** Fired after a completed delete so the parent can reload history. */
    onDeleted: (outcome: HistoryDeleteOutcome) => void;
    onBusyChange?: (busy: boolean) => void;
}): JSX.Element {
    const { t } = useTranslation();
    const [open, setOpen] = useState(false);
    const [state, setState] = useState<DialogState>({ kind: "idle" });
    const trigger = useRef<HTMLElement | null>(null);
    const cancelButton = useRef<HTMLButtonElement | null>(null);
    const busyCallback = useRef(onBusyChange);
    useEffect(() => {
        busyCallback.current = onBusyChange;
    }, [onBusyChange]);
    useEffect(() => {
        return () => {
            trigger.current = null;
            busyCallback.current?.(false);
        };
    }, []);

    const deleting = state.kind === "deleting";
    const refuseLabel =
        blockReason !== null
            ? t(`agents.restore.delete.blocked.${blockReason}`)
            : null;
    const trashLabel = refuseLabel ?? t("agents.restore.delete.label");

    async function runDelete(): Promise<void> {
        if (deleting || blockReason !== null) return;
        setState({ kind: "deleting" });
        busyCallback.current?.(true);
        try {
            const outcome = await deleteHistoryEntry(agent, entryKey);
            if (outcome.state === "failed") {
                setState({ kind: "failed", leftovers: outcome.leftovers });
                return;
            }
            setOpen(false);
            setState({ kind: "idle" });
            onDeleted(outcome);
        } catch (error: unknown) {
            const code =
                error instanceof HistoryDeleteRequestError
                    ? error.code
                    : "operation_failed";
            setState({ kind: "error", code });
        } finally {
            busyCallback.current?.(false);
        }
    }

    return (
        <Dialog.Root
            open={open}
            onOpenChange={(nextOpen) => {
                if (!nextOpen && deleting) return;
                setOpen(nextOpen);
                if (!nextOpen) setState({ kind: "idle" });
            }}
        >
            <IconButton
                icon="trash"
                destructive
                label={trashLabel}
                blocked={blocked || blockReason !== null}
                onClick={() => {
                    if (blocked || blockReason !== null) return;
                    trigger.current =
                        document.activeElement instanceof HTMLElement
                            ? document.activeElement
                            : null;
                    setState({ kind: "idle" });
                    setOpen(true);
                }}
            />
            <Dialog.Portal>
                <Dialog.Backdrop className={styles["backdrop"]} />
                <Dialog.Popup
                    className={styles["popup"]}
                    initialFocus={cancelButton}
                    finalFocus={() => trigger.current}
                >
                    <Dialog.Title className={styles["title"]}>
                        {t("agents.restore.delete.title")}
                    </Dialog.Title>
                    <Dialog.Description className={styles["body"]}>
                        {t("agents.restore.delete.body")}
                    </Dialog.Description>
                    {state.kind === "error" && (
                        <p className={styles["error"]} role="alert">
                            {t(`agents.restore.delete.errors.${state.code}`)}
                        </p>
                    )}
                    {state.kind === "failed" && (
                        <>
                            <p className={styles["error"]} role="alert">
                                {t("agents.restore.delete.leftoversHeading")}
                            </p>
                            <ul className={styles["leftovers"]}>
                                {state.leftovers.map((row) => (
                                    <li key={row.backupId}>{row.path}</li>
                                ))}
                            </ul>
                            <p className={styles["body"]}>
                                {t("agents.restore.delete.leftoverHint")}
                            </p>
                        </>
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
                            {t("agents.restore.delete.cancel")}
                        </button>
                        <button
                            type="button"
                            className={buttons["dangerSecondary"]}
                            aria-disabled={deleting}
                            onClick={() => {
                                if (deleting) return;
                                void runDelete();
                            }}
                        >
                            {deleting
                                ? t("agents.restore.delete.deleting")
                                : state.kind === "failed"
                                  ? t("agents.restore.delete.retry")
                                  : t("agents.restore.delete.confirm")}
                        </button>
                    </div>
                </Dialog.Popup>
            </Dialog.Portal>
        </Dialog.Root>
    );
}

/**
 * Map list-row facts to a refuse code the backend would return, when known.
 *
 * `history_referenced` is not always visible from the page payload; those
 * refusals surface after the confirm dialog calls IPC.
 */
export function historyDeleteBlockReason(entry: {
    requiresResolution: boolean;
    sourceState: string;
}): HistoryDeleteBlockCode | null {
    if (entry.requiresResolution) return "history_unresolved";
    if (
        entry.sourceState === "planned" ||
        entry.sourceState === "partial" ||
        entry.sourceState === "unknown"
    ) {
        return "history_active";
    }
    return null;
}
