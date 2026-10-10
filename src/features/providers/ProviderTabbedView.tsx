import { useId, useRef, useState, type JSX, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import type {
    ProviderKind,
    ProviderRecord,
    ProviderTemplate,
} from "../../lib/desktop/providers";
import { ProviderForm, type ProviderFormMode } from "./ProviderForm";
import { ProviderModels, type SelectedModelCount } from "./ProviderModels";
import { ProviderModelsCreateGate } from "./ProviderModelsCreateGate";
import styles from "./ProviderEditView.module.css";

type ProviderTab = "api" | "models";

export type { SelectedModelCount };

/** Inputs for {@link ProviderTabbedView}. */
export interface ProviderTabbedViewProps {
    mode: ProviderFormMode;
    templates: readonly ProviderTemplate[];
    onSaved: (record: ProviderRecord) => void;
    onRecordLoaded: (record: ProviderRecord) => void;
    onRefresh: () => Promise<boolean>;
    onFormBusyChange: (busy: boolean) => void;
    onModelsBusyChange: (busy: boolean) => void;
    onModelsDirtyChange?: (dirty: boolean) => void;
    /** Keeps the create-page title aligned with the provider field. */
    onCreateKindChange?: (kind: ProviderKind) => void;
    hidden?: boolean;
}

function formatModelsTabLabel(
    t: (key: string, options?: Record<string, unknown>) => string,
    selected: SelectedModelCount | null,
): string {
    if (selected === null || selected.count === 0) {
        return t("providers.tabs.models");
    }
    if (selected.hasMore) {
        return t("providers.tabs.modelsCountMany", { count: selected.count });
    }
    return t("providers.tabs.modelsCount", { count: selected.count });
}

/**
 * API / Models tabs for create and edit. Panels stay mounted while switching
 * tabs; models need a saved instance id (edit) before fetch and selection work.
 */
export function ProviderTabbedView({
    mode,
    templates,
    onSaved,
    onRecordLoaded,
    onRefresh,
    onFormBusyChange,
    onModelsBusyChange,
    onModelsDirtyChange,
    onCreateKindChange,
    hidden = false,
}: ProviderTabbedViewProps): JSX.Element {
    const { t } = useTranslation();
    const formId = useId();
    const [tab, setTab] = useState<ProviderTab>("api");
    const [selectedCount, setSelectedCount] =
        useState<SelectedModelCount | null>(null);
    const apiTab = useRef<HTMLButtonElement>(null);
    const modelsTab = useRef<HTMLButtonElement>(null);
    const tabIdPrefix =
        mode.kind === "edit" ? `provider-${mode.record.id}` : "provider-create";

    const [modelsDirty, setModelsDirty] = useState(false);

    function selectTab(next: ProviderTab): void {
        if (tab !== next && modelsDirty) {
            const ok = window.confirm(t("providers.models.unsavedLeave"));
            if (!ok) return;
        }
        setTab(next);
    }

    function handleTabKey(event: KeyboardEvent<HTMLButtonElement>): void {
        let next: ProviderTab;
        switch (event.key) {
            case "ArrowLeft":
            case "ArrowRight":
                next = tab === "api" ? "models" : "api";
                break;
            case "Home":
                next = "api";
                break;
            case "End":
                next = "models";
                break;
            default:
                return;
        }
        event.preventDefault();
        selectTab(next);
        (next === "api" ? apiTab : modelsTab).current?.focus();
    }

    const apiPanelHidden = hidden || tab !== "api";
    const modelsPanelHidden = hidden || tab !== "models";
    const modelsTabText = formatModelsTabLabel(t, selectedCount);

    return (
        <div className={styles["edit"]}>
            <div
                className={styles["tabs"]}
                role="tablist"
                aria-label={t("providers.tabs.ariaLabel")}
            >
                <button
                    type="button"
                    className={styles["tab"]}
                    role="tab"
                    onKeyDown={handleTabKey}
                    aria-selected={tab === "api"}
                    ref={apiTab}
                    id={`${tabIdPrefix}-tab-api`}
                    aria-controls={`${tabIdPrefix}-panel-api`}
                    tabIndex={tab === "api" ? 0 : -1}
                    onClick={() => {
                        selectTab("api");
                    }}
                >
                    {t("providers.tabs.api")}
                </button>
                <button
                    type="button"
                    className={styles["tab"]}
                    role="tab"
                    onKeyDown={handleTabKey}
                    aria-selected={tab === "models"}
                    ref={modelsTab}
                    id={`${tabIdPrefix}-tab-models`}
                    aria-controls={`${tabIdPrefix}-panel-models`}
                    tabIndex={tab === "models" ? 0 : -1}
                    onClick={() => {
                        selectTab("models");
                    }}
                >
                    {modelsTabText}
                </button>
            </div>
            <div
                className={[styles["panel"], styles["panelApi"]]
                    .filter((value): value is string => value !== undefined)
                    .join(" ")}
                role="tabpanel"
                id={`${tabIdPrefix}-panel-api`}
                aria-labelledby={`${tabIdPrefix}-tab-api`}
                hidden={tab !== "api"}
                inert={tab !== "api"}
            >
                <ProviderForm
                    templates={templates}
                    mode={mode}
                    layout="tabPanel"
                    formId={formId}
                    primaryActionPlacement="external"
                    {...(onCreateKindChange !== undefined
                        ? { onCreateKindChange }
                        : {})}
                    onSaved={onSaved}
                    onRecordLoaded={onRecordLoaded}
                    onRefresh={onRefresh}
                    onBusyChange={onFormBusyChange}
                    hidden={apiPanelHidden}
                />
            </div>
            <div
                className={[styles["panel"], styles["panelModels"]]
                    .filter((value): value is string => value !== undefined)
                    .join(" ")}
                role="tabpanel"
                id={`${tabIdPrefix}-panel-models`}
                aria-labelledby={`${tabIdPrefix}-tab-models`}
                hidden={tab !== "models"}
                inert={tab !== "models"}
            >
                {mode.kind === "edit" ? (
                    <ProviderModels
                        providerId={mode.record.id}
                        layout="tabPanel"
                        hidden={modelsPanelHidden}
                        onBusyChange={onModelsBusyChange}
                        onDirtyChange={(dirty) => {
                            setModelsDirty(dirty);
                            onModelsDirtyChange?.(dirty);
                        }}
                        onSelectedCountChange={setSelectedCount}
                    />
                ) : (
                    <ProviderModelsCreateGate hidden={modelsPanelHidden} />
                )}
            </div>
        </div>
    );
}
