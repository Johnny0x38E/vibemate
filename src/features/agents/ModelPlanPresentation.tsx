import { useEffect, useState, type JSX } from "react";
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
 * After summaries load, the generate-preview result renders **inline** below the
 * provider/model form (summary / file state on top, field diffs below). Full
 * config text is never shown here — only via the page-level 「详细配置」 entry.
 * Apply stays blocked until summaries load. Nested preview dialogs are not used
 * so the Write dialog can host this card without stacking modals.
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
            {state.kind === "ready" && (
                <>
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
                </>
            )}
            <div className={card["actions"]}>
                <button
                    type="button"
                    className={buttons["primary"]}
                    aria-disabled={state.kind !== "ready" || applying}
                    onClick={() => {
                        if (state.kind === "ready" && !applying) onApply();
                    }}
                >
                    {applying
                        ? t("agents.modelConfig.applying")
                        : t("agents.modelConfig.apply")}
                </button>
            </div>
        </>
    );
}
