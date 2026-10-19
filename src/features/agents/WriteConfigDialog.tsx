import { Dialog } from "@base-ui/react/dialog";
import { useRef, type JSX } from "react";
import { useTranslation } from "react-i18next";
import buttons from "../../components/actionButtons.module.css";
import type { ConfigurableAgent } from "../../lib/desktop/agents";
import { ConfigPreview } from "./ConfigPreview";
import { WriteCurrentSummary } from "./RestoreView";
import dialog from "./VersionTextDialog.module.css";

/**
 * List-row 「写入」 dialog: current Provider|Model summary (when matched) plus
 * the shared ConfigPreview form. Preview cards render inline below the form
 * inside this same dialog — never a second nested modal and never navigation
 * to the Agent detail page. Sits under the titlebar so traffic lights stay
 * reachable (shared VersionTextDialog.popupCompact with 「详细配置」 / restore).
 */
export function WriteConfigDialog({
    agent,
    agentName,
    entryPath,
    providerLabel,
    modelLabel,
    blocked,
    onBusyChange,
    onApplied,
    onClose,
    returnFocus,
}: {
    agent: ConfigurableAgent;
    agentName: string;
    entryPath: string | null;
    providerLabel: string | null;
    modelLabel: string | null;
    blocked: boolean;
    onBusyChange: (busy: boolean) => void;
    onApplied: () => void;
    onClose: () => void;
    returnFocus: () => HTMLElement | null;
}): JSX.Element {
    const { t } = useTranslation();
    const title = useRef<HTMLHeadingElement>(null);

    return (
        <Dialog.Root
            open
            /* Portaled FieldSelect menus sit outside Popup; trap-focus keeps
               focus in the dialog while allowing pointer hits on the menu.
               disablePointerDismissal so choosing a provider does not close. */
            modal="trap-focus"
            disablePointerDismissal
            onOpenChange={(open) => {
                if (!open && !blocked) onClose();
            }}
        >
            <Dialog.Portal>
                <Dialog.Backdrop className={dialog["backdrop"]} />
                <Dialog.Popup
                    className={dialog["popupCompact"]}
                    initialFocus={title}
                    finalFocus={returnFocus}
                >
                    <Dialog.Title
                        ref={title}
                        tabIndex={-1}
                        className={dialog["title"]}
                    >
                        {t("agents.writeDialog.title", { name: agentName })}
                    </Dialog.Title>
                    <WriteCurrentSummary
                        providerLabel={providerLabel}
                        modelLabel={modelLabel}
                    />
                    <ConfigPreview
                        key={JSON.stringify([agent, entryPath])}
                        agent={agent}
                        entryPath={entryPath}
                        blocked={blocked}
                        onBusyChange={onBusyChange}
                        onApplied={onApplied}
                    />
                    <div className={dialog["actions"]}>
                        <button
                            type="button"
                            className={buttons["secondary"]}
                            aria-disabled={blocked}
                            onClick={() => {
                                if (!blocked) onClose();
                            }}
                        >
                            {t("agents.writeDialog.close")}
                        </button>
                    </div>
                </Dialog.Popup>
            </Dialog.Portal>
        </Dialog.Root>
    );
}
