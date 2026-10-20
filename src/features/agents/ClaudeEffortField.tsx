import type { JSX } from "react";
import { useTranslation } from "react-i18next";
import { FieldSelect } from "../../components/FieldSelect";
import detailForm from "../../components/detailForm.module.css";
import {
    CLAUDE_EFFORT_LEVELS,
    type ClaudeEffortLevel,
} from "../../lib/desktop/agents";
import type { ProviderModel } from "../../lib/desktop/models";
import type { ProviderRecord } from "../../lib/desktop/providers";
import card from "./ConfigPreview.module.css";
import styles from "./AgentList.module.css";

/** Model-declared options the current native adapter can express without conversion.
 * Rust rechecks the stored facts; this intersection only improves form feedback.
 */
export function availableClaudeEfforts(
    provider: ProviderRecord,
    model: ProviderModel,
): ClaudeEffortLevel[] {
    const thinking =
        model.capabilityOverrides.thinking ?? model.capabilities.thinking.state;
    if (
        provider.kind !== "deepseek" ||
        model.modelId !== "deepseek-flash" ||
        thinking !== "supported" ||
        model.thinkingEffort?.kind !== "listed"
    )
        return [];
    const declared = model.thinkingEffort.levels;
    return CLAUDE_EFFORT_LEVELS.filter((level) => declared.includes(level));
}

/** One configuration-local effort draft; it never edits central model metadata.
 * The parent discards its preview and clears the draft when the selected pair changes.
 */
export function ClaudeEffortField({
    provider,
    model,
    value,
    blocked,
    onChange,
}: {
    provider: ProviderRecord;
    model: ProviderModel;
    value: ClaudeEffortLevel | null;
    blocked: boolean;
    onChange: (value: ClaudeEffortLevel | null) => void;
}): JSX.Element {
    const { t } = useTranslation();
    const levels = availableClaudeEfforts(provider, model);
    const defaultLevel = model.thinkingEffort?.defaultLevel ?? null;
    const unmapped = value !== null && !levels.includes(value);
    return (
        <>
            <div className={detailForm["row"]}>
                <label
                    className={detailForm["label"]}
                    htmlFor="agent-model-effort"
                >
                    {t("agents.modelConfig.effortLabel")}
                </label>
                <span
                    className={[detailForm["value"], card["selectValue"]].join(
                        " ",
                    )}
                >
                    <FieldSelect
                        id="agent-model-effort"
                        value={value ?? ""}
                        options={[
                            ...(unmapped
                                ? [
                                      {
                                          value,
                                          label: t(
                                              "agents.modelConfig.effortUnmappedValue",
                                              { value },
                                          ),
                                      },
                                  ]
                                : []),
                            {
                                value: "",
                                label: t("agents.modelConfig.effortInherit"),
                            },
                            ...levels.map((level) => ({
                                value: level,
                                label: level,
                            })),
                        ]}
                        size="fill"
                        blocked={blocked}
                        onChange={(next) => {
                            if (next === "") onChange(null);
                            else {
                                const level = levels.find(
                                    (candidate) => candidate === next,
                                );
                                if (level !== undefined) onChange(level);
                            }
                        }}
                    />
                </span>
            </div>
            {unmapped && (
                <p role="alert">
                    {t("agents.modelConfig.effortObsolete", { value })}
                </p>
            )}
            <p className={styles["hint"]}>
                {t("agents.modelConfig.effortScope")}
            </p>
            {value !== null && (
                <p className={styles["hint"]}>
                    {t("agents.modelConfig.effortEffective")}
                </p>
            )}
            {levels.length === 0 && (
                <p className={styles["hint"]}>
                    {t("agents.modelConfig.effortUnavailable")}
                </p>
            )}
            {defaultLevel !== null && (
                <p className={styles["hint"]}>
                    {t("agents.modelConfig.effortDefault", {
                        value: defaultLevel,
                    })}
                </p>
            )}
        </>
    );
}
