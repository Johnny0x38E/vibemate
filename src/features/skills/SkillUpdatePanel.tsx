import { useId, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import { DetailFact, DetailPanel } from "../../components/DetailForm";
import {
    applySkillUpdate,
    checkSkillRevision,
    previewSkillUpdate,
    SkillRequestError,
    type SkillErrorCode,
    type SkillRecord,
    type SkillUpdateApplied,
    type SkillUpdatePreview,
    type SkillUpdateStrategy,
} from "../../lib/desktop/skills";
import buttons from "../../components/actionButtons.module.css";
import styles from "./SkillLibrary.module.css";

type PanelState =
    | { kind: "idle" }
    | { kind: "checking_revision" | "checking_digest" | "applying" }
    | { kind: "preview_only" }
    | { kind: "ready"; preview: SkillUpdatePreview }
    | { kind: "applied"; result: SkillUpdateApplied }
    | { kind: "error"; code: SkillErrorCode; uncertain: boolean };

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
        "install_timed_out",
        "installer_failed",
        "installer_unavailable",
        "source_changed",
        "revision_check_failed",
        "update_plan_stale",
    ].includes(code);
}

function shortDigest(value: string): string {
    return `${value.slice(0, 12)}…`;
}

/** Inputs for {@link SkillUpdatePanel}. */
export interface SkillUpdatePanelProps {
    record: SkillRecord;
    /** Refresh the parent detail page after a stored revision check or apply. */
    onRecordChange?: (record: SkillRecord) => void;
    /** Protect sidebar navigation while a check or apply is pending. */
    onBusyChange?: (busy: boolean) => void;
}

/**
 * Check whether remote Git `HEAD` moved, preview content diffs, and apply a
 * reviewed keep or replace strategy. Revision check only writes check metadata;
 * digest preview never writes; apply re-checks digests before acting.
 */
