import { useEffect, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import { DetailPanel } from "../../components/DetailForm";
import {
    FieldSelect,
    type FieldSelectOption,
} from "../../components/FieldSelect";
import detailForm from "../../components/detailForm.module.css";
import buttons from "../../components/actionButtons.module.css";
import {
    PlanRequestError,
    planClaudeCodeSettings,
    type ClaudeCodePlan,
    type PlanErrorCode,
    type PlanNotice,
} from "../../lib/desktop/agents";
import {
    ModelRequestError,
    listProviderModels,
    type ModelErrorCode,
    type ProviderModel,
} from "../../lib/desktop/models";
import {
    ProviderRequestError,
    listProviders,
    type ProviderErrorCode,
    type ProviderRecord,
} from "../../lib/desktop/providers";
import styles from "./AgentList.module.css";
import card from "./ConfigPreview.module.css";

/** Saved instances are a small, hand-managed set; one page covers them. */
const PROVIDER_PAGE_LIMIT = 100;
/** Added models of one instance; Rust caps a model page at 200 rows. */
const MODEL_PAGE_LIMIT = 200;

type ProvidersState =
    | { kind: "loading" }
    | { kind: "preview" }
    | { kind: "error"; code: ProviderErrorCode }
    | { kind: "ready"; providers: ProviderRecord[] };

/** `null` means no answer for the current instance has arrived yet. */
type ModelsState =
    | { kind: "preview" }
    | { kind: "error"; providerId: string; code: ModelErrorCode }
    | { kind: "ready"; providerId: string; models: ProviderModel[] };

type PreviewState =
    | { kind: "idle" }
    | { kind: "loading" }
    | { kind: "preview" }
    | { kind: "error"; code: PlanErrorCode }
    | { kind: "ready"; plan: ClaudeCodePlan };

/**
 * Read-only link between one saved Provider instance and one of its added
 * models: the panel asks Rust for a sanitized Claude Code settings plan.
 *
 * Nothing is written, no credential value is read and no request is sent. Both
 * selections stay local to this panel, and changing either one discards the
 * shown plan, so a preview can never describe a pair the user no longer sees.
 */
export function ConfigPreview(): JSX.Element {
    const { t } = useTranslation();
    const [providers, setProviders] = useState<ProvidersState>({
        kind: "loading",
    });
    const [models, setModels] = useState<ModelsState | null>(null);
    const [preview, setPreview] = useState<PreviewState>({ kind: "idle" });
    const [providerId, setProviderId] = useState("");
    const [modelId, setModelId] = useState("");
    const [providersAttempt, setProvidersAttempt] = useState(0);
    const [modelsAttempt, setModelsAttempt] = useState(0);
    const pending = useRef(false);
    const mounted = useRef(false);
    const providersGeneration = useRef(0);
    const modelsGeneration = useRef(0);

    useEffect(() => {
        mounted.current = true;
        // A late reply must not update a view that is no longer mounted.
        return () => {
            mounted.current = false;
        };
    }, []);

    useEffect(() => {
        // A generation counter (not a captured flag) invalidates StrictMode's
        // discarded first request and every read that a newer one replaced.
        providersGeneration.current += 1;
        const generation = providersGeneration.current;
        listProviders({ after: null, limit: PROVIDER_PAGE_LIMIT }).then(
            (result) => {
                if (providersGeneration.current !== generation) return;
                if (result.kind === "preview") {
                    setProviders({ kind: "preview" });
                    return;
                }
                const items = result.page.items;
                setProviders({ kind: "ready", providers: items });
                // Keep the current instance while it is still saved; otherwise
                // start from the first one so the model list has an owner.
                setProviderId((current) =>
                    items.some((item) => item.id === current)
                        ? current
                        : (items[0]?.id ?? ""),
                );
            },
            (error: unknown) => {
                if (providersGeneration.current !== generation) return;
                setProviders({
                    kind: "error",
                    code:
                        error instanceof ProviderRequestError
                            ? error.code
                            : "operation_failed",
                });
            },
        );
        return () => {
            providersGeneration.current += 1;
        };
    }, [providersAttempt]);

    useEffect(() => {
        if (providerId === "") return;
        modelsGeneration.current += 1;
        const generation = modelsGeneration.current;
        const target = providerId;
        listProviderModels({
            providerId: target,
            after: null,
            limit: MODEL_PAGE_LIMIT,
            filter: "selected",
        }).then(
            (result) => {
                if (modelsGeneration.current !== generation) return;
                setModels(
                    result.kind === "preview"
                        ? { kind: "preview" }
                        : {
                              kind: "ready",
                              providerId: target,
                              models: result.page.items,
                          },
                );
            },
            (error: unknown) => {
                if (modelsGeneration.current !== generation) return;
                setModels({
                    kind: "error",
                    providerId: target,
                    code:
                        error instanceof ModelRequestError
                            ? error.code
                            : "operation_failed",
                });
            },
        );
        return () => {
            modelsGeneration.current += 1;
        };
    }, [providerId, modelsAttempt]);

    // Derived during render: an answer only belongs to the instance it was read
    // for, so a reply for a previous selection can never be displayed as current.
    const instances = providers.kind === "ready" ? providers.providers : null;
    const selectedProvider =
        instances?.find((record) => record.id === providerId) ?? null;
    const answer =
        models === null ||
        models.kind === "preview" ||
        models.providerId !== providerId
            ? null
            : models;
    const selectedModel =
        answer?.kind === "ready"
            ? (answer.models.find((model) => model.modelId === modelId) ?? null)
            : null;
    const canPreview =
        selectedProvider !== null &&
        selectedModel !== null &&
        preview.kind !== "loading";

    async function runPreview(): Promise<void> {
        // `canPreview` already proves both selections exist for this render.
        if (!canPreview || pending.current) return;
        pending.current = true;
        setPreview({ kind: "loading" });
        try {
            const result = await planClaudeCodeSettings(
                selectedProvider.id,
                selectedModel.modelId,
            );
            if (!mounted.current) return;
            setPreview(
                result.kind === "preview"
                    ? { kind: "preview" }
                    : { kind: "ready", plan: result.plan },
            );
        } catch (error: unknown) {
            if (!mounted.current) return;
            setPreview({
                kind: "error",
                code:
                    error instanceof PlanRequestError
                        ? error.code
                        : "operation_failed",
            });
        } finally {
            pending.current = false;
        }
    }

    if (providers.kind === "preview") {
        return (
            <DetailPanel label={t("agents.modelConfig.title")}>
                <p className={styles["hint"]}>
                    {t("agents.modelConfig.previewMode")}
                </p>
            </DetailPanel>
        );
    }

    const providerOptions: FieldSelectOption[] =
        instances?.map((record) => ({
            value: record.id,
            label: record.displayName,
        })) ?? [];
    const modelOptions: FieldSelectOption[] =
        answer?.kind === "ready"
            ? answer.models.map((model) => ({
                  value: model.modelId,
                  label:
                      model.alias === null
                          ? model.modelId
                          : `${model.alias} (${model.modelId})`,
              }))
            : [];

    return (
        <>
            <DetailPanel label={t("agents.modelConfig.title")}>
                {providers.kind === "loading" && (
                    <p className={styles["hint"]}>
                        {t("agents.modelConfig.loading")}
                    </p>
                )}
                {providers.kind === "error" && (
                    <>
                        <p className={styles["error"]} role="alert">
                            {t(`providers.errors.${providers.code}`)}
                        </p>
                        <p className={styles["hint"]}>
                            <button
                                type="button"
                                className={buttons["secondary"]}
                                onClick={() => {
                                    setProviders({ kind: "loading" });
                                    setProvidersAttempt(
                                        (attempt) => attempt + 1,
                                    );
                                }}
                            >
                                {t("providers.retry")}
                            </button>
                        </p>
                    </>
                )}
                {instances?.length === 0 && (
                    <p className={styles["hint"]}>
                        {t("agents.modelConfig.noProviders")}
                    </p>
                )}
                {instances !== null && instances.length > 0 && (
                    <>
                        <div className={detailForm["row"]}>
                            <label
                                className={detailForm["label"]}
                                htmlFor="agent-model-provider"
                            >
                                {t("agents.modelConfig.providerLabel")}
                            </label>
                            <span
                                className={[
                                    detailForm["value"],
                                    card["selectValue"],
                                ].join(" ")}
                            >
                                <FieldSelect
                                    id="agent-model-provider"
                                    value={providerId}
                                    options={providerOptions}
                                    size="fill"
                                    blocked={preview.kind === "loading"}
                                    onChange={(value) => {
                                        setProviderId(value);
                                        setModelId("");
                                        setPreview({ kind: "idle" });
                                    }}
                                />
                            </span>
                        </div>
                        {answer === null && (
                            <p className={styles["hint"]}>
                                {t("agents.modelConfig.loading")}
                            </p>
                        )}
                        {answer?.kind === "error" && (
                            <>
                                <p className={styles["error"]} role="alert">
                                    {t(
                                        `providers.models.errors.${answer.code}`,
                                    )}
                                </p>
                                <p className={styles["hint"]}>
                                    <button
                                        type="button"
                                        className={buttons["secondary"]}
                                        onClick={() => {
                                            setModels(null);
                                            setModelsAttempt(
                                                (attempt) => attempt + 1,
                                            );
                                        }}
                                    >
                                        {t("providers.models.retry")}
                                    </button>
                                </p>
                            </>
                        )}
                        {answer?.kind === "ready" &&
                            answer.models.length === 0 && (
                                <p className={styles["hint"]}>
                                    {t("agents.modelConfig.noModels")}
                                </p>
                            )}
                        {answer?.kind === "ready" &&
                            answer.models.length > 0 && (
                                <>
                                    <div className={detailForm["row"]}>
                                        <label
                                            className={detailForm["label"]}
                                            htmlFor="agent-model-model"
                                        >
                                            {t("agents.modelConfig.modelLabel")}
                                        </label>
                                        <span
                                            className={[
                                                detailForm["value"],
                                                card["selectValue"],
                                            ].join(" ")}
                                        >
                                            <FieldSelect
                                                id="agent-model-model"
                                                value={modelId}
                                                options={modelOptions}
                                                size="fill"
                                                blocked={
                                                    preview.kind === "loading"
                                                }
                                                onChange={(value) => {
                                                    setModelId(value);
                                                    setPreview({
                                                        kind: "idle",
                                                    });
                                                }}
                                            />
                                        </span>
                                    </div>
                                    <div className={card["actions"]}>
                                        <button
                                            type="button"
                                            className={buttons["primary"]}
                                            aria-disabled={!canPreview}
                                            onClick={() => {
                                                void runPreview();
                                            }}
                                        >
                                            {preview.kind === "loading"
                                                ? t(
                                                      "agents.modelConfig.previewing",
                                                  )
                                                : t(
                                                      "agents.modelConfig.preview",
                                                  )}
                                        </button>
                                    </div>
                                </>
                            )}
                    </>
                )}
                {preview.kind === "error" && (
                    <p className={styles["error"]} role="alert">
                        {t(`agents.modelConfig.errors.${preview.code}`)}
                    </p>
                )}
            </DetailPanel>
            {preview.kind === "ready" && <PlanCard plan={preview.plan} />}
        </>
    );
}

/** One sanitized plan: target file, key changes, notices and take-effect note. */
function PlanCard({ plan }: { plan: ClaudeCodePlan }): JSX.Element {
    const { t } = useTranslation();
    const fileState = plan.target.exists
        ? plan.target.symlink
            ? t("agents.modelConfig.fileStateSymlink")
            : t("agents.modelConfig.fileStateExists")
        : t("agents.modelConfig.fileStateNew");
    return (
        <DetailPanel label={t("agents.modelConfig.planHeading")}>
            <div className={card["summary"]}>
                <span className={card["path"]}>{plan.target.path}</span>
                <span className={card["fileState"]}>{fileState}</span>
                {plan.restartRequired && (
                    <span className={card["note"]}>
                        {t("agents.modelConfig.restartRequired")}
                    </span>
                )}
            </div>
            <ul className={card["changes"]}>
                {plan.changes.map((change) => (
                    <li key={change.keyPath} className={card["change"]}>
                        <span className={card["key"]}>{change.keyPath}</span>
                        <span className={card["transition"]}>
                            {change.before !== null && (
                                <>
                                    <span className={card["before"]}>
                                        {valueText(change.before, t)}
                                    </span>
                                    {/* Decorative: the two values already read
                                        as "previous" and "planned". */}
                                    <span
                                        className={card["arrow"]}
                                        aria-hidden="true"
                                    >
                                        →
                                    </span>
                                </>
                            )}
                            <span className={card["after"]}>
                                {change.secret
                                    ? t("agents.modelConfig.secretValue")
                                    : valueText(change.after, t)}
                            </span>
                        </span>
                    </li>
                ))}
            </ul>
            {plan.notices.length > 0 && (
                <ul className={card["notices"]}>
                    {plan.notices.map((notice) => (
                        <li key={notice.code}>{noticeText(notice, t)}</li>
                    ))}
                </ul>
            )}
        </DetailPanel>
    );
}

type Translator = ReturnType<typeof useTranslation>["t"];

/** Display text for one side of a change; an empty value stays visible. */
function valueText(value: string, t: Translator): string {
    return value === "" ? t("agents.modelConfig.emptyValue") : value;
}

/** Translated notice; `detail` carries the previous value when there is one. */
function noticeText(notice: PlanNotice, t: Translator): string {
    const key = `agents.modelConfig.notices.${notice.code}` as const;
    return notice.detail === null ? t(key) : t(key, { value: notice.detail });
}
