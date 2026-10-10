import { useEffect, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import {
    getLogLocation,
    LogRequestError,
    openLogDirectory,
    openLogFile,
    type LogErrorCode,
    type LogLocation,
} from "../../lib/desktop/logs";
import styles from "./LogSettings.module.css";

type LocationState =
    | { kind: "loading" }
    | { kind: "ready"; location: LogLocation }
    | { kind: "preview" }
    | { kind: "error"; code: LogErrorCode };
type OpenState =
    | { kind: "idle" }
    | { kind: "opening"; target: "file" | "directory" }
    | { kind: "error"; code: LogErrorCode };

function errorCode(error: unknown): LogErrorCode {
    return error instanceof LogRequestError ? error.code : "operation_failed";
}

/**
 * Show Rust-owned log paths and request explicit OS opens. No file contents or
 * executable names enter React. Keep requests/results across settings navigation,
 * but ignore obsolete reads and action results after unmount or a newer refresh.
 */
export function LogSettings(): JSX.Element {
    const { t } = useTranslation();
    const [state, setState] = useState<LocationState>({ kind: "loading" });
    const [openState, setOpenState] = useState<OpenState>({ kind: "idle" });
    const [attempt, setAttempt] = useState(0);
    const generation = useRef(0);
    const opening = useRef(false);

    useEffect(() => {
        // Each read has a generation; StrictMode cleanup and unmount invalidate
        // both stale path reads and in-flight OS-open acknowledgments.
        const current = ++generation.current;
        getLogLocation().then(
            (location) => {
                if (current !== generation.current) return;
                setState(
                    location === null
                        ? { kind: "preview" }
                        : { kind: "ready", location },
                );
            },
            (error: unknown) => {
                if (current === generation.current)
                    setState({ kind: "error", code: errorCode(error) });
            },
        );
        return () => {
            generation.current = current + 1;
        };
    }, [attempt]);

    async function open(target: "file" | "directory"): Promise<void> {
        if (state.kind !== "ready" || opening.current) return;
        const current = generation.current;
        opening.current = true;
        setOpenState({ kind: "opening", target });
        try {
            if (target === "file") await openLogFile();
            else await openLogDirectory();
            if (current === generation.current) setOpenState({ kind: "idle" });
        } catch (error: unknown) {
            if (current === generation.current)
                setOpenState({ kind: "error", code: errorCode(error) });
        } finally {
            opening.current = false;
        }
    }

    const busy = openState.kind === "opening";
    const canOpen = state.kind === "ready" && !busy;

    return (
        <section
            className={styles["group"]}
            aria-labelledby="log-settings-title"
        >
            <h2 className={styles["title"]} id="log-settings-title">
                {t("settings.logs.title")}
            </h2>
            {state.kind === "ready" && (
                <dl className={styles["paths"]}>
                    <div>
                        <dt>{t("settings.logs.filePath")}</dt>
                        <dd>
                            <code dir="auto">{state.location.filePath}</code>
                        </dd>
                    </div>
                    <div>
                        <dt>{t("settings.logs.directoryPath")}</dt>
                        <dd>
                            <code dir="auto">
                                {state.location.directoryPath}
                            </code>
                        </dd>
                    </div>
                </dl>
            )}
            {state.kind === "loading" && (
                <p role="status">{t("settings.logs.loading")}</p>
            )}
            {state.kind === "error" && (
                <p role="alert">{t(`settings.logs.errors.${state.code}`)}</p>
            )}
            {state.kind === "preview" ? (
                <p>{t("settings.logs.preview")}</p>
            ) : (
                <>
                    {state.kind === "ready" &&
                        !state.location.fileLoggingActive && (
                            <p role="status">
                                {t("settings.logs.fileInactive")}
                            </p>
                        )}
                    <div className={styles["actions"]}>
                        <button
                            type="button"
                            className={styles["button"]}
                            aria-disabled={!canOpen}
                            onClick={() => {
                                void open("file");
                            }}
                        >
                            {t("settings.logs.view")}
                        </button>
                        <button
                            type="button"
                            className={styles["button"]}
                            aria-disabled={!canOpen}
                            onClick={() => {
                                void open("directory");
                            }}
                        >
                            {t("settings.logs.openDirectory")}
                        </button>
                        <button
                            type="button"
                            className={styles["button"]}
                            aria-disabled={state.kind === "loading" || busy}
                            onClick={() => {
                                if (state.kind === "loading" || opening.current)
                                    return;
                                setState({ kind: "loading" });
                                setOpenState({ kind: "idle" });
                                setAttempt((previous) => previous + 1);
                            }}
                        >
                            {t("settings.logs.refresh")}
                        </button>
                    </div>
                    <p className={styles["hint"]}>{t("settings.logs.hint")}</p>
                    {openState.kind === "opening" && (
                        <p role="status">
                            {t(
                                openState.target === "file"
                                    ? "settings.logs.openingFile"
                                    : "settings.logs.openingDirectory",
                            )}
                        </p>
                    )}
                    {openState.kind === "error" && (
                        <p role="alert">
                            {t(`settings.logs.errors.${openState.code}`)}
                        </p>
                    )}
                </>
            )}
        </section>
    );
}
