import { Dialog } from "@base-ui/react/dialog";
import { useEffect, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import { IconButton } from "../../components/IconButton";
import type { ProviderModel } from "../../lib/desktop/models";
import { useNotify } from "../../components/Notifications";
import { ModelAliasEditor } from "./ModelAliasEditor";
import styles from "./ModelAliasAction.module.css";

interface ModelAliasActionProps {
    providerId: string;
    model: ProviderModel;
    blocked: boolean;
    onBusyChange: (busy: boolean) => void;
    onReload: () => void;
    onSaved: (model: ProviderModel) => void;
}

/** Open an isolated accessible alias editor for one saved model row. */
export function ModelAliasAction({
    providerId,
    model,
    blocked,
    onBusyChange,
    onReload,
    onSaved,
}: ModelAliasActionProps): JSX.Element {
    const { t } = useTranslation();
    const notify = useNotify();
    const [open, setOpen] = useState(false);
    const [busy, setBusy] = useState(false);
    const [saving, setSaving] = useState(false);
    const trigger = useRef<HTMLElement | null>(null);

    // Hiding Providers unmounts this row action while an edit may be open. Release
    // the parent's busy guard and drop the row-button reference with it.
    useEffect(() => {
        return () => {
            onBusyChange(false);
            trigger.current = null;
        };
    }, [onBusyChange]);

    function changeBusy(nextBusy: boolean): void {
        setBusy(nextBusy);
        onBusyChange(nextBusy);
    }

    // Programmatic closes (Save or Cancel) do not run Base UI's own close path,
    // so the row action is focused here. Base UI handles Escape through
    // `finalFocus` above.
    function returnFocus(): void {
        const element = trigger.current;
        if (element === null) return;
        window.setTimeout(() => {
            if (element.isConnected) element.focus();
        }, 0);
    }

    return (
        <Dialog.Root
            open={open}
            onOpenChange={(nextOpen) => {
                // Closing while a save is in flight could hide an unknown result.
                // Closing while the model read is still loading is harmless.
                if (!nextOpen && saving) return;
                setOpen(nextOpen);
                if (!nextOpen) {
                    changeBusy(false);
                }
            }}
        >
            <IconButton
                icon="edit"
                label={t("providers.models.aliasEditor.title", {
                    modelId: model.modelId,
                })}
                blocked={blocked || busy}
                onClick={() => {
                    if (blocked || busy) return;
                    trigger.current =
                        document.activeElement instanceof HTMLElement
                            ? document.activeElement
                            : null;
                    setOpen(true);
                    changeBusy(true);
                }}
            />
            <Dialog.Portal>
                <Dialog.Backdrop className={styles["backdrop"]} />
                <Dialog.Popup
                    className={styles["popup"]}
                    // The dialog is opened from a plain IconButton, so Base UI has no
                    // Dialog.Trigger of its own; point final focus at the row action
                    // so a keyboard user returns to the control they opened.
                    finalFocus={() => trigger.current}
                >
                    <Dialog.Title className={styles["visuallyHidden"]}>
                        {t("providers.models.aliasEditor.title", {
                            modelId: model.modelId,
                        })}
                    </Dialog.Title>
                    <ModelAliasEditor
                        providerId={providerId}
                        modelId={model.modelId}
                        onBusyChange={changeBusy}
                        onSavingChange={setSaving}
                        onSaved={(updated) => {
                            onSaved(updated);
                            notify({
                                tone: "success",
                                message: t(
                                    "providers.models.aliasEditor.saved",
                                ),
                            });
                            setOpen(false);
                            changeBusy(false);
                            returnFocus();
                        }}
                        onClose={() => {
                            setOpen(false);
                            changeBusy(false);
                            onReload();
                            returnFocus();
                        }}
                    />
                </Dialog.Popup>
            </Dialog.Portal>
        </Dialog.Root>
    );
}
