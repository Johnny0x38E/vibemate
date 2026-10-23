import { Dialog } from "@base-ui/react/dialog";
import type { JSX } from "react";
import { useTranslation } from "react-i18next";
import { DetailFact, DetailPanel } from "../../components/DetailForm";
import { VersionSummary } from "../agents/VersionRestoreCard";
import type { PiMcpPlan } from "../../lib/desktop/piMcp";
import buttons from "../../components/actionButtons.module.css";
import dialog from "./McpDeleteDialog.module.css";
import styles from "./McpPiPreview.module.css";

/** Review one entry only. Values have already passed the masked IPC boundary;
 * the separate details dialog never contains unrelated native configuration. */
export function PiMcpReview({ plan }: { plan: PiMcpPlan }): JSX.Element {
    const { t } = useTranslation();
    return (
        <div className={styles["section"]}>
            <DetailPanel label={t("mcp.pi.manage.review")}>
                <DetailFact
                    label={t("mcp.piPreview.target")}
                    value={plan.target.path}
                />
                <DetailFact
                    label={t("mcp.pi.manage.action")}
                    value={t(`mcp.pi.actions.${plan.action}`)}
                />
                <DetailFact
                    label={t("mcp.serverName")}
                    value={plan.serverName}
                />
                <DetailFact
                    label={t("mcp.piPreview.enabled")}
                    value={t(
                        plan.afterEntry === null
                            ? "mcp.pi.manage.removed"
                            : plan.afterEntry.enabled
                              ? "mcp.piPreview.on"
                              : "mcp.piPreview.off",
                    )}
                />
            </DetailPanel>
            <VersionSummary file={{ summary: plan.summary }} />
            <p className={styles["notice"]}>{t("mcp.pi.manage.preserve")}</p>
            {plan.action === "deploy" && (
                <p className={styles["notice"]}>
                    {t("mcp.pi.manage.credentials")}
                </p>
            )}
            {plan.action === "restore" && (
                <p className={styles["notice"]}>
                    {t("mcp.pi.manage.historicalCredentials")}
                </p>
            )}
            {plan.sessionRelativeCwd && (
                <p className={styles["notice"]}>
                    {t("mcp.piPreview.relativeCwd")}
                </p>
            )}
            {plan.expandsHome && (
                <p className={styles["notice"]}>{t("mcp.piPreview.home")}</p>
            )}
            <Dialog.Root>
                <Dialog.Trigger className={buttons["secondary"]}>
                    {t("mcp.piPreview.details")}
                </Dialog.Trigger>
                <Dialog.Portal>
                    <Dialog.Backdrop className={dialog["backdrop"]} />
                    <Dialog.Popup
                        className={[dialog["popup"], styles["popup"]].join(" ")}
                    >
                        <Dialog.Title className={dialog["title"]}>
                            {t("mcp.piPreview.detailsTitle")}
                        </Dialog.Title>
                        <Dialog.Description className={dialog["body"]}>
                            {t("mcp.pi.manage.entryOnly")}
                        </Dialog.Description>
                        <pre className={styles["code"]}>
                            {JSON.stringify(
                                {
                                    before: plan.beforeEntry,
                                    after: plan.afterEntry,
                                },
                                null,
                                2,
                            )}
                        </pre>
                        <div className={dialog["actions"]}>
                            <Dialog.Close className={buttons["secondary"]}>
                                {t("mcp.piPreview.close")}
                            </Dialog.Close>
                        </div>
                    </Dialog.Popup>
                </Dialog.Portal>
            </Dialog.Root>
        </div>
    );
}
