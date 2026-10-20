import type { TFunction } from "i18next";
import { useState, type JSX } from "react";
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
import dialog from "./VersionTextDialog.module.css";

type Translator = TFunction;

/**
 * Sanitized current-to-retained preview for one history entry.
 *
 * Rendered inside the history detail dialog, one selected file at a time.
 * Field-level diffs are the only comparison layer here; current-config text lives
 * solely on the page-level 「详细配置」 entry. Confirmation is offered only when
 * Rust marked the whole entry restorable.
 */
export function VersionRestorePlanContent({
    plan,
    restoring,
    onConfirm,
    onDismiss,
}: {
    plan: VersionRestorePlan;
    /** A restore is in flight, so the confirmation waits. */
    restoring: boolean;
    onConfirm: () => void;
    /** Close the preview without restoring. */
    onDismiss: () => void;
}): JSX.Element {
    const { t } = useTranslation();
    const [selectedOrdinal, setSelectedOrdinal] = useState<number | null>(null);
    const selected =
        plan.files.find((file) => file.ordinal === selectedOrdinal) ??
        plan.files[0];
    const blocked = plan.files.some((file) => file.action === "blocked");
    // Rust compares the written version with the live files. Only its plan
    // can decide whether selecting this version would actually change them.
    const canConfirm = plan.restorable;
    return (
        <div
            className={restore["versionPreview"]}
            role="region"
            aria-label={t("agents.version.previewHeading")}
        >
            {plan.reason !== null && (
                <p className={restore["versionHint"]}>
                    {t(`agents.version.planReasons.${plan.reason}`)}
                </p>
            )}
            {plan.files.length > 1 && (
                <div
                    className={dialog["fileTabs"]}
                    role="group"
                    aria-label={t("agents.version.textFiles")}
                >
                    {plan.files.map((file) => (
                        <button
                            key={file.ordinal}
                            type="button"
                            className={dialog["fileTab"]}
                            aria-pressed={file.ordinal === selected?.ordinal}
                            aria-label={file.targetPath}
                            onClick={() => {
                                setSelectedOrdinal(file.ordinal);
                            }}
                        >
                            {file.targetPath.split(/[/\\]/u).pop() ??
                                file.targetPath}
                        </button>
                    ))}
                </div>
            )}
            {selected !== undefined && (
                <ul className={restore["versionFiles"]}>
                    <VersionFilePreview file={selected} />
                </ul>
            )}
            {canConfirm ? (
                <div className={restore["versionHint"]}>
                    {plan.files.length > 1 && (
                        <p className={styles["hint"]}>
                            {t("agents.version.restoreAllFiles")}
                        </p>
                    )}
                    <p className={styles["hint"]}>
                        {t("agents.version.switchNotice")}
                    </p>
                </div>
            ) : (
                plan.reason === null &&
                !blocked && (
                    <p className={restore["versionHint"]}>
                        {t("agents.version.nothingToRestore")}
                    </p>
                )
            )}
            <div className={card["actions"]}>
                <button
                    type="button"
                    className={buttons["secondary"]}
                    aria-disabled={restoring}
                    onClick={() => {
                        if (!restoring) onDismiss();
                    }}
                >
                    {t("agents.version.textClose")}
                </button>
                {canConfirm && (
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
                )}
            </div>
        </div>
    );
}

/** One file: action and sanitized field summary (no detailed-config entry). */
function VersionFilePreview({
    file,
}: {
    file: VersionRestoreFile;
}): JSX.Element {
    const { t } = useTranslation();
    return (
        <li className={restore["versionFile"]}>
            <span className={restore["versionFilePath"]}>
                {file.targetPath}
            </span>
            <div className={restore["versionFileBody"]}>
                {file.action === "remove" && (
                    <span className={restore["action"]}>
                        {t("agents.version.actions.remove")}
                    </span>
                )}
                {file.reason !== null && (
                    <p className={styles["hint"]}>
                        {t(reasonKey(file.reason))}
                    </p>
                )}
                {file.action !== "blocked" && <VersionSummary file={file} />}
            </div>
        </li>
    );
}

/** The default field-level layer of the two-layer preview.
 * Reuses the ConfigPreview field-summary row: path and old → new values.
 */
export function VersionSummary({
    file,
}: {
    file: Pick<VersionRestoreFile, "summary">;
}): JSX.Element {
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
            <ul className={card["changes"]}>
                {summary.changes.map((change, index) => (
                    <li
                        key={`${change.path}:${String(index)}`}
                        className={card["change"]}
                    >
                        <span className={card["key"]}>{change.path}</span>
                        <span className={card["transition"]}>
                            <span className={card["before"]}>
                                {displayValue(change.before, t)}
                            </span>
                            {/* The values describe the change; the arrow is visual only. */}
                            <span className={card["arrow"]} aria-hidden="true">
                                →
                            </span>
                            <span className={card["after"]}>
                                {displayValue(change.after, t)}
                            </span>
                        </span>
                        {change.secret && (
                            <span className={styles["hint"]}>
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
