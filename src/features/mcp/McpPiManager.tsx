import { Dialog } from "@base-ui/react/dialog";
import { useEffect, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import { DetailFact, DetailPanel } from "../../components/DetailForm";
import {
    applyPiMcpChange,
    PiMcpError,
    previewPiMcpChange,
    readPiMcpHistory,
    readPiMcpState,
    recoverPiMcpChange,
    type PiMcpAction,
    type PiMcpApplied,
    type PiMcpErrorCode,
    type PiMcpHistory,
    type PiMcpPlan,
    type PiMcpRecovery,
    type PiMcpSelection,
    type PiMcpState,
} from "../../lib/desktop/piMcp";
import type { McpRecord } from "../../lib/desktop/mcp";
import { PiMcpReview } from "./PiMcpReview";
import buttons from "../../components/actionButtons.module.css";
import dialog from "./McpDeleteDialog.module.css";
import styles from "./McpPiPreview.module.css";
import manager from "./McpPiManager.module.css";

type ReadState =
    | { kind: "loading" }
    | { kind: "error"; code: PiMcpErrorCode }
    | { kind: "ready"; state: PiMcpState; history: PiMcpHistory };
type Step =
    | { kind: "idle" }
    | { kind: "previewing" | "running" }
    | { kind: "review"; selection: PiMcpSelection; plan: PiMcpPlan }
    | { kind: "recovery"; recovery: PiMcpRecovery }
    | { kind: "error"; code: PiMcpErrorCode }
    | { kind: "done"; result: PiMcpApplied };
function code(error: unknown): PiMcpErrorCode {
    return error instanceof PiMcpError ? error.code : "outcome_unknown";
}
/** Complete per-definition Pi registration workflow. Reads expire on retry/close;
 * a confirmed write is never cancelled by navigation, duplicated, or auto-retried.
 * Parents retain a busy guard even if hiding this dialog unmounts its view. */
export function McpPiManager({
    record,
    returnFocus,
    onClose,
    onReload,
    onBusyChange,
    onChanged,
}: {
    record: McpRecord;
    returnFocus: HTMLElement | null;
    onClose: () => void;
    onReload: () => void;
    onBusyChange: (busy: boolean) => void;
    onChanged: () => void;
}): JSX.Element {
    const { t, i18n } = useTranslation();
    const [read, setRead] = useState<ReadState>({ kind: "loading" });
    const [step, setStep] = useState<Step>({ kind: "idle" });
    const [attempt, setAttempt] = useState(0);
    const [more, setMore] = useState(false);
    const alive = useRef(false),
        generation = useRef(0),
        mutation = useRef(false),
        readingMore = useRef(false);
    const closeButton = useRef<HTMLButtonElement>(null);
    useEffect(() => {
        alive.current = true;
        return () => {
            alive.current = false;
            generation.current += 1;
        };
    }, []);
    useEffect(() => {
        // IPC has no cancellation signal. Only this mounted read generation may
        // replace data, including after StrictMode's initial cleanup.
        const token = ++generation.current;
        Promise.all([
            readPiMcpState(record.id),
            readPiMcpHistory(record.id),
        ]).then(
            ([state, history]) => {
                if (alive.current && generation.current === token)
                    setRead({ kind: "ready", state, history });
            },
            (error: unknown) => {
                if (alive.current && generation.current === token)
                    setRead({ kind: "error", code: code(error) });
            },
        );
        return () => {
            generation.current += 1;
        };
    }, [record.id, record.revision, attempt]);
    const locked =
        step.kind === "previewing" || step.kind === "running" || more;
    function refresh(): void {
        if (mutation.current || readingMore.current) return;
        generation.current++;
        setRead({ kind: "loading" });
        setStep({ kind: "idle" });
        setAttempt((value) => value + 1);
    }
    async function review(
        action: PiMcpAction,
        historyId: string | null = null,
    ): Promise<void> {
        if (locked || mutation.current) return;
        const token = ++generation.current;
        const selection: PiMcpSelection = {
            definitionId: record.id,
            expectedRevision: record.revision,
            action,
            historyId,
        };
        setStep({ kind: "previewing" });
        try {
            const plan = await previewPiMcpChange(selection);
            if (alive.current && token === generation.current)
                setStep({ kind: "review", selection, plan });
        } catch (error: unknown) {
            if (alive.current && token === generation.current)
                setStep({ kind: "error", code: code(error) });
        }
    }
    async function confirm(): Promise<void> {
        if (
            mutation.current ||
            (step.kind !== "review" && step.kind !== "recovery")
        )
            return;
        const reviewed = step;
        mutation.current = true;
        onBusyChange(true);
        setStep({ kind: "running" });
        try {
            const result =
                reviewed.kind === "review"
                    ? await applyPiMcpChange(reviewed.selection, reviewed.plan)
                    : await recoverPiMcpChange(reviewed.recovery);
            if (alive.current) {
                setStep({ kind: "done", result });
                setRead({ kind: "loading" });
                setAttempt((value) => value + 1);
            }
        } catch (error: unknown) {
            if (alive.current) {
                setStep({ kind: "error", code: code(error) });
                setRead({ kind: "loading" });
                setAttempt((value) => value + 1);
            }
        } finally {
            mutation.current = false;
            onBusyChange(false);
            onChanged();
        }
    }
    async function loadMore(): Promise<void> {
        if (
            read.kind !== "ready" ||
            read.history.nextCursor === null ||
            locked ||
            readingMore.current ||
            mutation.current
        )
            return;
        const snapshot = read;
        const token = generation.current;
        readingMore.current = true;
        setMore(true);
        try {
            const page = await readPiMcpHistory(
                record.id,
                snapshot.history.nextCursor,
            );
            if (alive.current && generation.current === token) {
                const seen = new Set(
                    snapshot.history.items.map((row) => row.id),
                );
                setRead({
                    ...snapshot,
                    history: {
                        items: [
                            ...snapshot.history.items,
                            ...page.items.filter((row) => !seen.has(row.id)),
                        ],
                        nextCursor: page.nextCursor,
                    },
                });
            }
        } catch (error: unknown) {
            if (alive.current && generation.current === token)
                setStep({ kind: "error", code: code(error) });
        } finally {
            readingMore.current = false;
            if (alive.current) setMore(false);
        }
    }
    const data = read.kind === "ready" ? read : null;
    const current = data?.state.deployments.find(
        (row) => row.configRoot === data.state.configRoot,
    );
    const pending = data?.state.recovery;
    const reviewing = step.kind === "review" || step.kind === "recovery";
    return (
        <Dialog.Root
            open
            onOpenChange={(open) => {
                if (!open && !mutation.current) onClose();
            }}
        >
            <Dialog.Portal>
                <Dialog.Backdrop className={dialog["backdrop"]} />
                <Dialog.Popup
                    className={[dialog["popup"], styles["popup"]].join(" ")}
                    initialFocus={closeButton}
                    finalFocus={() =>
                        returnFocus?.isConnected ? returnFocus : null
                    }
                >
                    <Dialog.Title className={dialog["title"]}>
                        {t("mcp.pi.manage.title", { name: record.displayName })}
                    </Dialog.Title>
                    <Dialog.Description className={dialog["body"]}>
                        {t("mcp.pi.manage.description")}
                    </Dialog.Description>
                    <div className={styles["content"]}>
                        <p className={styles["notice"]}>
                            {t("mcp.piPreview.runtime")}
                        </p>
                        {step.kind === "error" && (
                            <p role="alert" className={dialog["error"]}>
                                {t(`mcp.pi.errors.${step.code}`)}
                            </p>
                        )}
                        {step.kind === "done" && (
                            <p role="status">
                                {t(
                                    step.result.reloadRequired
                                        ? "mcp.pi.manage.written"
                                        : "mcp.pi.manage.recorded",
                                )}
                            </p>
                        )}
                        {(step.kind === "previewing" ||
                            step.kind === "running") && (
                            <p role="status">
                                {t(
                                    step.kind === "running"
                                        ? "mcp.pi.manage.writing"
                                        : "mcp.piPreview.loading",
                                )}
                            </p>
                        )}
                        {step.kind === "review" && (
                            <PiMcpReview plan={step.plan} />
                        )}
                        {step.kind === "recovery" && (
                            <section
                                className={styles["section"]}
                                aria-label={t("mcp.pi.recovery.review")}
                            >
                                <p>{t("mcp.pi.recovery.noReplay")}</p>
                                <p>
                                    {t(
                                        `mcp.pi.recovery.${step.recovery.observation}`,
                                    )}
                                </p>
                            </section>
                        )}
                        {!reviewing &&
                            step.kind !== "running" &&
                            step.kind !== "previewing" && (
                                <>
                                    {read.kind === "loading" && (
                                        <p role="status">
                                            {t("mcp.pi.manage.loading")}
                                        </p>
                                    )}
                                    {read.kind === "error" && (
                                        <p role="alert">
                                            {t(`mcp.pi.errors.${read.code}`)}
                                        </p>
                                    )}
                                    {data !== null && (
                                        <>
                                            <DetailPanel
                                                label={t(
                                                    "mcp.pi.manage.registration",
                                                )}
                                            >
                                                <DetailFact
                                                    label={t(
                                                        "mcp.piPreview.target",
                                                    )}
                                                    value={
                                                        data.state.targetPath
                                                    }
                                                />
                                                <DetailFact
                                                    label={t(
                                                        "mcp.pi.manage.diskState",
                                                    )}
                                                    value={t(
                                                        current
                                                            ? `mcp.pi.states.${current.state}`
                                                            : "mcp.pi.states.unregistered",
                                                    )}
                                                />
                                                {current && (
                                                    <DetailFact
                                                        label={t(
                                                            "mcp.piPreview.enabled",
                                                        )}
                                                        value={t(
                                                            current.ownership
                                                                .enabled
                                                                ? "mcp.piPreview.on"
                                                                : "mcp.piPreview.off",
                                                        )}
                                                    />
                                                )}
                                                {current && (
                                                    <DetailFact
                                                        label={t(
                                                            "mcp.pi.manage.revision",
                                                        )}
                                                        value={t(
                                                            "mcp.pi.manage.revisionValue",
                                                            {
                                                                written:
                                                                    current
                                                                        .ownership
                                                                        .definitionRevision,
                                                                central:
                                                                    record.revision,
                                                            },
                                                        )}
                                                    />
                                                )}
                                            </DetailPanel>
                                            {data.state.deployments
                                                .filter(
                                                    (row) =>
                                                        row.configRoot !==
                                                        data.state.configRoot,
                                                )
                                                .map((row) => (
                                                    <p
                                                        key={row.configRoot}
                                                        className={
                                                            styles["notice"]
                                                        }
                                                    >
                                                        {t(
                                                            "mcp.pi.manage.otherRoot",
                                                            {
                                                                path: row.configRoot,
                                                            },
                                                        )}
                                                    </p>
                                                ))}
                                            {pending && (
                                                <section
                                                    className={
                                                        manager["recovery"]
                                                    }
                                                    aria-label={t(
                                                        "mcp.pi.recovery.title",
                                                    )}
                                                >
                                                    <h3>
                                                        {t(
                                                            "mcp.pi.recovery.title",
                                                        )}
                                                    </h3>
                                                    <p>
                                                        {t(
                                                            "mcp.pi.recovery.pending",
                                                            {
                                                                name: pending.serverName,
                                                            },
                                                        )}
                                                    </p>
                                                    <p>
                                                        {t(
                                                            `mcp.pi.recovery.${pending.observation}`,
                                                        )}
                                                    </p>
                                                    {pending.approval !==
                                                        null && (
                                                        <button
                                                            type="button"
                                                            className={
                                                                buttons[
                                                                    "secondary"
                                                                ]
                                                            }
                                                            disabled={locked}
                                                            onClick={() => {
                                                                setStep({
                                                                    kind: "recovery",
                                                                    recovery:
                                                                        pending,
                                                                });
                                                            }}
                                                        >
                                                            {t(
                                                                "mcp.pi.recovery.review",
                                                            )}
                                                        </button>
                                                    )}
                                                </section>
                                            )}
                                            <div className={manager["actions"]}>
                                                <button
                                                    type="button"
                                                    className={
                                                        buttons["primary"]
                                                    }
                                                    disabled={
                                                        locked || !!pending
                                                    }
                                                    onClick={() => {
                                                        void review("deploy");
                                                    }}
                                                >
                                                    {t(
                                                        current
                                                            ? "mcp.pi.manage.update"
                                                            : "mcp.pi.manage.deploy",
                                                    )}
                                                </button>
                                                {current && (
                                                    <>
                                                        <button
                                                            type="button"
                                                            className={
                                                                buttons[
                                                                    "secondary"
                                                                ]
                                                            }
                                                            disabled={
                                                                locked ||
                                                                !!pending
                                                            }
                                                            onClick={() => {
                                                                void review(
                                                                    current
                                                                        .ownership
                                                                        .enabled
                                                                        ? "disable"
                                                                        : "enable",
                                                                );
                                                            }}
                                                        >
                                                            {t(
                                                                current
                                                                    .ownership
                                                                    .enabled
                                                                    ? "mcp.pi.actions.disable"
                                                                    : "mcp.pi.actions.enable",
                                                            )}
                                                        </button>
                                                        <button
                                                            type="button"
                                                            className={
                                                                buttons[
                                                                    "secondary"
                                                                ]
                                                            }
                                                            disabled={
                                                                locked ||
                                                                !!pending
                                                            }
                                                            onClick={() => {
                                                                void review(
                                                                    "revoke",
                                                                );
                                                            }}
                                                        >
                                                            {t(
                                                                "mcp.pi.actions.revoke",
                                                            )}
                                                        </button>
                                                    </>
                                                )}
                                            </div>
                                            <section
                                                className={styles["section"]}
                                                aria-label={t(
                                                    "mcp.pi.history.title",
                                                )}
                                            >
                                                <h3>
                                                    {t("mcp.pi.history.title")}
                                                </h3>
                                                <p className={styles["notice"]}>
                                                    {t(
                                                        "mcp.pi.history.description",
                                                    )}
                                                </p>
                                                {data.history.items.length ===
                                                    0 && (
                                                    <p>
                                                        {t(
                                                            "mcp.pi.history.empty",
                                                        )}
                                                    </p>
                                                )}
                                                <ul
                                                    className={
                                                        manager["history"]
                                                    }
                                                >
                                                    {data.history.items.map(
                                                        (row) => (
                                                            <li key={row.id}>
                                                                <div>
                                                                    <span>
                                                                        {t(
                                                                            `mcp.pi.actions.${row.action}`,
                                                                        )}
                                                                    </span>
                                                                    <time
                                                                        dateTime={new Date(
                                                                            row.createdAt,
                                                                        ).toISOString()}
                                                                    >
                                                                        {new Intl.DateTimeFormat(
                                                                            i18n.language,
                                                                            {
                                                                                dateStyle:
                                                                                    "short",
                                                                                timeStyle:
                                                                                    "short",
                                                                            },
                                                                        ).format(
                                                                            row.createdAt,
                                                                        )}
                                                                    </time>
                                                                </div>
                                                                <button
                                                                    type="button"
                                                                    className={
                                                                        buttons[
                                                                            "secondary"
                                                                        ]
                                                                    }
                                                                    disabled={
                                                                        locked ||
                                                                        !!pending
                                                                    }
                                                                    aria-label={t(
                                                                        "mcp.pi.history.restoreLabel",
                                                                        {
                                                                            id: row.id.slice(
                                                                                0,
                                                                                8,
                                                                            ),
                                                                        },
                                                                    )}
                                                                    onClick={() => {
                                                                        void review(
                                                                            "restore",
                                                                            row.id,
                                                                        );
                                                                    }}
                                                                >
                                                                    {t(
                                                                        "mcp.pi.actions.restore",
                                                                    )}
                                                                </button>
                                                            </li>
                                                        ),
                                                    )}
                                                </ul>
                                                {data.history.nextCursor !==
                                                    null && (
                                                    <button
                                                        type="button"
                                                        className={
                                                            buttons["secondary"]
                                                        }
                                                        disabled={locked}
                                                        onClick={() => {
                                                            void loadMore();
                                                        }}
                                                    >
                                                        {t("mcp.loadMore")}
                                                    </button>
                                                )}
                                            </section>
                                        </>
                                    )}
                                </>
                            )}
                    </div>
                    <div className={dialog["actions"]}>
                        {reviewing && (
                            <button
                                type="button"
                                className={buttons["primary"]}
                                onClick={() => {
                                    void confirm();
                                }}
                            >
                                {t(
                                    step.kind === "recovery"
                                        ? "mcp.pi.recovery.confirm"
                                        : "mcp.pi.manage.confirm",
                                )}
                            </button>
                        )}
                        {reviewing && (
                            <button
                                type="button"
                                className={buttons["secondary"]}
                                onClick={() => {
                                    setStep({ kind: "idle" });
                                }}
                            >
                                {t("mcp.pi.manage.cancelReview")}
                            </button>
                        )}
                        {!reviewing && (
                            <button
                                type="button"
                                className={buttons["secondary"]}
                                disabled={locked}
                                onClick={refresh}
                            >
                                {t("mcp.pi.manage.refresh")}
                            </button>
                        )}
                        {step.kind === "error" &&
                            step.code === "definition_changed" && (
                                <button
                                    type="button"
                                    className={buttons["secondary"]}
                                    onClick={onReload}
                                >
                                    {t("mcp.piPreview.reload")}
                                </button>
                            )}
                        <button
                            ref={closeButton}
                            type="button"
                            className={buttons["secondary"]}
                            disabled={step.kind === "running"}
                            onClick={() => {
                                if (!mutation.current) onClose();
                            }}
                        >
                            {t("mcp.piPreview.close")}
                        </button>
                    </div>
                </Dialog.Popup>
            </Dialog.Portal>
        </Dialog.Root>
    );
}
