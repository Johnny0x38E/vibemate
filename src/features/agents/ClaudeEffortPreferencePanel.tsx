import type { JSX } from "react";
import { useTranslation } from "react-i18next";
import type { ProviderRecord } from "../../lib/desktop/providers";
import type { ClaudeEffortLevel } from "../../lib/desktop/agents";
import { ClaudeEffortField, availableClaudeEfforts } from "./ClaudeEffortField";
import type { ClaudePreferenceState } from "./useClaudeEffortPreference";
import buttons from "../../components/actionButtons.module.css";
import card from "./ConfigPreview.module.css";
import styles from "./AgentList.module.css";

/** Read/retry and explicit remember controls for one binding's starting draft.
 * A failed save keeps the draft; conflicts or uncertain outcomes require reloading.
 * None of these controls applies an Agent configuration.
 */
export function ClaudeEffortPreferencePanel({
    provider,
    state,
    blocked,
    onChange,
    onSave,
    onReload,
}: {
    provider: ProviderRecord;
    state: ClaudePreferenceState;
    blocked: boolean;
    onChange: (value: ClaudeEffortLevel | null) => void;
    onSave: () => void;
    onReload: () => void;
}): JSX.Element | null {
    const { t } = useTranslation();
    if (state.kind === "inactive") return null;
    if (state.kind === "loading")
        return <p role="status">{t("agents.modelConfig.preferenceLoading")}</p>;
    if (state.kind === "preview")
        return <p>{t("agents.modelConfig.preferencePreview")}</p>;
    if (state.kind === "readError")
        return (
            <>
                <p role="alert">
                    {t(`agents.modelConfig.preferenceErrors.${state.code}`)}
                </p>
                <button
                    type="button"
                    className={buttons["secondary"]}
                    aria-disabled={blocked}
                    onClick={() => {
                        if (!blocked) onReload();
                    }}
                >
                    {t("agents.modelConfig.preferenceReload")}
                </button>
            </>
        );
    const valid =
        state.draft === null ||
        availableClaudeEfforts(
            provider,
            state.preference.snapshot.model,
        ).includes(state.draft);
    const retryable =
        state.kind === "ready" &&
        (state.outcome.kind !== "error" ||
            state.outcome.code === "write_failed");
    const canSave =
        !blocked &&
        state.kind === "ready" &&
        valid &&
        retryable &&
        state.draft !== state.preference.selectedEffort;
    return (
        <>
            <ClaudeEffortField
                provider={provider}
                model={state.preference.snapshot.model}
                value={state.draft}
                blocked={blocked || state.kind === "saving"}
                onChange={onChange}
            />
            <p className={styles["hint"]}>
                {t("agents.modelConfig.preferenceScope")}
            </p>
            <div className={card["actions"]}>
                <button
                    type="button"
                    className={buttons["secondary"]}
                    aria-disabled={!canSave}
                    onClick={() => {
                        if (canSave) onSave();
                    }}
                >
                    {t(
                        state.kind === "saving"
                            ? "agents.modelConfig.preferenceSaving"
                            : "agents.modelConfig.preferenceSave",
                    )}
                </button>
                {state.kind === "ready" && state.outcome.kind === "error" && (
                    <button
                        type="button"
                        className={buttons["secondary"]}
                        aria-disabled={blocked}
                        onClick={() => {
                            if (!blocked) onReload();
                        }}
                    >
                        {t("agents.modelConfig.preferenceReload")}
                    </button>
                )}
            </div>
            {state.kind === "saving" && (
                <p role="status">{t("agents.modelConfig.preferenceSaving")}</p>
            )}
            {state.kind === "ready" && state.outcome.kind === "saved" && (
                <p role="status">{t("agents.modelConfig.preferenceSaved")}</p>
            )}
            {state.kind === "ready" && state.outcome.kind === "error" && (
                <p role="alert">
                    {t(
                        `agents.modelConfig.preferenceErrors.${state.outcome.code}`,
                    )}
                </p>
            )}
        </>
    );
}