export function SkillUpdatePanel({
    record,
    onRecordChange,
    onBusyChange,
}: SkillUpdatePanelProps): JSX.Element {
    const { t } = useTranslation();
    const prefix = useId();
    const [state, setState] = useState<PanelState>({ kind: "idle" });

    const updatable = record.fetchMethod !== null && record.sourceUrl !== null;
    const gitCheckable =
        record.fetchMethod === "git" &&
        record.sourceUrl !== null &&
        record.sourceRevision !== null;
    const busy =
        state.kind === "checking_revision" ||
        state.kind === "checking_digest" ||
        state.kind === "applying";

    async function checkRevision(): Promise<void> {
        if (!gitCheckable || busy) return;
        setState({ kind: "checking_revision" });
        onBusyChange?.(true);
        try {
            const updated = await checkSkillRevision(record.id);
            onRecordChange?.(updated);
            setState({ kind: "idle" });
        } catch (error: unknown) {
            const code = errorCode(error);
            setState({ kind: "error", code, uncertain: uncertain(code) });
        } finally {
            onBusyChange?.(false);
        }
    }

    async function checkDigest(): Promise<void> {
        if (!updatable || busy) return;
        setState({ kind: "checking_digest" });
        onBusyChange?.(true);
        try {
            const result = await previewSkillUpdate({
                id: record.id,
                expectedRevision: record.revision,
            });
            if (result.kind === "preview") {
                setState({ kind: "preview_only" });
                return;
            }
            setState({ kind: "ready", preview: result.preview });
        } catch (error: unknown) {
            const code = errorCode(error);
            setState({ kind: "error", code, uncertain: uncertain(code) });
        } finally {
            onBusyChange?.(false);
        }
    }

    async function apply(strategy: SkillUpdateStrategy): Promise<void> {
        if (state.kind !== "ready" || busy) return;
        const { preview } = state;
        if (!preview.strategies.includes(strategy)) return;
        setState({ kind: "applying" });
        onBusyChange?.(true);
        try {
            const result = await applySkillUpdate({
                id: preview.skillId,
                expectedRevision: preview.skillRevision,
                strategy,
                expectedStatus: preview.status,
                expectedRecordedDigest: preview.recordedContentDigest,
                expectedLiveDigest: preview.liveContentDigest,
                expectedCandidateDigest: preview.candidateContentDigest,
                expectedCandidateSourceRevision:
                    preview.candidateSourceRevision,
            });
            onRecordChange?.(result.skill);
            setState({ kind: "applied", result });
        } catch (error: unknown) {
            const code = errorCode(error);
            setState({ kind: "error", code, uncertain: uncertain(code) });
        } finally {
            onBusyChange?.(false);
        }
    }

    return (
        <DetailPanel label={t("skills.update.title")}>
            {!updatable && (
                <p className={styles["hint"]}>
                    {t("skills.update.unavailable")}
                </p>
            )}
            {updatable && state.kind === "idle" && (
                <p className={styles["hint"]}>{t("skills.update.idle")}</p>
            )}
            {gitCheckable && record.updateCheckStatus !== null && (
                <>
                    <DetailFact
                        label={t("skills.update.revisionStatus")}
                        value={t(
                            `skills.update.revisionStatuses.${record.updateCheckStatus}`,
                        )}
                    />
                    {record.remoteRevision !== null && (
                        <DetailFact
                            label={t("skills.update.remoteRevision")}
                            value={record.remoteRevision}
                        />
                    )}
                    {record.lastCheckedAtMs !== null && (
                        <DetailFact
                            label={t("skills.update.lastChecked")}
                            value={new Date(
                                record.lastCheckedAtMs,
                            ).toLocaleString()}
                        />
                    )}
                    {record.updateCheckStatus === "error" &&
                        record.lastCheckError !== null && (
                            <p className={styles["error"]} role="alert">
                                {t(`skills.errors.${record.lastCheckError}`)}
                            </p>
                        )}
                </>
            )}
            {updatable && (
                <div className={styles["deployActions"]}>
                    {gitCheckable && (
                        <button
                            className={buttons["secondary"]}
                            type="button"
                            aria-disabled={busy}
                            onClick={() => {
                                void checkRevision();
                            }}
                        >
                            {state.kind === "checking_revision"
                                ? t("skills.update.checkingRevision")
                                : t("skills.update.checkRevision")}
                        </button>
                    )}
                    <button
                        id={`${prefix}-digest`}
                        className={buttons["primary"]}
                        type="button"
                        aria-disabled={busy}
                        onClick={() => {
                            void checkDigest();
                        }}
                    >
                        {state.kind === "checking_digest"
                            ? t("skills.update.checking")
                            : t("skills.update.check")}
                    </button>
                </div>
            )}
            {state.kind === "preview_only" && (
                <p className={styles["hint"]}>
                    {t("skills.update.desktopRequired")}
                </p>
            )}
            {state.kind === "error" && (
                <>
                    <p className={styles["error"]} role="alert">
                        {t(`skills.errors.${state.code}`)}
                    </p>
                    {state.uncertain && (
                        <p className={styles["hint"]}>
                            {t("skills.reloadUnknown")}
                        </p>
                    )}
                </>
            )}
            {state.kind === "applying" && (
                <p className={styles["hint"]}>{t("skills.update.applying")}</p>
            )}
            {state.kind === "applied" && (
                <>
                    <DetailFact
                        label={t("skills.update.appliedStrategy")}
                        value={t(
                            `skills.update.strategies.${state.result.strategy}`,
                        )}
                    />
                    <DetailFact
                        label={t("skills.update.status")}
                        value={t(
                            `skills.update.statuses.${state.result.status}`,
                        )}
                    />
                    {state.result.deployments.length === 0 ? (
                        <p className={styles["hint"]}>
                            {t("skills.update.noDeployments")}
                        </p>
                    ) : (
                        <DetailFact
                            label={t("skills.update.deployments")}
                            value={
                                <ul className={styles["longValue"]}>
                                    {state.result.deployments.map((row) => (
                                        <li
                                            key={`${row.agentId}:${row.configRoot}`}
                                        >
                                            {t(
                                                row.agentId === "pi"
                                                    ? "skills.deploy.agentPi"
                                                    : "skills.deploy.agentGrokBuild",
                                            )}
                                            {" · "}
                                            {t(
                                                `skills.update.deployOutcomes.${row.outcome}`,
                                            )}
                                        </li>
                                    ))}
                                </ul>
                            }
                        />
                    )}
                </>
            )}
            {state.kind === "ready" && (
                <>
                    <DetailFact
                        label={t("skills.update.status")}
                        value={t(
                            `skills.update.statuses.${state.preview.status}`,
                        )}
                    />
                    <DetailFact
                        label={t("skills.update.method")}
                        value={
                            state.preview.fetchMethod === "git"
                                ? t("skills.sourceGit")
                                : t("skills.sourceNpx", {
                                      version:
                                          record.installerVersion ?? "1.7.4",
                                  })
                        }
                    />
                    <DetailFact
                        label={t("skills.update.command")}
                        value={
                            <p className={styles["longValue"]}>
                                {state.preview.command}
                            </p>
                        }
                    />
                    {state.preview.recordedSourceRevision !== null && (
                        <DetailFact
                            label={t("skills.update.recordedRevision")}
                            value={state.preview.recordedSourceRevision}
                        />
                    )}
                    {state.preview.candidateSourceRevision !== null && (
                        <DetailFact
                            label={t("skills.update.candidateRevision")}
                            value={state.preview.candidateSourceRevision}
                        />
                    )}
                    <DetailFact
                        label={t("skills.update.recordedDigest")}
                        value={shortDigest(state.preview.recordedContentDigest)}
                    />
                    <DetailFact
                        label={t("skills.update.liveDigest")}
                        value={shortDigest(state.preview.liveContentDigest)}
                    />
                    <DetailFact
                        label={t("skills.update.candidateDigest")}
                        value={shortDigest(
                            state.preview.candidateContentDigest,
                        )}
                    />
                    <DetailFact
                        label={t("skills.update.candidateSize")}
                        value={t("skills.update.candidateSizeValue", {
                            files: state.preview.candidateFileCount,
                            bytes: state.preview.candidateByteSize,
                        })}
                    />
                    {state.preview.diff.descriptionChanged && (
                        <DetailFact
                            label={t("skills.update.description")}
                            value={
                                <p className={styles["longValue"]}>
                                    {t("skills.update.descriptionChange", {
                                        from: state.preview.diff
                                            .liveDescription,
                                        to: state.preview.diff
                                            .candidateDescription,
                                    })}
                                </p>
                            }
                        />
                    )}
                    <DetailFact
                        label={t("skills.update.addedFiles")}
                        value={
                            state.preview.diff.addedFiles.length === 0
                                ? t("skills.update.noAddedFiles")
                                : state.preview.diff.addedFiles.join(", ")
                        }
                    />
                    <DetailFact
                        label={t("skills.update.removedFiles")}
                        value={
                            state.preview.diff.removedFiles.length === 0
                                ? t("skills.update.noRemovedFiles")
                                : state.preview.diff.removedFiles.join(", ")
                        }
                    />
                    <DetailFact
                        label={t("skills.update.changedFiles")}
                        value={
                            state.preview.diff.changedFiles.length === 0
                                ? t("skills.update.noChangedFiles")
                                : state.preview.diff.changedFiles.join(", ")
                        }
                    />
                    {state.preview.strategies.length === 0 ? (
                        <p className={styles["hint"]}>
                            {t("skills.update.noStrategies")}
                        </p>
                    ) : (
                        <div className={styles["deployActions"]}>
                            {state.preview.strategies.includes("keep") && (
                                <button
                                    className={buttons["secondary"]}
                                    type="button"
                                    aria-disabled={busy}
                                    onClick={() => {
                                        void apply("keep");
                                    }}
                                >
                                    {t("skills.update.applyKeep")}
                                </button>
                            )}
                            {state.preview.strategies.includes("replace") && (
                                <button
                                    className={buttons["primary"]}
                                    type="button"
                                    aria-disabled={busy}
                                    onClick={() => {
                                        void apply("replace");
                                    }}
                                >
                                    {t("skills.update.applyReplace")}
                                </button>
                            )}
                        </div>
                    )}
                </>
            )}
        </DetailPanel>
    );
}
