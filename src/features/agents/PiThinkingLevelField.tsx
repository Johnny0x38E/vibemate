import type { JSX } from "react";
import { useTranslation } from "react-i18next";
import { FieldSelect } from "../../components/FieldSelect";
import detailForm from "../../components/detailForm.module.css";
import type { EffortLevel, ProviderModel } from "../../lib/desktop/models";
import type { PiThinkingLevel } from "../../lib/desktop/pi";
import type { ProviderRecord } from "../../lib/desktop/providers";
import card from "./ConfigPreview.module.css";
import styles from "./AgentList.module.css";

/**
 * Pi levels a reasoning model accepts without a thinking-level mapping.
 * Extended levels require a verified binding-specific mapping or a user map.
 */
const UNMAPPED_PI_LEVELS: readonly PiThinkingLevel[] = [
    "off",
    "minimal",
    "low",
    "medium",
    "high",
];

/** The catalog spells "no reasoning" as `none`; Pi calls the same state `off`. */
const PI_LEVEL_BY_TOKEN: Record<EffortLevel, PiThinkingLevel> = {
    none: "off",
    minimal: "minimal",
    low: "low",
    medium: "medium",
    high: "high",
    xhigh: "xhigh",
    max: "max",
};

/** Whether the effective central state declares reasoning support for this model. */
function thinkingSupported(model: ProviderModel): boolean {
    const thinking =
        model.capabilityOverrides.thinking ?? model.capabilities.thinking.state;
    return thinking === "supported";
}

/**
 * Declared efforts expressible by the verified binding's Pi mapping.
 * Other extended levels are reported separately, not guessed or downgraded.
 * Rust rechecks the choice against the stored definition before anything is written.
 */
export function availablePiThinkingLevels(
    model: ProviderModel,
    provider?: ProviderRecord,
): PiThinkingLevel[] {
    if (!thinkingSupported(model) || model.thinkingEffort?.kind !== "listed")
        return [];
    const declared = new Set(
        model.thinkingEffort.levels.map((token) => PI_LEVEL_BY_TOKEN[token]),
    );
    const verifiedMap =
        model.resolvedProtocol === "chat_completions" &&
        ((provider?.kind === "deepseek" &&
            (model.modelId === "deepseek-flash" ||
                model.modelId === "deepseek-v4-pro") &&
            provider.baseUrl === "https://api.deepseek.com") ||
            (provider?.kind === "openrouter" &&
                provider.baseUrl === "https://openrouter.ai/api/v1"));
    const levels: readonly PiThinkingLevel[] = verifiedMap
        ? [...UNMAPPED_PI_LEVELS, "xhigh", "max"]
        : UNMAPPED_PI_LEVELS;
    return levels.filter((level) => declared.has(level));
}

/** Declared levels that Pi accepts only after a thinking-level mapping. */
function levelsNeedingMapping(model: ProviderModel): PiThinkingLevel[] {
    if (!thinkingSupported(model) || model.thinkingEffort?.kind !== "listed")
        return [];
    return model.thinkingEffort.levels
        .map((token) => PI_LEVEL_BY_TOKEN[token])
        .filter((level) => !UNMAPPED_PI_LEVELS.includes(level));
}

/**
 * One configuration-local Pi thinking-level choice. It is a draft that becomes a
 * native `modelThinkingLevels` entry only through preview and apply; this field
 * never edits central model metadata. The parent clears the draft when the
 * provider or model changes and discards any preview this choice produced.
 */
export function PiThinkingLevelField({
    model,
    provider,
    value,
    blocked,
    onChange,
}: {
    model: ProviderModel;
    provider?: ProviderRecord | undefined;
    value: PiThinkingLevel | null;
    blocked: boolean;
    onChange: (value: PiThinkingLevel | null) => void;
}): JSX.Element {
    const { t } = useTranslation();
    const levels = availablePiThinkingLevels(model, provider);
    const mapping = levelsNeedingMapping(model).filter(
        (level) => !levels.includes(level),
    );
    const supported = thinkingSupported(model);
    const unavailable = value !== null && !levels.includes(value);
    return (
        <>
            <div className={detailForm["row"]}>
                <label
                    className={detailForm["label"]}
                    htmlFor="agent-model-thinking"
                >
                    {t("agents.modelConfig.thinkingLabel")}
                </label>
                <span
                    className={[detailForm["value"], card["selectValue"]].join(
                        " ",
                    )}
                >
                    <FieldSelect
                        id="agent-model-thinking"
                        value={value ?? ""}
                        options={[
                            ...(unavailable
                                ? [
                                      {
                                          value,
                                          label: t(
                                              "agents.modelConfig.thinkingUnmappedValue",
                                              { value },
                                          ),
                                      },
                                  ]
                                : []),
                            {
                                value: "",
                                label: t("agents.modelConfig.thinkingInherit"),
                            },
                            ...levels.map((level) => ({
                                value: level,
                                label: level,
                            })),
                        ]}
                        size="fill"
                        blocked={blocked || levels.length === 0}
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
            {unavailable && (
                <p role="alert">
                    {t("agents.modelConfig.thinkingObsolete", { value })}
                </p>
            )}
            {!supported && (
                <p className={styles["hint"]}>
                    {t("agents.modelConfig.thinkingUnsupported")}
                </p>
            )}
            {supported && levels.length === 0 && (
                <p className={styles["hint"]}>
                    {t("agents.modelConfig.thinkingNoLevels")}
                </p>
            )}
            {mapping.length > 0 && (
                <p className={styles["hint"]}>
                    {t("agents.modelConfig.thinkingMappingNeeded", {
                        levels: mapping.join(", "),
                    })}
                </p>
            )}
            {model.thinkingEffort?.defaultLevel !== null &&
                model.thinkingEffort?.defaultLevel !== undefined && (
                    <p className={styles["hint"]}>
                        {t("agents.modelConfig.thinkingDeclaredDefault", {
                            value: model.thinkingEffort.defaultLevel,
                        })}
                    </p>
                )}
            {levels.length > 0 && (
                <p className={styles["hint"]}>
                    {t("agents.modelConfig.thinkingScope")}
                </p>
            )}
        </>
    );
}
