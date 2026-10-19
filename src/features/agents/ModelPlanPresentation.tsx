import { Dialog } from "@base-ui/react/dialog";
import { useEffect, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import buttons from "../../components/actionButtons.module.css";
import {
    readModelConfigurationPreview,
    type ModelConfigurationPlan,
    type ModelPreviewFile,
} from "../../lib/desktop/configurationPreview";
import { VersionRestoreRequestError } from "../../lib/desktop/versionRestore";
import { VersionSummary } from "./VersionRestoreCard";
import card from "./ConfigPreview.module.css";
import dialog from "./VersionTextDialog.module.css";

type State =
    | { kind: "loading" }
    | { kind: "preview" }
    | { kind: "error"; code: VersionRestoreRequestError["code"] }
    | { kind: "ready"; files: ModelPreviewFile[] };

/**
 * Plan-bound field summaries for model application.
 *
 * After summaries load, the generate-preview result opens in a contained
 * dialog (below the titlebar): top = summary / file state, bottom = field
 * diffs. Full/actual config text is never shown here — only via the page-level
 * 「详细配置」 entry. Apply stays blocked until summaries load.
 */
export function ModelPlanPresentation({
    plan,
    entryPath,
    applying,
    onApply,
}: {
    plan: ModelConfigurationPlan;
    entryPath: string | null;
    applying: boolean;
    onApply: () => void;
}): JSX.Element {
    const { t } = useTranslation();
    const [state, setState] = useState<State>({ kind: "loading" });
    const [attempt, setAttempt] = useState(0);
    const [dialogOpen, setDialogOpen] = useState(false);
    const reopenButton = useRef<HTMLButtonElement | null>(null);
    const title = useRef<HTMLHeadingElement>(null);
    useEffect(() => {
        // Ignore reads from a discarded plan or an unmounted configuration page.
        let active = true;
        readModelConfigurationPreview(plan, entryPath).then(
            (result) => {
                if (!active) return;
                if (result.kind === "preview") {
                    setState({ kind: "preview" });
                    return;
                }
                setState({ kind: "ready", files: result.files });
                setDialogOpen(true);
            },
            (error: unknown) => {
                if (active)
                    setState({
                        kind: "error",
                        code:
                            error instanceof VersionRestoreRequestError
                                ? error.code
                                : "operation_failed",
                    });
            },
        );
        return () => {
            active = false;
        };
    }, [plan, entryPath, attempt]);
    const targets =
        plan.agent === "pi"
            ? plan.files.map((file) => file.target)
            : plan.agent === "grokBuild"
              ? [plan.file.target]
              : [plan.target];

    return (
        <>
            {state.kind === "loading" && (
                <p role="status">{t("agents.version.textLoading")}</p>
            )}
            {state.kind === "preview" && (
                <p>{t("agents.version.textPreviewMode")}</p>
            )}
            {state.kind === "error" && (
                <p role="alert">
                    {t(`agents.restore.errors.${state.code}`)}{" "}
                    <button
                        type="button"
                        className={buttons["secondary"]}
                        onClick={() => {
                            setState({ kind: "loading" });
                            setAttempt((value) => value + 1);
                        }}
                    >
                        {t("agents.version.textRetry")}
                    </button>
                </p>
            )}
            {state.kind === "ready" && !dialogOpen && (
                <div className={card["actions"]}>
                    <button
                        ref={reopenButton}
                        type="button"
                        className={buttons["secondary"]}
                        disabled={applying}
                        onClick={() => {
                            setDialogOpen(true);
                        }}
                    >
                        {t("agents.version.viewDetailsFor", {
                            path: state.files[0]?.targetPath ?? "",
                        })}
                    </button>
                    <button
                        type="button"
                        className={buttons["primary"]}
                        aria-disabled={applying}
                        onClick={() => {
                            if (!applying) onApply();
                        }}
                    >
                        {applying
                            ? t("agents.modelConfig.applying")
                            : t("agents.modelConfig.apply")}
                    </button>
                </div>
            )}
            {state.kind === "ready" && dialogOpen && (
                <Dialog.Root
                    open
                    onOpenChange={(open) => {
                        if (!open && !applying) setDialogOpen(false);
                    }}
                >
                    <Dialog.Portal>
                        <Dialog.Backdrop className={dialog["backdrop"]} />
                        <Dialog.Popup
                            className={dialog["popup"]}
                            initialFocus={title}
                            finalFocus={() =>
                                reopenButton.current?.isConnected
                                    ? reopenButton.current
                                    : null
                            }
                        >
                            <Dialog.Title
                                ref={title}
                                tabIndex={-1}
                                className={dialog["title"]}
                            >
                                {t("agents.version.viewDetailsFor", {
                                    path:
                                        state.files.length === 1
                                            ? (state.files[0]?.targetPath ?? "")
                                            : t(
                                                  "agents.modelConfig.planHeading",
                                              ),
                                })}
                            </Dialog.Title>
                            <p className={dialog["meta"]}>
                                {t("agents.version.textMaskedNotice")}
                            </p>
                            {/* Top: generate-preview summary / file state */}
                            <section
                                className={dialog["diff"]}
                                aria-label={t("agents.modelConfig.planHeading")}
                            >
                                {state.files.map((file, index) => (
                                    <div
                                        key={file.targetPath}
                                        className={card["summary"]}
                                    >
                                        <span className={card["path"]}>
                                            {file.targetPath}
                                        </span>
                                        <span className={card["fileState"]}>
                                            {t(
                                                targets[index]?.exists
                                                    ? targets[index].symlink
                                                        ? "agents.modelConfig.fileStateSymlink"
                                                        : "agents.modelConfig.fileStateExists"
                                                    : "agents.modelConfig.fileStateNew",
                                            )}
                                        </span>
                                    </div>
                                ))}
                            </section>
                            {/* Bottom: field-level diffs */}
                            <section
                                className={dialog["diff"]}
                                aria-label={t("agents.version.textDiff")}
                            >
                                <h3 className={dialog["sideHeading"]}>
                                    {t("agents.version.textDiff")}
                                </h3>
                                {state.files.map((file) => (
                                    <div key={`diff:${file.targetPath}`}>
                                        <VersionSummary file={file} />
                                    </div>
                                ))}
                            </section>
                            <div className={dialog["actions"]}>
                                <button
                                    type="button"
                                    className={buttons["secondary"]}
                                    aria-disabled={applying}
                                    onClick={() => {
                                        if (!applying) setDialogOpen(false);
                                    }}
                                >
                                    {t("agents.version.textClose")}
                                </button>
                                <button
                                    type="button"
                                    className={buttons["primary"]}
                                    aria-disabled={applying}
                                    onClick={() => {
                                        if (!applying) onApply();
                                    }}
                                >
                                    {applying
                                        ? t("agents.modelConfig.applying")
                                        : t("agents.modelConfig.apply")}
                                </button>
                            </div>
                        </Dialog.Popup>
                    </Dialog.Portal>
                </Dialog.Root>
            )}
            {state.kind === "loading" && (
                <div className={card["actions"]}>
                    <button
                        type="button"
                        className={buttons["primary"]}
                        aria-disabled
                    >
                        {t("agents.modelConfig.apply")}
                    </button>
                </div>
            )}
            {state.kind !== "ready" && state.kind !== "loading" && (
                <div className={card["actions"]}>
                    <button
                        type="button"
                        className={buttons["primary"]}
                        aria-disabled
                    >
                        {t("agents.modelConfig.apply")}
                    </button>
                </div>
            )}
        </>
    );
}
