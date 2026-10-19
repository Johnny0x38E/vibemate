import { Dialog } from "@base-ui/react/dialog";
import { useCallback, useEffect, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import buttons from "../../components/actionButtons.module.css";
import type { ConfigurableAgent } from "../../lib/desktop/agents";
import {
    VersionRestoreRequestError,
    readVersionText,
    type VersionText,
    type VersionTextSide,
} from "../../lib/desktop/versionRestore";
import styles from "./AgentList.module.css";
import { ConfigText } from "./ConfigText";
import dialog from "./VersionTextDialog.module.css";

type TextState =
    | { kind: "loading" }
    | { kind: "preview" }
    | { kind: "error"; code: VersionRestoreRequestError["code"] }
    | { kind: "ready"; text: VersionText };

export type CurrentConfigTextFile = {
    revisionId: string;
    targetPath: string;
    currentDigest: string | null;
};

/**
 * Page-level 「详细配置」 dialog: sanitized *current* configuration text only.
 *
 * Bound to a matched history entry so Rust can re-check the live digest. Never
 * shows retained/planned text or a dual-column layout — those belong nowhere
 * on the view-version path, and write previews use field diffs instead.
 */
export function CurrentConfigurationDialog({
    agent,
    entryKey,
    files,
    initialRevisionId,
    onClose,
    returnFocus,
}: {
    agent: ConfigurableAgent;
    entryKey: string;
    files: CurrentConfigTextFile[];
    initialRevisionId: string;
    onClose: () => void;
    returnFocus: () => HTMLElement | null;
}): JSX.Element {
    const { t } = useTranslation();
    const [selectedId, setSelectedId] = useState(initialRevisionId);
    const [reload, setReload] = useState(0);
    const [state, setState] = useState<TextState>({ kind: "loading" });
    const generation = useRef(0);
    const title = useRef<HTMLHeadingElement>(null);

    const loadText = useCallback(
        (id: string) => {
            const file = files.find((candidate) => candidate.revisionId === id);
            if (file === undefined) {
                return Promise.reject(
                    new VersionRestoreRequestError("invalid_request"),
                );
            }
            return readVersionText(
                agent,
                entryKey,
                file.revisionId,
                file.currentDigest,
            );
        },
        [agent, entryKey, files],
    );

    useEffect(() => {
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
                            <CurrentTextSide
                                side={state.text.current}
                                format={state.text.format}
                                label={t("agents.version.textCurrent")}
                            />
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

/** One read-only current side: present text, absence or a safe reason. */
function CurrentTextSide({
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
