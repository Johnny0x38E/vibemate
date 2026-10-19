import { Dialog } from "@base-ui/react/dialog";
import { useCallback, useEffect, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import buttons from "../../components/actionButtons.module.css";
import type { ConfigurableAgent } from "../../lib/desktop/agents";
import {
    VersionRestoreRequestError,
    readVersionText,
    type VersionRestoreFile,
    type VersionRestorePlan,
    type VersionText,
    type VersionTextSide,
} from "../../lib/desktop/versionRestore";
import styles from "./AgentList.module.css";
import { ConfigText } from "./ConfigText";
import { lineDiff } from "./lineDiff";
import dialog from "./VersionTextDialog.module.css";

type TextState =
    | { kind: "loading" }
    | { kind: "preview" }
    | { kind: "error"; code: VersionRestoreRequestError["code"] }
    | { kind: "ready"; text: VersionText };

type RevisionFile = VersionRestoreFile & { revisionId: string };

/**
 * Detail dialog for a reviewed plan: line-level differences first, with the
 * complete sanitized current and retained/planned text available as an optional
 * expand. Files switch on demand.
 *
 * The parent only mounts this while open. Text is fetched per selected file on
 * demand, bound to the current digest the summary preview showed; a changed
 * file returns `plan_stale` and the retry path, never a mixed comparison. The
 * dialog manages focus and Escape through the shared Base UI primitive.
 */
export function VersionTextDialog({
    agent,
    plan,
    initialRevisionId,
    onClose,
    returnFocus,
}: {
    agent: ConfigurableAgent;
    plan: VersionRestorePlan;
    initialRevisionId: string;
    onClose: () => void;
    returnFocus: () => HTMLElement | null;
}): JSX.Element {
    const files = plan.files.filter(
        (file): file is RevisionFile => file.revisionId !== null,
    );
    const loadText = useCallback(
        (id: string) => {
            const file = plan.files.find(
                (candidate) => candidate.revisionId === id,
            );
            if (file === undefined || file.revisionId === null) {
                return Promise.reject(
                    new VersionRestoreRequestError("invalid_request"),
                );
            }
            return readVersionText(
                agent,
                plan.entryKey,
                file.revisionId,
                file.currentDigest,
            );
        },
        [agent, plan],
    );
    return (
        <ConfigurationTextDialog
            files={files}
            loadText={loadText}
            initialRevisionId={initialRevisionId}
            onClose={onClose}
            returnFocus={returnFocus}
        />
    );
}

/** Shared on-demand JSON/TOML dialog for native model and restore plans.
 * The caller supplies a plan-bound reader; this component owns loading, file
 * selection, accessible focus, the primary line diff, and optional full text.
 */
export function ConfigurationTextDialog({
    files,
    loadText,
    initialRevisionId,
    onClose,
    returnFocus,
    planned = false,
}: {
    files: { revisionId: string; targetPath: string }[];
    loadText: (
        id: string,
    ) => Promise<{ kind: "preview" } | { kind: "desktop"; text: VersionText }>;
    initialRevisionId: string;
    onClose: () => void;
    returnFocus: () => HTMLElement | null;
    planned?: boolean;
}): JSX.Element {
    const { t } = useTranslation();
    const [selectedId, setSelectedId] = useState(initialRevisionId);
    const [reload, setReload] = useState(0);
    const [state, setState] = useState<TextState>({ kind: "loading" });
    const generation = useRef(0);
    // Focus the title, not the bottom action: focusing Close would scroll a
    // long dialog straight past its file switcher and configuration headings.
    const title = useRef<HTMLHeadingElement>(null);

    useEffect(() => {
        // A late reply must not replace the text for a different file or a
        // dialog that has already closed.
        const current = ++generation.current;
        loadText(selectedId).then(
            (result) => {
                if (current !== generation.current) return;
                setState(
                    result.kind === "preview"
                        ? { kind: "preview" }
                        : { kind: "ready", text: result.text },
                );
            },
            (error: unknown) => {
                if (current !== generation.current) return;
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
            generation.current += 1;
        };
    }, [loadText, selectedId, reload]);

    const selected =
        files.find((file) => file.revisionId === selectedId) ?? files[0];
    const text = state.kind === "ready" ? state.text : null;
    // The dialog renders only on load and file switches; the diff is bounded by
    // `lineDiff`, so computing it here avoids a memo that React would discard.
    const comparable =
        text !== null &&
        text.current.state === "present" &&
        text.target.state === "present";
    const diff =
        text !== null &&
        text.current.state === "present" &&
        text.target.state === "present"
            ? lineDiff(text.current.text, text.target.text)
            : null;

    return (
        <Dialog.Root
            open
            onOpenChange={(open) => {
                if (!open) onClose();
            }}
        >
            <Dialog.Portal>
                <Dialog.Backdrop className={dialog["backdrop"]} />
                <Dialog.Popup
                    className={dialog["popup"]}
                    initialFocus={title}
                    finalFocus={returnFocus}
                >
                    <Dialog.Title
                        ref={title}
                        tabIndex={-1}
                        className={dialog["title"]}
                    >
                        {t("agents.version.textTitle")}
                    </Dialog.Title>
                    <Dialog.Description className={dialog["meta"]}>
                        {selected?.targetPath ?? ""}
                    </Dialog.Description>
                    {files.length > 1 && (
                        <div
                            className={dialog["fileTabs"]}
                            role="group"
                            aria-label={t("agents.version.textFiles")}
                        >
                            {files.map((file) => (
                                <button
                                    key={file.revisionId}
                                    type="button"
                                    className={dialog["fileTab"]}
                                    aria-pressed={
                                        file.revisionId === selectedId
                                    }
                                    aria-label={file.targetPath}
                                    onClick={() => {
                                        setState({ kind: "loading" });
                                        setSelectedId(file.revisionId);
                                    }}
                                >
                                    {baseName(file.targetPath)}
                                </button>
                            ))}
                        </div>
                    )}
                    {selected === undefined && (
                        <p className={styles["error"]} role="alert">
                            {t("agents.restore.errors.invalid_request")}
                        </p>
                    )}
                    {selected !== undefined && state.kind === "loading" && (
                        <p className={styles["hint"]} role="status">
                            {t("agents.version.textLoading")}
                        </p>
                    )}
                    {state.kind === "preview" && (
                        <p className={styles["hint"]}>
                            {t("agents.version.textPreviewMode")}
                        </p>
                    )}
                    {state.kind === "error" && (
                        <>
                            <p className={styles["error"]} role="alert">
                                {t(`agents.restore.errors.${state.code}`)}
                            </p>
                            <div className={dialog["actions"]}>
                                <button
                                    type="button"
                                    className={buttons["secondary"]}
                                    onClick={() => {
                                        setState({ kind: "loading" });
                                        setReload((value) => value + 1);
                                    }}
                                >
                                    {t("agents.version.textRetry")}
                                </button>
                            </div>
                        </>
                    )}
                    {state.kind === "ready" && (
                        <>
                            <p className={dialog["meta"]}>
                                {t("agents.version.textMaskedNotice")}
                            </p>
                            <section
                                className={dialog["diff"]}
                                aria-label={t("agents.version.textDiff")}
                            >
                                <h3 className={dialog["sideHeading"]}>
                                    {t("agents.version.textDiff")}
                                </h3>
                                {!comparable ? (
                                    <p className={styles["hint"]}>
                                        {t("agents.version.textNoComparison")}
                                    </p>
                                ) : diff === null ? (
                                    <p className={styles["hint"]}>
                                        {t("agents.version.textDiffTooLarge")}
                                    </p>
                                ) : diff.length === 0 ? (
                                    <p className={styles["hint"]}>
                                        {t("agents.version.textNoDiff")}
                                    </p>
                                ) : (
                                    <ul className={dialog["diffRows"]}>
                                        {diff.map((line, index) => (
                                            <li
                                                key={index}
                                                className={[
                                                    dialog["diffRow"],
                                                    line.kind === "added"
                                                        ? dialog["diffAdded"]
                                                        : line.kind ===
                                                            "removed"
                                                          ? dialog[
                                                                "diffRemoved"
                                                            ]
                                                          : null,
                                                ]
                                                    .filter(
                                                        (
                                                            value,
                                                        ): value is string =>
                                                            typeof value ===
                                                            "string",
                                                    )
                                                    .join(" ")}
                                                aria-label={`${t(
                                                    line.kind === "added"
                                                        ? "agents.version.lineAdded"
                                                        : line.kind ===
                                                            "removed"
                                                          ? "agents.version.lineRemoved"
                                                          : "agents.version.lineSame",
                                                )}: ${line.text}`}
                                            >
                                                <span
                                                    className={
                                                        dialog["lineNumber"]
                                                    }
                                                >
                                                    {line.oldLine ?? ""}
                                                </span>
                                                <span
                                                    className={
                                                        dialog["lineNumber"]
                                                    }
                                                >
                                                    {line.newLine ?? ""}
                                                </span>
                                                <span
                                                    className={dialog["sign"]}
                                                >
                                                    {line.kind === "added"
                                                        ? "+"
                                                        : line.kind ===
                                                            "removed"
                                                          ? "-"
                                                          : " "}
                                                </span>
                                                <span
                                                    className={
                                                        dialog["diffText"]
                                                    }
                                                >
                                                    {line.text}
                                                </span>
                                            </li>
                                        ))}
                                    </ul>
                                )}
                            </section>
                            <details className={dialog["fullText"]}>
                                <summary className={dialog["fullTextSummary"]}>
                                    {t("agents.version.textShowFull")}
                                </summary>
                                <div className={dialog["sides"]}>
                                    <TextSide
                                        side={state.text.current}
                                        format={state.text.format}
                                        label={t("agents.version.textCurrent")}
                                    />
                                    <TextSide
                                        side={state.text.target}
                                        format={state.text.format}
                                        label={t(
                                            planned
                                                ? "agents.version.textPlanned"
                                                : "agents.version.textRetained",
                                        )}
                                    />
                                </div>
                            </details>
                        </>
                    )}
                    <div className={dialog["actions"]}>
                        <button
                            type="button"
                            className={buttons["secondary"]}
                            onClick={onClose}
                        >
                            {t("agents.version.textClose")}
                        </button>
                    </div>
                </Dialog.Popup>
            </Dialog.Portal>
        </Dialog.Root>
    );
}

/** One read-only side: present text, absence or a safe unavailability reason. */
function TextSide({
    side,
    format,
    label,
}: {
    side: VersionTextSide;
    format: "json" | "toml";
    label: string;
}): JSX.Element {
    const { t } = useTranslation();
    return (
        <section className={dialog["side"]} aria-label={label}>
            <h3 className={dialog["sideHeading"]}>{label}</h3>
            {side.state === "absent" && (
                <p className={styles["hint"]}>
                    {t("agents.version.textAbsent")}
                </p>
            )}
            {side.state === "unavailable" && (
                <p className={styles["error"]} role="alert">
                    {t("agents.version.textUnavailable", {
                        reason: t(`agents.version.reasons.${side.reason}`),
                    })}
                </p>
            )}
            {side.state === "present" && (
                <>
                    <p className={dialog["meta"]}>
                        {t("agents.version.textBytes", {
                            bytes: side.bytes,
                        })}
                    </p>
                    <ConfigText text={side.text} format={format} />
                </>
            )}
        </section>
    );
}

/** Last path segment for the compact file switcher. */
function baseName(path: string): string {
    const parts = path.split(/[\\/]/u);
    return parts[parts.length - 1] ?? path;
}
