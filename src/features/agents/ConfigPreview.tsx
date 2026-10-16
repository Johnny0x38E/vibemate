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
    ApplyRequestError,
    PlanRequestError,
    applyClaudeCodeSettings,
    planClaudeCodeSettings,
    reviewedPlanTokens,
    type ApplyErrorCode,
    type ApplyStep,
    type ClaudeCodeApplied,
    type ClaudeCodePlan,
    type PlanErrorCode,
    type PlanNotice,
    type WriteErrorCode,
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
import {
    planPiSettings,
    applyPiSettings,
    reviewedPiPlan,
    type PiPlan,
} from "../../lib/desktop/pi";
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
    | { kind: "ready"; plan: ClaudeCodePlan | PiPlan };

/** The apply step's own outcome; it outlives the plan it was built from. */
type ApplyState =
    | { kind: "idle" }
    | { kind: "applying" }
    | { kind: "error"; code: ApplyErrorCode }
    | { kind: "done"; result: ClaudeCodeApplied };

/**
 * Preview and apply one saved Provider/model through its native Agent adapter.
 *
 * Preview is credential-blind; applying explicitly approves the shown files. Both
 * selections stay local to this panel, and changing either one discards the
 * shown plan, so a preview can never describe a pair the user no longer sees.
 */
export function ConfigPreview({
    agent = "claudeCode",
    entryPath = null,
    onBusyChange,
    onApplied,
    blocked = false,
}: {
    agent?: "claudeCode" | "pi";
    entryPath?: string | null;
    onBusyChange?: (busy: boolean) => void;
    onApplied?: () => void;
    blocked?: boolean;
}): JSX.Element {
    const { t } = useTranslation();
    const [providers, setProviders] = useState<ProvidersState>({
        kind: "loading",
    });
    const [models, setModels] = useState<ModelsState | null>(null);
    const [preview, setPreview] = useState<PreviewState>({ kind: "idle" });
    const [apply, setApply] = useState<ApplyState>({ kind: "idle" });
    const [providerId, setProviderId] = useState("");
    const [modelId, setModelId] = useState("");
    const [providersAttempt, setProvidersAttempt] = useState(0);
    const [modelsAttempt, setModelsAttempt] = useState(0);
    const pending = useRef(false);
    const busyCallback = useRef(onBusyChange);
    useEffect(() => {
        // Keep cleanup and late async completions bound to the current parent.
        busyCallback.current = onBusyChange;
    }, [onBusyChange]);
    const mounted = useRef(false);
    const providersGeneration = useRef(0);
    const modelsGeneration = useRef(0);

    useEffect(() => {
        mounted.current = true;
        // A late reply must not update a view that is no longer mounted.
        return () => {
            mounted.current = false;
            busyCallback.current?.(false);
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
        !blocked &&
        selectedProvider !== null &&
        selectedModel !== null &&
        preview.kind !== "loading" &&
        apply.kind !== "applying";

    async function runPreview(): Promise<void> {
        // `canPreview` already proves both selections exist for this render.
        if (!canPreview || pending.current) return;
        pending.current = true;
        busyCallback.current?.(true);
        // A new preview replaces the previous outcome, which described another
        // plan and would otherwise sit under the card it no longer belongs to.
        setApply({ kind: "idle" });
        setPreview({ kind: "loading" });
        try {
            const result =
                agent === "pi"
                    ? await planPiSettings(
                          selectedProvider.id,
                          selectedModel.modelId,
                          entryPath,
                      )
                    : await planClaudeCodeSettings(
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
            busyCallback.current?.(false);
        }
    }

    /**
     * Write the shown plan.
     *
     * The tokens travel back with the request, so Rust refuses a plan whose
     * settings file, Provider revision or model content changed since the
     * preview. Any answer clears the plan: the file may have changed, and a
     * second click must build and review a fresh preview instead of repeating
     * this write.
     */
    async function runApply(): Promise<void> {
        if (blocked || preview.kind !== "ready" || pending.current) return;
        const plan = preview.plan;
        pending.current = true;
        busyCallback.current?.(true);
        setApply({ kind: "applying" });
        try {
            const result =
                plan.agent === "pi"
                    ? await applyPiSettings(
                          plan.source.providerId,
                          plan.source.modelId,
                          entryPath,
                          reviewedPiPlan(plan),
                      )
                    : await applyClaudeCodeSettings(
                          plan.source.providerId,
                          plan.source.modelId,
                          reviewedPlanTokens(plan),
                      );
            if (!mounted.current) return;
            setPreview({ kind: "idle" });
            setApply({ kind: "done", result });
            onApplied?.();
        } catch (error: unknown) {
            if (!mounted.current) return;
            const code =
                error instanceof ApplyRequestError
                    ? error.code
                    : "operation_failed";
            // A stale plan cannot be applied at all; the other refusals leave the
            // file untouched, so the user can fix the cause and preview again.
            if (
                code === "plan_stale" ||
                code === "invalid_response" ||
                code === "operation_failed"
            )
                setPreview({ kind: "idle" });
            setApply({ kind: "error", code });
        } finally {
            pending.current = false;
            busyCallback.current?.(false);
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
                                    blocked={
                                        blocked ||
                                        preview.kind === "loading" ||
                                        apply.kind === "applying"
                                    }
                                    onChange={(value) => {
                                        setProviderId(value);
                                        setModelId("");
                                        setPreview({ kind: "idle" });
                                        setApply({ kind: "idle" });
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
                                                    preview.kind ===
                                                        "loading" ||
                                                    apply.kind === "applying"
                                                }
                                                onChange={(value) => {
                                                    setModelId(value);
                                                    setPreview({
                                                        kind: "idle",
                                                    });
                                                    setApply({
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
            {preview.kind === "ready" && (
                <PlanCard
                    plan={preview.plan}
                    applying={blocked || apply.kind === "applying"}
                    onApply={() => {
                        void runApply();
                    }}
                />
            )}
            {apply.kind === "done" && (
                <AppliedCard result={apply.result} agent={agent} />
            )}
            {apply.kind === "error" && (
                <p className={styles["error"]} role="alert">
                    {t(`agents.modelConfig.errors.${apply.code}`)}
                </p>
            )}
        </>
    );
}

/** One sanitized plan: target file, key changes, notices and the apply action. */
function PlanCard({
    plan,
    applying,
    onApply,
}: {
    plan: ClaudeCodePlan | PiPlan;
    applying: boolean;
    onApply: () => void;
}): JSX.Element {
    const { t } = useTranslation();
    const files =
        plan.agent === "pi"
            ? plan.files
            : [{ target: plan.target, changes: plan.changes }];
    return (
        <DetailPanel label={t("agents.modelConfig.planHeading")}>
            {files.map(({ target, changes }) => (
                <div key={target.path}>
                    <div className={card["summary"]}>
                        <span className={card["path"]}>{target.path}</span>
                        <span className={card["fileState"]}>
                            {target.exists
                                ? target.symlink
                                    ? t("agents.modelConfig.fileStateSymlink")
                                    : t("agents.modelConfig.fileStateExists")
                                : t("agents.modelConfig.fileStateNew")}
                        </span>
                        {plan.restartRequired && (
                            <span className={card["note"]}>
                                {t(
                                    plan.agent === "pi"
                                        ? "agents.modelConfig.piRestartRequired"
                                        : "agents.modelConfig.restartRequired",
                                )}
                            </span>
                        )}
                    </div>
                    <ul className={card["changes"]}>
                        {changes.map((change) => (
                            <li key={change.keyPath} className={card["change"]}>
                                <span className={card["key"]}>
                                    {change.keyPath}
                                </span>
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
                                            ? t(
                                                  "agents.modelConfig.secretValue",
                                              )
                                            : valueText(change.after, t)}
                                    </span>
                                </span>
                            </li>
                        ))}
                    </ul>
                </div>
            ))}
            {plan.agent === "pi" && (
                <p className={styles["hint"]}>
                    {t("agents.modelConfig.piNotice")}
                </p>
            )}
            {plan.agent === "claudeCode" && plan.notices.length > 0 && (
                <ul className={card["notices"]}>
                    {plan.notices.map((notice) => (
                        <li key={notice.code}>{noticeText(notice, t)}</li>
                    ))}
                </ul>
            )}
            <div className={card["actions"]}>
                <button
                    type="button"
                    className={buttons["primary"]}
                    aria-disabled={applying}
                    onClick={onApply}
                >
                    {applying
                        ? t("agents.modelConfig.applying")
                        : t("agents.modelConfig.apply")}
                </button>
            </div>
        </DetailPanel>
    );
}

/** What an apply call did: the settled state, failed files and the backup. */
function AppliedCard({
    result,
    agent,
}: {
    result: ClaudeCodeApplied;
    agent: "claudeCode" | "pi";
}): JSX.Element {
    const { t } = useTranslation();
    // A failed step always carries its code; the IPC boundary refuses other shapes.
    const failed = result.steps.filter(
        (step): step is ApplyStep & { errorCode: WriteErrorCode } =>
            step.state === "failed" && step.errorCode !== null,
    );
    return (
        <DetailPanel label={t("agents.modelConfig.outcomeHeading")}>
            <div className={card["summary"]}>
                <span className={card["path"]}>
                    {t(
                        agent === "pi"
                            ? `agents.modelConfig.piOutcome.${result.state}`
                            : `agents.modelConfig.outcome.${result.state}`,
                    )}
                </span>
                {result.steps
                    .filter((step) => step.backupPath !== null)
                    .map((step) => (
                        <span className={card["note"]} key={step.ordinal}>
                            {t("agents.modelConfig.backupAt", {
                                path: step.backupPath,
                            })}
                        </span>
                    ))}
                {result.restartRequired && (
                    <span className={card["note"]}>
                        {t(
                            agent === "pi"
                                ? "agents.modelConfig.piRestartRequired"
                                : "agents.modelConfig.restartRequired",
                        )}
                    </span>
                )}
            </div>
            {failed.length > 0 && (
                <ul className={card["notices"]}>
                    {failed.map((step) => (
                        <li key={step.ordinal}>
                            {t("agents.modelConfig.stepFailed", {
                                path: step.targetPath,
                                error: t(
                                    `agents.modelConfig.writeErrors.${step.errorCode}`,
                                ),
                            })}
                        </li>
                    ))}
                </ul>
            )}
            <p className={styles["hint"]}>
                {t("agents.modelConfig.applyPreviewAgain")}
            </p>
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
