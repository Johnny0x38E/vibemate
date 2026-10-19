import { useCallback, useEffect, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import buttons from "../../components/actionButtons.module.css";
import {
    readModelConfigurationPreview,
    type ModelConfigurationPlan,
    type ModelPreviewFile,
} from "../../lib/desktop/configurationPreview";
import { VersionRestoreRequestError } from "../../lib/desktop/versionRestore";
import { ConfigurationTextDialog } from "./VersionTextDialog";
import { VersionSummary } from "./VersionRestoreCard";
import styles from "./AgentList.module.css";
import card from "./ConfigPreview.module.css";

type State =
    | { kind: "loading" }
    | { kind: "preview" }
    | { kind: "error"; code: VersionRestoreRequestError["code"] }
    | { kind: "ready"; files: ModelPreviewFile[] };

/** Plan-bound field summaries and on-demand native text for model application.
 * Mount with a new key for every new preview: stale summary/detail reads cannot
 * authorize a replacement plan, and apply remains blocked until summaries load.
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
    const [selected, setSelected] = useState<number | null>(null);
    const detailButton = useRef<HTMLButtonElement | null>(null);
    useEffect(() => {
        // Ignore reads from a discarded plan or an unmounted configuration page.
        let active = true;
        readModelConfigurationPreview(plan, entryPath).then(
            (result) => {
                if (active)
                    setState(
                        result.kind === "preview"
                            ? { kind: "preview" }
                            : { kind: "ready", files: result.files },
                    );
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
    const loadText = useCallback(
        async (id: string) => {
            const index = Number(id.replace(/^file-/u, ""));
            const result = await readModelConfigurationPreview(
                plan,
                entryPath,
                index,
            );
            if (result.kind === "preview") return result;
            const text = result.files[index]?.text;
            if (text === undefined || text === null)
                throw new VersionRestoreRequestError("invalid_response");
            return { kind: "desktop" as const, text };
        },
        [plan, entryPath],
    );
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
            {state.kind === "ready" &&
                state.files.map((file, index) => (
                    <div key={file.targetPath}>
                        <div className={card["summary"]}>
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
                        <VersionSummary file={file} />
                        <button
                            type="button"
                            className={buttons["secondary"]}
                            disabled={applying}
                            onClick={(event) => {
                                detailButton.current = event.currentTarget;
                                setSelected(index);
                            }}
                        >
                            {t("agents.version.viewDetailsFor", {
                                path: file.targetPath,
                            })}
                        </button>
                    </div>
                ))}
            <p className={styles["hint"]}>
                {t("agents.version.textMaskedNotice")}
            </p>
            <div className={card["actions"]}>
                <button
                    type="button"
                    className={buttons["primary"]}
                    aria-disabled={
                        applying || state.kind !== "ready" || selected !== null
                    }
                    onClick={() => {
                        if (
                            !applying &&
                            state.kind === "ready" &&
                            selected === null
                        )
                            onApply();
                    }}
                >
                    {applying
                        ? t("agents.modelConfig.applying")
                        : t("agents.modelConfig.apply")}
                </button>
            </div>
            {selected !== null && state.kind === "ready" && (
                <ConfigurationTextDialog
                    planned
                    files={state.files.map((file, index) => ({
                        revisionId: `file-${String(index)}`,
                        targetPath: file.targetPath,
                    }))}
                    initialRevisionId={`file-${String(selected)}`}
                    loadText={loadText}
                    onClose={() => {
                        setSelected(null);
                    }}
                    returnFocus={() => detailButton.current}
                />
            )}
        </>
    );
}
