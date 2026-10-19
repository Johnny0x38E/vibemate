import type { JSX } from "react";
import { useTranslation } from "react-i18next";
import { DetailPanel } from "../../components/DetailForm";
import buttons from "../../components/actionButtons.module.css";
import type { ResolutionReview } from "../../lib/desktop/recovery";
import styles from "./AgentList.module.css";
import card from "./ConfigPreview.module.css";

/**
 * Metadata-only reconciliation review. Confirmation records observed state,
 * never restores configuration. Rust decides whether the whole set is eligible.
 */
export function ResolutionReviewCard({
    review,
    busy,
    onConfirm,
    onCancel,
}: {
    review: ResolutionReview;
    busy: boolean;
    onConfirm: () => void;
    onCancel: () => void;
}): JSX.Element {
    const { t } = useTranslation();
    return (
        <DetailPanel label={t("agents.resolution.title")}>
            <p className={styles["hint"]}>{t("agents.resolution.notice")}</p>
            {review.revisions.length === 0 && (
                <p className={styles["hint"]}>{t("agents.resolution.empty")}</p>
            )}
            <ul className={card["changes"]}>
                {review.revisions.map((revision) => (
                    <li key={revision.id} className={card["change"]}>
                        <span className={card["path"]}>
                            {revision.targetPath}
                        </span>
                        <span className={card["note"]}>
                            {t("agents.resolution.direction", {
                                intent: t(
                                    `agents.resolution.intents.${revision.intent}`,
                                ),
                                state: t(
                                    `agents.resolution.stages.${revision.state}`,
                                ),
                            })}
                        </span>
                        <span className={card["note"]}>
                            {t(
                                `agents.resolution.matches.${revision.currentMatch}`,
                            )}
                        </span>
                        <span className={card["note"]}>
                            {t(
                                `agents.resolution.backups.${revision.backupState}`,
                            )}
                        </span>
                        {revision.errorCode !== null && (
                            <span className={card["note"]}>
                                {t(
                                    `agents.restore.reasons.${revision.errorCode}`,
                                )}
                            </span>
                        )}
                        <span className={card["path"]}>
                            {t("agents.resolution.observedDigest", {
                                digest:
                                    revision.currentMatch === "unreadable"
                                        ? t("agents.resolution.unreadable")
                                        : (revision.currentDigest ??
                                          t("agents.resolution.absent")),
                            })}
                        </span>
                    </li>
                ))}
            </ul>
            {!review.resolvable && review.revisions.length > 0 && (
                <p className={styles["hint"]}>
                    {t("agents.resolution.blocked")}
                </p>
            )}
            <div className={card["actions"]}>
                {review.resolvable && (
                    <button
                        type="button"
                        className={buttons["primary"]}
                        aria-disabled={busy}
                        onClick={onConfirm}
                    >
                        {busy
                            ? t("agents.resolution.recording")
                            : t("agents.resolution.confirm")}
                    </button>
                )}
                <button
                    type="button"
                    className={buttons["secondary"]}
                    aria-disabled={busy}
                    onClick={onCancel}
                >
                    {t("agents.resolution.cancel")}
                </button>
            </div>
        </DetailPanel>
    );
}
