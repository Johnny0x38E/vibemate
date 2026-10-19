import type { TFunction } from "i18next";
import type { JSX } from "react";
import { useTranslation } from "react-i18next";
import { DetailPanel } from "../../components/DetailForm";
import buttons from "../../components/actionButtons.module.css";
import type {
    VersionRestoreFile,
    VersionRestoreOutcome,
    VersionRestorePlan,
} from "../../lib/desktop/versionRestore";
import styles from "./AgentList.module.css";
import card from "./ConfigPreview.module.css";
import restore from "./RestoreView.module.css";
import { formatTime } from "./formatTime";

type Translator = TFunction;

/**
 * Sanitized current-to-retained preview for one history entry.
 *
 * The default layer lists field changes; the complete-text dialog is a separate
 * button per file so a large document is only loaded on demand. Confirmation is
 * offered only when Rust marked the whole entry restorable.
 */
export function VersionRestorePlanCard({
    plan,
    locale,
    restoring,
    onConfirm,
    onViewDetails,
}: {
    plan: VersionRestorePlan;
    /** Language used for the timestamp; the app language decides it. */
    locale: string;
    /** A restore is in flight, so the confirmation waits. */
    restoring: boolean;
    onConfirm: () => void;
    onViewDetails: (file: VersionRestoreFile) => void;
}): JSX.Element {
    const { t } = useTranslation();
    const blocked = plan.files.some((file) => file.action === "blocked");
    return (
        <DetailPanel label={t("agents.version.previewHeading")}>
            <div className={card["summary"]}>
                <span className={card["path"]}>
                    {t("agents.version.previewFor", {
                        kind: t(`agents.restore.intents.${plan.intent}`),
                        time: formatTime(plan.createdAt, locale),
                    })}
                </span>
                {plan.ownerRef !== null && (
                    <span className={card["note"]}>
                        {t("agents.version.ownerRef", { id: plan.ownerRef })}
                    </span>
                )}
            </div>
            {plan.reason !== null && (
                <p className={styles["hint"]}>
                    {t(`agents.version.planReasons.${plan.reason}`)}
                </p>
            )}
            <ul className={card["changes"]}>
                {plan.files.map((file) => (
                    <VersionFilePreview
                        key={file.revisionId ?? `step:${String(file.ordinal)}`}
                        file={file}
                        onViewDetails={() => {
                            onViewDetails(file);
                        }}
                    />
                ))}
            </ul>
            {blocked && (
                <p className={styles["hint"]}>
                    {t("agents.version.blockedHint")}
                </p>
            )}
            {plan.restorable ? (
                <>
                    <p className={styles["hint"]}>
                        {t("agents.version.keyNotice")}
                    </p>
                    <p className={styles["hint"]}>
                        {t("agents.version.sessionNotice")}
                    </p>
                    <div className={card["actions"]}>
                        <button
                            type="button"
                            className={buttons["primary"]}
                            aria-disabled={restoring}
                            onClick={onConfirm}
                        >
                            {restoring
                                ? t("agents.version.restoring")
                                : t("agents.version.confirm")}
                        </button>
                    </div>
                </>
            ) : (
                plan.reason === null && (
                    <p className={styles["hint"]}>
                        {t("agents.version.nothingToRestore")}
                    </p>
                )
            )}
        </DetailPanel>
    );
}

/** One file: action, sanitized field summary and the detail entry. */
function VersionFilePreview({
    file,
    onViewDetails,
}: {
    file: VersionRestoreFile;
    onViewDetails: () => void;
}): JSX.Element {
    const { t } = useTranslation();
    return (
        <li className={card["change"]}>
            <span className={card["path"]}>{file.targetPath}</span>
            <span className={restore["action"]}>
                {t(`agents.version.actions.${file.action}`)}
            </span>
            {file.reason !== null && (
                <span className={card["note"]}>
                    {t(reasonKey(file.reason))}
                </span>
            )}
            <VersionSummary file={file} />
            {file.revisionId !== null && (
                <div className={card["actions"]}>
                    <button
                        type="button"
                        className={buttons["secondary"]}
                        onClick={onViewDetails}
                    >
                        {t("agents.version.viewDetailsFor", {
                            path: file.targetPath,
                        })}
                    </button>
                </div>
            )}
        </li>
    );
}

/** The default field-level layer of the two-layer preview. */
function VersionSummary({ file }: { file: VersionRestoreFile }): JSX.Element {
    const { t } = useTranslation();
    const { summary } = file;
    if (summary.reason !== null) {
        return (
            <p className={styles["hint"]}>
                {t("agents.version.summaryUnavailable", {
                    reason: t(reasonKey(summary.reason)),
                })}
            </p>
        );
    }
    if (summary.changes.length === 0) {
        return (
            <p className={styles["hint"]}>{t("agents.version.noChanges")}</p>
        );
    }
    return (
        <>
            <ul className={card["notices"]}>
                {summary.changes.map((change, index) => (
                    <li key={`${change.path}:${String(index)}`}>
                        <span className={card["path"]}>{change.path}</span>{" "}
                        <span className={restore["action"]}>
                            {t(`agents.version.kinds.${change.kind}`)}
                        </span>{" "}
                        <span className={card["note"]}>
                            {displayValue(change.before, t)}
                            {" → "}
                            {displayValue(change.after, t)}
                        </span>
                        {change.secret && (
                            <span className={card["note"]}>
                                {t("agents.version.credentialMasked")}
                            </span>
                        )}
                    </li>
                ))}
            </ul>
            {summary.truncated && (
                <p className={styles["hint"]}>
                    {t("agents.version.truncated")}
                </p>
            )}
        </>
    );
}

/** What one confirmed version restore did per file. */
export function VersionRestoreResultCard({
    result,
}: {
    result: VersionRestoreOutcome;
}): JSX.Element {
    const { t } = useTranslation();
    return (
        <DetailPanel label={t("agents.version.outcomeHeading")}>
            <ul className={card["notices"]}>
                {result.files.map((file) => (
                    <li key={file.revisionId}>
                        {t("agents.version.outcomeFile", {
                            path: file.targetPath,
                            state: t(`agents.version.states.${file.state}`),
                        })}
                        {file.errorCode !== null &&
                            ` — ${t(reasonKey(file.errorCode))}`}
                    </li>
                ))}
            </ul>
            <p className={styles["hint"]}>{t("agents.version.resultHint")}</p>
        </DetailPanel>
    );
}

/** Known file/outcome reason codes, narrowed for the typed translation keys. */
const REASON_CODES = [
    "symlink_refused",
    "target_read_only",
    "target_path_invalid",
    "target_unreadable",
    "target_too_large",
    "backup_unavailable",
    "snapshot_unavailable",
    "text_unavailable",
    "target_changed",
    "outcome_unknown",
    "write_failed",
    "invalid_request",
    "invalid_stored_operation",
] as const;

type ReasonCode = (typeof REASON_CODES)[number];

/** Map an arbitrary safe reason code onto a translated, known code. */
function reasonKey(code: string): `agents.version.reasons.${ReasonCode}` {
    const known = REASON_CODES.find((candidate) => candidate === code);
    return `agents.version.reasons.${known ?? "write_failed"}`;
}

/** Show absence and an empty string as different values. */
function displayValue(value: string | null, t: Translator): string {
    if (value === null) return t("agents.version.absent");
    if (value === "") return t("agents.version.empty");
    return value;
}
