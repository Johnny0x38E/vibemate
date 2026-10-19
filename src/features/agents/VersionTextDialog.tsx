import { Dialog } from "@base-ui/react/dialog";
import { useCallback, useEffect, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import buttons from "../../components/actionButtons.module.css";
import type { ConfigurableAgent } from "../../lib/desktop/agents";
import {
    VersionRestoreRequestError,
    readCurrentConfigurationText,
    type CurrentConfigText,
} from "../../lib/desktop/versionRestore";
import styles from "./AgentList.module.css";
import { ConfigText } from "./ConfigText";
import dialog from "./VersionTextDialog.module.css";

type TextState =
    | { kind: "loading" }
    | { kind: "preview" }
    | { kind: "error"; code: VersionRestoreRequestError["code"] }
    | { kind: "ready"; text: CurrentConfigText };

/**
 * Page-level 「详细配置」 dialog: sanitized live configuration text only.
 *
 * Reads the current file directly and does not need a matched history entry, so
 * an external edit (Grok Build rewrites its own `config.toml` frequently) does
 * not disable the dialog. The caller passes target paths reported by the
 * backend; Rust revalidates each one against the Agent's fixed scope. No
 * retained version, planned text or dual-column layout is shown here.
 */
export function CurrentConfigurationDialog({
    agent,
    files,
    onClose,
    returnFocus,
}: {
    agent: ConfigurableAgent;
    files: string[];
    onClose: () => void;
    returnFocus: () => HTMLElement | null;
}): JSX.Element {
    const { t } = useTranslation();
    const [selectedPath, setSelectedPath] = useState(files[0] ?? "");
    const [reload, setReload] = useState(0);
    const [state, setState] = useState<TextState>({ kind: "loading" });
    const generation = useRef(0);
    const title = useRef<HTMLHeadingElement>(null);

    const loadText = useCallback(
        (path: string) => readCurrentConfigurationText(agent, path),
        [agent],
    );

    useEffect(() => {
        const current = ++generation.current;
        loadText(selectedPath).then(
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
    }, [loadText, selectedPath, reload]);

    const selected = files.includes(selectedPath)
        ? selectedPath
        : (files[0] ?? "");

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
                    className={[
                        dialog["popupCompact"],
                        dialog["popupText"],
                    ].join(" ")}
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
                        {selected}
                    </Dialog.Description>
                    {files.length > 1 && (
                        <div
                            className={dialog["fileTabs"]}
                            role="group"
                            aria-label={t("agents.version.textFiles")}
                        >
                            {files.map((path) => (
                                <button
                                    key={path}
                                    type="button"
                                    className={dialog["fileTab"]}
                                    aria-pressed={path === selected}
                                    aria-label={path}
                                    onClick={() => {
                                        setState({ kind: "loading" });
                                        setSelectedPath(path);
                                    }}
                                >
                                    {baseName(path)}
                                </button>
                            ))}
                        </div>
                    )}
                    {selected === "" && (
                        <p className={styles["error"]} role="alert">
                            {t("agents.restore.errors.invalid_request")}
                        </p>
                    )}
                    {selected !== "" && state.kind === "loading" && (
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
                        <CurrentTextSide
                            side={state.text.current}
                            format={state.text.format}
                        />
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
}: {
    side: CurrentConfigText["current"];
    format: CurrentConfigText["format"];
}): JSX.Element {
    const { t } = useTranslation();
    return (
        <section className={dialog["side"]}>
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
                <ConfigText text={side.text} format={format} />
            )}
        </section>
    );
}

/** Last path segment for the compact file switcher. */
function baseName(path: string): string {
    const parts = path.split(/[\\/]/u);
    return parts[parts.length - 1] ?? path;
}
