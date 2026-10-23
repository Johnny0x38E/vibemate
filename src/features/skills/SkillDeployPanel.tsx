import { useEffect, useId, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import { DetailFact, DetailPanel } from "../../components/DetailForm";
import { FieldSelect } from "../../components/FieldSelect";
import {
    applyPiSkillDeploy,
    disablePiSkillDeploy,
    listSkillDeployments,
    previewPiSkillDeploy,
    SkillRequestError,
    type PiSkillDeployPreview,
    type SkillDeployMode,
    type SkillDeploymentRecord,
    type SkillErrorCode,
    type SkillRecord,
} from "../../lib/desktop/skills";
import buttons from "../../components/actionButtons.module.css";
import styles from "./SkillLibrary.module.css";

type PanelState =
    | { kind: "loading" }
    | { kind: "preview_only" }
    | {
          kind: "ready";
          preview: PiSkillDeployPreview | null;
          deployments: SkillDeploymentRecord[];
      }
    | { kind: "error"; code: SkillErrorCode; uncertain: boolean };

type Answer = {
    skillId: string;
    skillRevision: number;
    state: PanelState;
};

function errorCode(error: unknown): SkillErrorCode {
    return error instanceof SkillRequestError ? error.code : "operation_failed";
}

function uncertain(code: SkillErrorCode): boolean {
    return [
        "operation_failed",
        "invalid_response",
        "outcome_unknown",
        "write_failed",
        "storage_unavailable",
        "revision_conflict",
        "target_changed",
    ].includes(code);
}

/** Inputs for {@link SkillDeployPanel}. */
export interface SkillDeployPanelProps {
    record: SkillRecord;
    /** Protect sidebar navigation while a deploy or disable is pending. */
    onBusyChange?: (busy: boolean) => void;
}

/**
 * Deploy one library skill into Pi's user skills directory. Symlink is preferred
 * when available; copy-sync is an explicit choice. Foreign targets are shown but
 * never overwritten. Disable only removes a still-owned target.
 */
export function SkillDeployPanel({
    record,
    onBusyChange,
}: SkillDeployPanelProps): JSX.Element {
    const { t } = useTranslation();
    const prefix = useId();
    const skillId = record.id;
    const skillRevision = record.revision;
    const [answer, setAnswer] = useState<Answer>({
        skillId,
        skillRevision,
        state: { kind: "loading" },
    });
    const [mode, setMode] = useState<SkillDeployMode>("symlink");
    const [busy, setBusy] = useState(false);
    const [actionError, setActionError] = useState<{
        code: SkillErrorCode;
        uncertain: boolean;
    } | null>(null);
    const [attempt, setAttempt] = useState(0);
    const generation = useRef(0);

    const sameRecord =
        answer.skillId === skillId && answer.skillRevision === skillRevision;
    // Reset in render when the detail page shows a different skill or revision,
    // so the effect never calls setState synchronously for "loading".
    if (!sameRecord) {
        setAnswer({ skillId, skillRevision, state: { kind: "loading" } });
    }
    const state: PanelState = sameRecord ? answer.state : { kind: "loading" };

    useEffect(() => {
        onBusyChange?.(busy);
        return () => {
            onBusyChange?.(false);
        };
    }, [busy, onBusyChange]);

    useEffect(() => {
        const token = ++generation.current;
        void (async () => {
            try {
                const [previewResult, listResult] = await Promise.all([
                    previewPiSkillDeploy(skillId),
                    listSkillDeployments(skillId),
                ]);
                if (generation.current !== token) return;
                if (
                    previewResult.kind === "preview" ||
                    listResult.kind === "preview"
                ) {
                    setAnswer({
                        skillId,
                        skillRevision,
                        state: { kind: "preview_only" },
                    });
                    return;
                }
                const preview = previewResult.preview;
                setMode(
                    preview.symlinkAvailable ? preview.suggestedMode : "copy",
                );
                setAnswer({
                    skillId,
                    skillRevision,
                    state: {
                        kind: "ready",
                        preview,
                        deployments: listResult.items,
                    },
                });
            } catch (error: unknown) {
                if (generation.current !== token) return;
                const code = errorCode(error);
                setAnswer({
                    skillId,
                    skillRevision,
                    state: { kind: "error", code, uncertain: uncertain(code) },
                });
            }
        })();
        return () => {
            generation.current += 1;
        };
    }, [skillId, skillRevision, attempt]);

    function reload(): void {
        setActionError(null);
        setAttempt((value) => value + 1);
    }

    async function deploy(preview: PiSkillDeployPreview): Promise<void> {
        if (busy || !preview.canApply) return;
        const chosen =
            !preview.symlinkAvailable || mode === "copy" ? "copy" : mode;
        if (chosen === "symlink" && !preview.symlinkAvailable) return;
        setBusy(true);
        setActionError(null);
        try {
            await applyPiSkillDeploy({
                skillId: preview.skillId,
                expectedSkillRevision: preview.skillRevision,
                configRoot: preview.configRoot,
                mode: chosen,
                targetState: preview.targetState,
            });
            reload();
        } catch (error: unknown) {
            const code = errorCode(error);
            setActionError({ code, uncertain: uncertain(code) });
            if (uncertain(code)) reload();
        } finally {
            setBusy(false);
        }
    }

    async function disable(row: SkillDeploymentRecord): Promise<void> {
        if (busy) return;
        setBusy(true);
        setActionError(null);
        try {
            await disablePiSkillDeploy({
                skillId: row.skillId,
                configRoot: row.configRoot,
            });
            reload();
        } catch (error: unknown) {
            const code = errorCode(error);
            setActionError({ code, uncertain: uncertain(code) });
            if (uncertain(code)) reload();
        } finally {
            setBusy(false);
        }
    }

    return (
        <DetailPanel label={t("skills.deploy.title")}>
            {state.kind === "loading" && (
                <p className={styles["hint"]}>{t("skills.deploy.loading")}</p>
            )}
            {state.kind === "preview_only" && (
                <p className={styles["hint"]}>
                    {t("skills.deploy.desktopRequired")}
                </p>
            )}
            {state.kind === "error" && (
                <>
                    <p className={styles["error"]} role="alert">
                        {t(`skills.errors.${state.code}`)}
                    </p>
                    <button
                        className={buttons["secondary"]}
                        type="button"
                        onClick={reload}
                    >
                        {t("skills.reload")}
                    </button>
                </>
            )}
            {state.kind === "ready" &&
                (() => {
                    const preview = state.preview;
                    if (preview === null) return null;
                    return (
                        <>
                            <DetailFact
                                label={t("skills.deploy.target")}
                                value={
                                    <p className={styles["longValue"]}>
                                        {preview.targetPath}
                                    </p>
                                }
                            />
                            <DetailFact
                                label={t("skills.deploy.targetState")}
                                value={t(
                                    `skills.deploy.states.${preview.targetState}`,
                                )}
                            />
                            <DetailFact
                                label={t("skills.deploy.mode")}
                                value={
                                    <FieldSelect
                                        id={`${prefix}-mode`}
                                        value={
                                            preview.symlinkAvailable
                                                ? mode
                                                : "copy"
                                        }
                                        options={[
                                            ...(preview.symlinkAvailable
                                                ? [
                                                      {
                                                          value: "symlink",
                                                          label: t(
                                                              "skills.deploy.modeSymlink",
                                                          ),
                                                      },
                                                  ]
                                                : []),
                                            {
                                                value: "copy",
                                                label: t(
                                                    "skills.deploy.modeCopy",
                                                ),
                                            },
                                        ]}
                                        blocked={busy || !preview.canApply}
                                        size="fill"
                                        onChange={(value) => {
                                            if (
                                                value === "symlink" ||
                                                value === "copy"
                                            ) {
                                                setMode(value);
                                            }
                                        }}
                                    />
                                }
                            />
                            {!preview.symlinkAvailable && (
                                <p className={styles["hint"]}>
                                    {t("skills.deploy.symlinkUnavailable")}
                                </p>
                            )}
                            {!preview.canApply && (
                                <p className={styles["hint"]}>
                                    {t("skills.deploy.foreignHint")}
                                </p>
                            )}
                            <div className={styles["deployActions"]}>
                                <button
                                    className={buttons["primary"]}
                                    type="button"
                                    aria-disabled={busy || !preview.canApply}
                                    onClick={() => {
                                        void deploy(preview);
                                    }}
                                >
                                    {busy
                                        ? t("skills.deploy.applying")
                                        : t("skills.deploy.apply")}
                                </button>
                            </div>
                        </>
                    );
                })()}
            {state.kind === "ready" &&
                (state.deployments.length === 0 ? (
                    <p className={styles["hint"]}>{t("skills.deploy.none")}</p>
                ) : (
                    <ul
                        className={styles["deployList"]}
                        aria-label={t("skills.deploy.listLabel")}
                    >
                        {state.deployments.map((row) => (
                            <li key={`${row.agentId}:${row.configRoot}`}>
                                <div className={styles["deployRow"]}>
                                    <p className={styles["longValue"]}>
                                        {t("skills.deploy.rowSummary", {
                                            agent: t("skills.deploy.agentPi"),
                                            mode: t(
                                                row.mode === "symlink"
                                                    ? "skills.deploy.modeSymlink"
                                                    : "skills.deploy.modeCopy",
                                            ),
                                            revision: row.skillRevision,
                                            sync: t(
                                                row.inSync
                                                    ? "skills.deploy.inSync"
                                                    : "skills.deploy.outOfSync",
                                            ),
                                        })}
                                    </p>
                                    <button
                                        className={buttons["secondary"]}
                                        type="button"
                                        aria-disabled={busy}
                                        onClick={() => {
                                            void disable(row);
                                        }}
                                    >
                                        {t("skills.deploy.disable")}
                                    </button>
                                </div>
                            </li>
                        ))}
                    </ul>
                ))}
            {actionError !== null && (
                <>
                    <p className={styles["error"]} role="alert">
                        {t(`skills.errors.${actionError.code}`)}
                    </p>
                    {actionError.uncertain && (
                        <p className={styles["hint"]}>
                            {t("skills.reloadUnknown")}
                        </p>
                    )}
                </>
            )}
        </DetailPanel>
    );
}
