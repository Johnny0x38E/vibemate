import { useEffect, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { DetailPanel } from "../../components/DetailForm";
import buttons from "../../components/actionButtons.module.css";
import { IconButton } from "../../components/IconButton";
import listActions from "../../components/listActions.module.css";
import {
    RestoreRequestError,
    planAgentRestore,
    applyAgentRestore,
    type ApplyState as JournalApplyState,
    type ClaudeCodeRestorePlan,
    type ConfigurableAgent,
    type RestoreErrorCode,
    type RestoreOutcome,
    type RestorePlanStep,
    type RestoreReasonCode,
    type RestoreState,
    type RestoreStepOutcome,
    type WriteErrorCode,
} from "../../lib/desktop/agents";
import {
    HistoryRequestError,
    readConfigurationHistory,
    type HistoryCurrentConfig,
    type HistoryCursor,
    type HistoryEntry,
    type HistoryRevisionState,
} from "../../lib/desktop/history";
import {
    VersionRestoreRequestError,
    applyVersionRestore,
    planVersionRestore,
    type VersionRestoreErrorCode,
    type VersionRestoreFile,
    type VersionRestoreOutcome,
    type VersionRestorePlan,
} from "../../lib/desktop/versionRestore";
import styles from "./AgentList.module.css";
import card from "./ConfigPreview.module.css";
import restore from "./RestoreView.module.css";
import { ResolutionReviewCard } from "./ResolutionReviewCard";
import {
    VersionRestorePlanCard,
    VersionRestoreResultCard,
} from "./VersionRestoreCard";
import { VersionTextDialog } from "./VersionTextDialog";
import {
    HistoryDeleteAction,
    historyDeleteBlockReason,
} from "./HistoryDeleteAction";
import { formatTime } from "./formatTime";
import {
    reviewConfigurationResolution,
    resolveConfigurationOperation,
    type ResolutionReview,
    type ResolutionOutcome,
} from "../../lib/desktop/recovery";

type ListState =
    | { kind: "loading" }
    | { kind: "preview" }
    | { kind: "error"; code: RestoreErrorCode }
    | {
          kind: "ready";
          /** Live pin from every page; independent of the seek cursor. */
          current: HistoryCurrentConfig;
          /** Newest first; every multi-file batch stays inside one entry. */
          entries: HistoryEntry[];
          /** Seek position for the next page, or null at the end. */
          nextCursor: HistoryCursor | null;
          loadingMore: boolean;
          /** A failed "load more" keeps the entries already read. */
          moreError: RestoreErrorCode | null;
      };

/** `loading` keeps the row it belongs to, so only that row shows progress. */
type PreviewState =
    | { kind: "idle" }
    | { kind: "loading"; operationId: string }
    | { kind: "inspecting"; operationId: string }
    | { kind: "resolution"; review: ResolutionReview }
    | { kind: "resolutionError"; code: RestoreErrorCode }
    | { kind: "preview" }
    | { kind: "error"; code: RestoreErrorCode }
    | { kind: "ready"; plan: ClaudeCodeRestorePlan };

/** The restore's own outcome; it outlives the plan it was built from. */
type ApplyState =
    | { kind: "idle" }
    | { kind: "applying" }
    | { kind: "resolving" }
    | { kind: "resolved"; result: ResolutionOutcome }
    | { kind: "resolutionError"; code: RestoreErrorCode }
    | { kind: "error"; code: RestoreErrorCode }
    | { kind: "done"; result: RestoreOutcome };

/** H2 version restore preview, separate from the legacy operation rollback. */
type VersionPlanState =
    | { kind: "idle" }
    | { kind: "preview" }
    | { kind: "loading"; entryKey: string }
    | { kind: "error"; code: VersionRestoreErrorCode }
    | { kind: "ready"; plan: VersionRestorePlan };

/** Result of one confirmed version restore. */
type VersionApplyState =
    | { kind: "idle" }
    | { kind: "applying" }
    | { kind: "error"; code: VersionRestoreErrorCode }
    | { kind: "done"; result: VersionRestoreOutcome };

/**
 * Configuration writes vibemate recorded for one Agent, with a preview of what
 * undoing one would do.
 *
 * Read-only until the user confirms a preview: the list carries paths, states
 * and digests, never file content, and the restore only writes the content the
 * recorded write replaced. A file that changed after its write stays a conflict
 * the user resolves by hand, and every answer replaces the previously shown
 * preview, so a confirmation always belongs to what is on screen. Rust keeps the
 * histories apart, so a Claude Code row can never restore a Pi or Grok file.
 */
export function RestoreView({
    agent = "claudeCode",
    onBusyChange,
    blocked = false,
    hidePin = false,
    onCurrentChange,
}: {
    agent?: ConfigurableAgent;
    onBusyChange?: (busy: boolean) => void;
    blocked?: boolean;
    /** When true, the page-level top owns the current-config pin. */
    hidePin?: boolean;
    /** Notifies the parent whenever the live pin from a page changes. */
    onCurrentChange?: (
        current: HistoryCurrentConfig | null,
        summary?: { providerLabel: string | null; modelLabel: string | null },
    ) => void;
}): JSX.Element {
    const { t, i18n } = useTranslation();
    const [list, setList] = useState<ListState>({ kind: "loading" });
    const [preview, setPreview] = useState<PreviewState>({ kind: "idle" });
    const [apply, setApply] = useState<ApplyState>({ kind: "idle" });
    const [versionPlan, setVersionPlan] = useState<VersionPlanState>({
        kind: "idle",
    });
    const [versionApply, setVersionApply] = useState<VersionApplyState>({
        kind: "idle",
    });
    const [textTarget, setTextTarget] = useState<{
        plan: VersionRestorePlan;
        file: VersionRestoreFile;
    } | null>(null);
    const textTrigger = useRef<HTMLElement | null>(null);
    const [attempt, setAttempt] = useState(0);
    const [deleteBusy, setDeleteBusy] = useState(false);
    const pending = useRef(false);
    const busyCallback = useRef(onBusyChange);
    useEffect(() => {
        // Keep cleanup and late async completions bound to the current parent.
        busyCallback.current = onBusyChange;
    }, [onBusyChange]);
    const currentCallback = useRef(onCurrentChange);
    useEffect(() => {
        currentCallback.current = onCurrentChange;
    }, [onCurrentChange]);
    const mounted = useRef(false);
    const listGeneration = useRef(0);

    useEffect(() => {
        mounted.current = true;
        // A late reply must not update a view that is no longer mounted.
        return () => {
            mounted.current = false;
            busyCallback.current?.(false);
        };
    }, []);

    useEffect(() => {
        // A generation counter (not a captured flag) invalidates StrictMode's
        // discarded first request and every read that a newer one replaced.
        listGeneration.current += 1;
        const generation = listGeneration.current;
        readConfigurationHistory(agent).then(
            (result) => {
                if (listGeneration.current !== generation) return;
                setList(
                    result.kind === "preview"
                        ? { kind: "preview" }
                        : {
                              kind: "ready",
                              current: result.page.current,
                              entries: result.page.entries,
                              nextCursor: result.page.nextCursor,
                              loadingMore: false,
                              moreError: null,
                          },
                );
            },
            (error: unknown) => {
                if (listGeneration.current !== generation) return;
                setList({
                    kind: "error",
                    code:
                        error instanceof HistoryRequestError
                            ? error.code
                            : "operation_failed",
                });
            },
        );
        return () => {
            listGeneration.current += 1;
        };
    }, [attempt, agent]);

    useEffect(() => {
        // Publish through a ref so an inline parent callback cannot retrigger
        // this effect and loop setState on every render.
        if (list.kind !== "ready") {
            currentCallback.current?.(null);
            return;
        }
        const matchedKey = list.current.matchedEntryKey;
        const matched =
            list.current.status === "matched" && matchedKey !== null
                ? list.entries.find((entry) => entry.entryKey === matchedKey)
                : undefined;
        const providerLabel = matched?.ownerLabel ?? matched?.ownerRef ?? null;
        // History pages do not yet carry a resolved model label.
        currentCallback.current?.(list.current, {
            providerLabel,
            modelLabel: null,
        });
    }, [list]);

    // A page read and a restore action share one list; do not let them interleave
    // so a late page cannot re-add rows an action has already settled.
    const readingMore = list.kind === "ready" && list.loadingMore;

    /**
     * Append the next seek page without resetting previews or outcomes.
     *
     * The generation check makes a page read that a newer first-page load
     * replaced harmless. A failure keeps the already-read rows and offers the
     * same button again, rather than claiming the history ends there.
     */
    async function loadMore(): Promise<void> {
        if (
            list.kind !== "ready" ||
            list.nextCursor === null ||
            list.loadingMore ||
            pending.current ||
            blocked
        )
            return;
        const generation = listGeneration.current;
        const snapshot = list;
        setList({ ...snapshot, loadingMore: true, moreError: null });
        try {
            const result = await readConfigurationHistory(
                agent,
                snapshot.nextCursor,
            );
            if (!mounted.current || generation !== listGeneration.current)
                return;
            if (result.kind === "preview") {
                setList({ kind: "preview" });
                return;
            }
            const seen = new Set(
                snapshot.entries.map((entry) => entry.entryKey),
            );
            setList({
                kind: "ready",
                // Every page repeats the same live pin; keep the freshest copy.
                current: result.page.current,
                entries: [
                    ...snapshot.entries,
                    ...result.page.entries.filter(
                        (entry) => !seen.has(entry.entryKey),
                    ),
                ],
                nextCursor: result.page.nextCursor,
                loadingMore: false,
                moreError: null,
            });
        } catch (error: unknown) {
            if (!mounted.current || generation !== listGeneration.current)
                return;
            setList({
                ...snapshot,
                loadingMore: false,
                moreError:
                    error instanceof HistoryRequestError
                        ? error.code
                        : "operation_failed",
            });
        }
    }

    /**
     * Ask Rust what restoring one retained version would change.
     *
     * Reads journal metadata and the current files; snapshots are verified but
     * no file is written and no credential is read. Starting this preview
     * discards the legacy rollback preview so only one card is on screen.
     */
    async function runVersionPreview(entryKey: string): Promise<void> {
        if (blocked || pending.current || readingMore) return;
        pending.current = true;
        busyCallback.current?.(true);
        setPreview({ kind: "idle" });
        setApply({ kind: "idle" });
        setVersionApply({ kind: "idle" });
        setTextTarget(null);
        setVersionPlan({ kind: "loading", entryKey });
        try {
            const result = await planVersionRestore(agent, entryKey);
            if (!mounted.current) return;
            setVersionPlan(
                result.kind === "preview"
                    ? { kind: "preview" }
                    : { kind: "ready", plan: result.plan },
            );
        } catch (error: unknown) {
            if (!mounted.current) return;
            setVersionPlan({
                kind: "error",
                code:
                    error instanceof VersionRestoreRequestError
                        ? error.code
                        : "operation_failed",
            });
        } finally {
            pending.current = false;
            busyCallback.current?.(false);
        }
    }

    /**
     * Confirm a reviewed version restore.
     *
     * Only the files the preview said would change travel back, each with the
     * current digest it showed. Any answer clears the card and re-reads the
     * history; a stale approval cannot be applied twice.
     */
    async function runVersionApply(plan: VersionRestorePlan): Promise<void> {
        if (blocked || pending.current || readingMore || !plan.restorable)
            return;
        pending.current = true;
        busyCallback.current?.(true);
        setTextTarget(null);
        setVersionApply({ kind: "applying" });
        try {
            const result = await applyVersionRestore(
                agent,
                plan.entryKey,
                reviewedVersionFiles(plan),
            );
            if (!mounted.current) return;
            setVersionPlan({ kind: "idle" });
            setVersionApply({ kind: "done", result });
            setAttempt((current) => current + 1);
        } catch (error: unknown) {
            if (!mounted.current) return;
            const code =
                error instanceof VersionRestoreRequestError
                    ? error.code
                    : "operation_failed";
            if (
                code === "plan_stale" ||
                code === "invalid_response" ||
                code === "operation_failed" ||
                code === "invalid_request" ||
                code === "operation_not_found"
            ) {
                setVersionPlan({ kind: "idle" });
                setAttempt((current) => current + 1);
            }
            setVersionApply({ kind: "error", code });
        } finally {
            pending.current = false;
            busyCallback.current?.(false);
        }
    }

    /**
     * Ask Rust what undoing one recorded write would do.
     *
     * Read-only: the preview reads each file's change token and compares it with
     * what the write left. Nothing is written and no credential is read.
     */
    async function runPreview(operationId: string): Promise<void> {
        if (blocked || pending.current || readingMore) return;
        setVersionPlan({ kind: "idle" });
        setVersionApply({ kind: "idle" });
        pending.current = true;
        busyCallback.current?.(true);
        // A new preview replaces the previous outcome, which described another
        // write and would otherwise sit under the card it no longer belongs to.
        setApply({ kind: "idle" });
        setPreview({ kind: "loading", operationId });
        try {
            const result = await planAgentRestore(agent, operationId);
            if (!mounted.current) return;
            setPreview(
                result.kind === "preview"
                    ? { kind: "preview" }
                    : { kind: "ready", plan: result.plan },
            );
        } catch (error: unknown) {
            if (!mounted.current) return;
            setPreview({
                kind: "error",
                code:
                    error instanceof RestoreRequestError
                        ? error.code
                        : "operation_failed",
            });
        } finally {
            pending.current = false;
            busyCallback.current?.(false);
        }
    }

    /**
     * Put the reviewed content back.
     *
     * The digests the preview showed travel back with the request, so Rust
     * refuses a file that changed again since the preview instead of overwriting
     * it. Any answer clears the preview and re-reads the list: the write this
     * card described is settled, and a second click must start from a fresh one.
     */
    async function runApply(plan: ClaudeCodeRestorePlan): Promise<void> {
        if (blocked || pending.current || readingMore) return;
        pending.current = true;
        busyCallback.current?.(true);
        setApply({ kind: "applying" });
        try {
            const result = await applyAgentRestore(
                agent,
                plan.operationId,
                reviewedSteps(plan),
            );
            if (!mounted.current) return;
            setPreview({ kind: "idle" });
            setApply({ kind: "done", result });
            // The settled states and the new conflict situation are read again,
            // so the buttons under the list describe the files as they are now.
            setAttempt((current) => current + 1);
        } catch (error: unknown) {
            if (!mounted.current) return;
            const code =
                error instanceof RestoreRequestError
                    ? error.code
                    : "operation_failed";
            // A stale plan cannot be applied at all; the other refusals changed
            // nothing, so the user can act on the cause and preview again.
            if (
                code === "plan_stale" ||
                code === "invalid_response" ||
                code === "operation_failed"
            ) {
                setPreview({ kind: "idle" });
                setAttempt((current) => current + 1);
            }
            setApply({ kind: "error", code });
        } finally {
            pending.current = false;
            busyCallback.current?.(false);
        }
    }

    /** Inspect interrupted metadata separately from any configuration restore. */
    async function runInspection(operationId: string): Promise<void> {
        if (blocked || pending.current || readingMore) return;
        pending.current = true;
        busyCallback.current?.(true);
        setApply({ kind: "idle" });
        setVersionPlan({ kind: "idle" });
        setVersionApply({ kind: "idle" });
        setPreview({ kind: "inspecting", operationId });
        try {
            const result = await reviewConfigurationResolution(
                agent,
                operationId,
            );
            if (!mounted.current) return;
            setPreview(
                result.kind === "preview"
                    ? { kind: "preview" }
                    : { kind: "resolution", review: result.review },
            );
        } catch (error: unknown) {
            if (!mounted.current) return;
            setPreview({
                kind: "resolutionError",
                code:
                    error instanceof RestoreRequestError
                        ? error.code
                        : "operation_failed",
            });
        } finally {
            pending.current = false;
            busyCallback.current?.(false);
        }
    }

    /** Record only the reviewed observations. Never chain a restore afterwards. */
    async function runResolution(review: ResolutionReview): Promise<void> {
        if (blocked || pending.current || readingMore || !review.resolvable)
            return;
        pending.current = true;
        busyCallback.current?.(true);
        setApply({ kind: "resolving" });
        setVersionPlan({ kind: "idle" });
        setVersionApply({ kind: "idle" });
        try {
            const result = await resolveConfigurationOperation(agent, review);
            if (!mounted.current) return;
            setApply({ kind: "resolved", result });
        } catch (error: unknown) {
            if (!mounted.current) return;
            setApply({
                kind: "resolutionError",
                code:
                    error instanceof RestoreRequestError
                        ? error.code
                        : "operation_failed",
            });
        } finally {
            // A refusal may be stale or an uncertain IPC result. Always discard
            // approval and reread history; recording is never blindly repeated.
            if (mounted.current) {
                setPreview({ kind: "idle" });
                setList({ kind: "loading" });
                setAttempt((current) => current + 1);
            }
            pending.current = false;
            busyCallback.current?.(false);
        }
    }

    if (list.kind === "preview") {
        return (
            <DetailPanel label={t("agents.restore.title")}>
                <p className={styles["hint"]}>
                    {t("agents.restore.previewMode")}
                </p>
            </DetailPanel>
        );
    }

    const busy =
        blocked ||
        deleteBusy ||
        readingMore ||
        preview.kind === "loading" ||
        preview.kind === "inspecting" ||
        apply.kind === "applying" ||
        apply.kind === "resolving" ||
        versionPlan.kind === "loading" ||
        versionApply.kind === "applying";

    return (
        <>
            <DetailPanel label={t("agents.restore.title")}>
                {list.kind === "loading" && (
                    <p className={styles["hint"]}>
                        {t("agents.restore.loading")}
                    </p>
                )}
                {list.kind === "error" && (
                    <>
                        <p className={styles["error"]} role="alert">
                            {t(`agents.restore.errors.${list.code}`)}
                        </p>
                        <p className={styles["hint"]}>
                            <button
                                type="button"
                                className={buttons["secondary"]}
                                onClick={() => {
                                    setList({ kind: "loading" });
                                    setAttempt((current) => current + 1);
                                }}
                            >
                                {t("agents.restore.retry")}
                            </button>
                        </p>
                    </>
                )}
                {list.kind === "ready" && (
                    <>
                        {!hidePin && (
                            <CurrentConfigPin current={list.current} />
                        )}
                        {list.entries.length === 0 ? (
                            <p className={styles["hint"]}>
                                {t("agents.restore.empty")}
                            </p>
                        ) : (
                            <ul className={restore["rows"]}>
                                {list.entries.map((entry) => (
                                    <OperationRow
                                        key={entry.entryKey}
                                        agent={agent}
                                        entry={entry}
                                        isCurrent={
                                            list.current.status === "matched" &&
                                            list.current.matchedEntryKey ===
                                                entry.entryKey
                                        }
                                        locale={i18n.language}
                                        busy={busy}
                                        previewing={
                                            preview.kind === "loading" &&
                                            preview.operationId ===
                                                entry.operationId
                                        }
                                        onPreview={() => {
                                            void runPreview(entry.operationId);
                                        }}
                                        inspecting={
                                            preview.kind === "inspecting" &&
                                            preview.operationId ===
                                                entry.operationId
                                        }
                                        onInspect={() => {
                                            void runInspection(
                                                entry.operationId,
                                            );
                                        }}
                                        versionPreviewing={
                                            versionPlan.kind === "loading" &&
                                            versionPlan.entryKey ===
                                                entry.entryKey
                                        }
                                        onVersionPreview={() => {
                                            void runVersionPreview(
                                                entry.entryKey,
                                            );
                                        }}
                                        onDeleted={() => {
                                            setPreview({ kind: "idle" });
                                            setVersionPlan({ kind: "idle" });
                                            setVersionApply({ kind: "idle" });
                                            setApply({ kind: "idle" });
                                            setList({ kind: "loading" });
                                            setAttempt(
                                                (current) => current + 1,
                                            );
                                        }}
                                        onDeleteBusy={(deleting) => {
                                            pending.current = deleting;
                                            setDeleteBusy(deleting);
                                            busyCallback.current?.(deleting);
                                        }}
                                    />
                                ))}
                            </ul>
                        )}
                        {list.moreError !== null && (
                            <p className={styles["error"]} role="alert">
                                {t("agents.restore.loadMoreFailed", {
                                    error: t(
                                        `agents.restore.errors.${list.moreError}`,
                                    ),
                                })}
                            </p>
                        )}
                        <div className={card["actions"]}>
                            {list.nextCursor !== null && (
                                <button
                                    type="button"
                                    className={buttons["secondary"]}
                                    aria-disabled={busy}
                                    onClick={() => {
                                        void loadMore();
                                    }}
                                >
                                    {list.loadingMore
                                        ? t("agents.restore.loadingMore")
                                        : t("agents.restore.loadMore")}
                                </button>
                            )}
                            <button
                                type="button"
                                className={buttons["secondary"]}
                                aria-disabled={busy}
                                onClick={() => {
                                    if (busy || pending.current) return;
                                    setPreview({ kind: "idle" });
                                    setList({ kind: "loading" });
                                    setAttempt((current) => current + 1);
                                }}
                            >
                                {t("agents.restore.recheck")}
                            </button>
                        </div>
                    </>
                )}
                {preview.kind === "resolutionError" && (
                    <p className={styles["error"]} role="alert">
                        {t(`agents.resolution.errors.${preview.code}`)}
                    </p>
                )}
                {preview.kind === "error" && (
                    <p className={styles["error"]} role="alert">
                        {t(`agents.restore.errors.${preview.code}`)}
                    </p>
                )}
            </DetailPanel>
            {preview.kind === "ready" && (
                <RestorePlanCard
                    plan={preview.plan}
                    locale={i18n.language}
                    restoring={apply.kind === "applying"}
                    onConfirm={() => {
                        void runApply(preview.plan);
                    }}
                />
            )}
            {preview.kind === "resolution" && (
                <ResolutionReviewCard
                    review={preview.review}
                    busy={busy}
                    onConfirm={() => {
                        void runResolution(preview.review);
                    }}
                    onCancel={() => {
                        if (busy || pending.current) return;
                        setPreview({ kind: "idle" });
                    }}
                />
            )}
            {apply.kind === "resolved" && (
                <p className={styles["hint"]} role="status">
                    {t("agents.resolution.recorded", {
                        state: t(`agents.restore.states.${apply.result.state}`),
                    })}
                </p>
            )}
            {apply.kind === "done" && (
                <RestoreResultCard result={apply.result} />
            )}
            {apply.kind === "resolutionError" && (
                <p className={styles["error"]} role="alert">
                    {t(`agents.resolution.errors.${apply.code}`)}
                </p>
            )}
            {apply.kind === "error" && (
                <p className={styles["error"]} role="alert">
                    {t(`agents.restore.errors.${apply.code}`)}
                </p>
            )}
            {versionPlan.kind === "ready" && (
                <VersionRestorePlanCard
                    plan={versionPlan.plan}
                    locale={i18n.language}
                    restoring={versionApply.kind === "applying"}
                    onConfirm={() => {
                        void runVersionApply(versionPlan.plan);
                    }}
                    onViewDetails={(file) => {
                        if (file.revisionId === null) return;
                        textTrigger.current =
                            document.activeElement instanceof HTMLElement
                                ? document.activeElement
                                : null;
                        setTextTarget({
                            plan: versionPlan.plan,
                            file,
                        });
                    }}
                />
            )}
            {versionPlan.kind === "error" && (
                <p className={styles["error"]} role="alert">
                    {t(`agents.restore.errors.${versionPlan.code}`)}
                </p>
            )}
            {versionPlan.kind === "preview" && (
                <p className={styles["hint"]}>
                    {t("agents.restore.previewMode")}
                </p>
            )}
            {versionApply.kind === "done" && (
                <VersionRestoreResultCard result={versionApply.result} />
            )}
            {versionApply.kind === "error" && (
                <p className={styles["error"]} role="alert">
                    {t(`agents.restore.errors.${versionApply.code}`)}
                </p>
            )}
            {textTarget !== null && textTarget.file.revisionId !== null && (
                <VersionTextDialog
                    agent={agent}
                    plan={textTarget.plan}
                    initialRevisionId={textTarget.file.revisionId}
                    onClose={() => {
                        setTextTarget(null);
                    }}
                    returnFocus={() =>
                        textTrigger.current?.isConnected
                            ? textTrigger.current
                            : null
                    }
                />
            )}
        </>
    );
}

/**
 * Page-level or inline pin for the live configuration summary.
 *
 * Matching a retained entry only marks that row; it never invents restore rights.
 * Managed files stay visible; Provider/model live on the Write segment instead.
 */
export function CurrentConfigPin({
    current,
}: {
    current: HistoryCurrentConfig;
}): JSX.Element {
    const { t } = useTranslation();
    return (
        <div
            className={restore["currentPin"]}
            role="region"
            aria-label={t("agents.restore.current.title")}
        >
            <div className={restore["rowSummary"]}>
                <span className={restore["state"]}>
                    {t("agents.restore.current.title")}
                    {" · "}
                    {t(`agents.restore.current.statusShort.${current.status}`)}
                </span>
            </div>
            {current.files.length > 0 && (
                <ul className={restore["files"]}>
                    {current.files.map((file) => (
                        <li key={file.targetPath} className={restore["file"]}>
                            <span className={card["path"]}>
                                {file.targetPath}
                            </span>
                            <span className={restore["stepState"]}>
                                {!file.readable
                                    ? t("agents.restore.current.fileUnreadable")
                                    : file.currentDigest === null
                                      ? t("agents.restore.current.fileAbsent")
                                      : t(
                                            "agents.restore.current.fileReadable",
                                        )}
                            </span>
                        </li>
                    ))}
                </ul>
            )}
        </div>
    );
}

/** Concise Provider/Model line at the top of the Write segment. */
export function WriteCurrentSummary({
    providerLabel,
    modelLabel,
}: {
    providerLabel: string | null;
    modelLabel: string | null;
}): JSX.Element {
    const { t } = useTranslation();
    const dash = t("agents.summary.dash");
    return (
        <div className={restore["writeSummary"]} role="status">
            <span>
                {t("agents.summary.provider", {
                    value: providerLabel ?? dash,
                })}
            </span>
            <span>
                {t("agents.summary.model", {
                    value: modelLabel ?? dash,
                })}
            </span>
        </div>
    );
}

/** One journal entry: compact summary by default, files on expand. */
function OperationRow({
    agent,
    entry,
    isCurrent,
    locale,
    busy,
    previewing,
    onPreview,
    inspecting,
    onInspect,
    versionPreviewing,
    onVersionPreview,
    onDeleted,
    onDeleteBusy,
}: {
    agent: ConfigurableAgent;
    entry: HistoryEntry;
    /** Backend named this entry as the full current-configuration match. */
    isCurrent: boolean;
    /** Language used for the timestamp; the app language decides it. */
    locale: string;
    /** Another history read or restore action is in flight. */
    busy: boolean;
    /** This row's legacy rollback preview is being built. */
    previewing: boolean;
    onPreview: () => void;
    inspecting: boolean;
    onInspect: () => void;
    /** This row's retained-version preview is being built. */
    versionPreviewing: boolean;
    onVersionPreview: () => void;
    onDeleted: () => void;
    onDeleteBusy: (busy: boolean) => void;
}): JSX.Element {
    const { t } = useTranslation();
    const [expanded, setExpanded] = useState(false);
    const state = entryState(entry);
    const files = entryFiles(entry);
    const fileCount = files.length;
    const backupShort =
        files.length === 0
            ? null
            : files.every((file) => file.backupState === "available")
              ? "available"
              : files.every((file) => file.backupState === "not_needed")
                ? "not_needed"
                : files.some((file) => file.backupState === "unavailable")
                  ? "unavailable"
                  : "available";
    const deleteBlock = historyDeleteBlockReason(entry);
    const providerText =
        entry.ownerLabel !== null
            ? entry.ownerLabel
            : entry.ownerRef !== null
              ? entry.ownerRef
              : null;

    return (
        <li className={restore["row"]}>
            <div className={restore["rowSummary"]}>
                <div className={restore["rowMain"]}>
                    <span className={restore["state"]}>
                        {t(`agents.restore.intents.${entry.intent}`)}
                        {" · "}
                        {entryStateText(state, t)}
                        {isCurrent && (
                            <>
                                {" · "}
                                <span className={restore["currentMark"]}>
                                    {t("agents.restore.current.mark")}
                                </span>
                            </>
                        )}
                    </span>
                    {providerText !== null && (
                        <span className={restore["meta"]}>{providerText}</span>
                    )}
                    <span className={restore["time"]}>
                        {formatTime(entry.createdAt, locale)}
                    </span>
                    {backupShort !== null && (
                        <span className={restore["meta"]}>
                            {t(`agents.restore.backupShort.${backupShort}`)}
                        </span>
                    )}
                    {fileCount > 0 && (
                        <button
                            type="button"
                            className={restore["expandToggle"]}
                            aria-expanded={expanded}
                            onClick={() => {
                                setExpanded((open) => !open);
                            }}
                        >
                            {t("agents.restore.fileCount", {
                                count: fileCount,
                            })}
                        </button>
                    )}
                </div>
                <div className={listActions["group"]}>
                    {entry.requiresResolution ? (
                        <IconButton
                            icon="check"
                            label={
                                inspecting
                                    ? t("agents.resolution.inspecting")
                                    : t("agents.resolution.inspect")
                            }
                            blocked={busy}
                            onClick={onInspect}
                        />
                    ) : entry.revisions.length > 0 ? (
                        <IconButton
                            icon="logFile"
                            label={
                                versionPreviewing
                                    ? t("agents.version.reviewing")
                                    : t("agents.version.review")
                            }
                            blocked={busy}
                            onClick={onVersionPreview}
                        />
                    ) : entry.intent === "apply" ? (
                        <IconButton
                            icon="logFile"
                            label={
                                previewing
                                    ? t("agents.restore.previewing")
                                    : t("agents.restore.preview")
                            }
                            blocked={busy}
                            onClick={onPreview}
                        />
                    ) : null}
                    <HistoryDeleteAction
                        agent={agent}
                        entryKey={entry.entryKey}
                        blocked={busy}
                        blockReason={deleteBlock}
                        onDeleted={() => {
                            onDeleted();
                        }}
                        onBusyChange={onDeleteBusy}
                    />
                </div>
            </div>
            {expanded && (
                <>
                    <ul className={restore["files"]}>
                        {files.map((file) => (
                            <li key={file.key} className={restore["file"]}>
                                <span className={card["path"]}>
                                    {file.targetPath}
                                </span>
                                {file.note !== null && (
                                    <span className={restore["stepState"]}>
                                        {fileNoteText(file.note, t)}
                                    </span>
                                )}
                                <span className={restore["stepState"]}>
                                    {t(
                                        `agents.restore.backups.${file.backupState}`,
                                    )}
                                </span>
                            </li>
                        ))}
                    </ul>
                    {entry.ownerLabel !== null && (
                        <p className={restore["provider"]}>
                            {t("agents.restore.provider", {
                                name: entry.ownerLabel,
                            })}
                        </p>
                    )}
                    {entry.ownerLabel === null && entry.ownerRef !== null && (
                        <p className={restore["provider"]}>
                            {t("agents.restore.providerRemoved", {
                                id: entry.ownerRef,
                            })}
                        </p>
                    )}
                    {entry.requiresResolution && (
                        <p className={styles["hint"]}>
                            {t("agents.resolution.required")}
                        </p>
                    )}
                </>
            )}
        </li>
    );
}

/** One file row, carrying either the legacy rollback or the batch result. */
type EntryFileNote =
    | { group: "stepStates"; state: RestoreState }
    | { group: "revisionStates"; state: HistoryRevisionState };

/** Files an entry changed, with the state and backup check for each row. */
function entryFiles(entry: HistoryEntry): {
    key: string;
    targetPath: string;
    note: EntryFileNote | null;
    backupState: HistoryEntry["sourceSteps"][number]["backupState"];
}[] {
    if (entry.intent === "apply") {
        return entry.sourceSteps.map((step) => ({
            key: `step:${String(step.ordinal)}`,
            targetPath: step.targetPath,
            note:
                step.restoreState === null
                    ? null
                    : { group: "stepStates", state: step.restoreState },
            // Apply rows report the source-step check; do not invent eligibility.
            backupState: step.backupState,
        }));
    }
    return entry.revisions.flatMap((revision) => {
        // Rust rejects revisions whose ordinal has no source step. Keeping the
        // guard explicit satisfies TypeScript's unchecked-index rule without
        // pretending a missing step has a path.
        const step = entry.sourceSteps[revision.ordinal];
        return step === undefined
            ? []
            : [
                  {
                      key: revision.revisionId,
                      targetPath: step.targetPath,
                      note: {
                          group: "revisionStates" as const,
                          state: revision.state,
                      },
                      backupState: revision.backupState,
                  },
              ];
    });
}

/** One summary state for the row; a batch keeps every file's own state below. */
type EntryState =
    | { group: "states"; state: JournalApplyState }
    | { group: "revisionStates"; state: HistoryRevisionState };

/** Summary state label: operation state or the batch's least certain result. */
function entryState(entry: HistoryEntry): EntryState {
    if (entry.intent === "apply") {
        return { group: "states", state: entry.sourceState };
    }
    const states = entry.revisions.map((revision) => revision.state);
    // A batch has no operation-level result. Report the least certain file
    // result rather than claiming the whole batch completed.
    const state: HistoryRevisionState = states.includes("unknown")
        ? "unknown"
        : states.includes("failed")
          ? "failed"
          : states.includes("prepared")
            ? "prepared"
            : "completed";
    return { group: "revisionStates", state };
}

/** Translate a summary state without building an unchecked dynamic key. */
function entryStateText(state: EntryState, t: Translator): string {
    return state.group === "states"
        ? t(`agents.restore.states.${state.state}`)
        : t(`agents.restore.revisionStates.${state.state}`);
}

/** Translate one file row's state from its own journal family. */
function fileNoteText(note: EntryFileNote, t: Translator): string {
    return note.group === "stepStates"
        ? t(`agents.restore.stepStates.${note.state}`)
        : t(`agents.restore.revisionStates.${note.state}`);
}

/** What restoring one recorded write would do to each file it changed. */
function RestorePlanCard({
    plan,
    locale,
    restoring,
    onConfirm,
}: {
    plan: ClaudeCodeRestorePlan;
    /** Language used for the timestamp; the app language decides it. */
    locale: string;
    restoring: boolean;
    onConfirm: () => void;
}): JSX.Element {
    const { t } = useTranslation();
    // A conflict or a blocked file needs the user's decision before any retry,
    // so the card explains what to do instead of only naming the state.
    const stopped = plan.steps.some(
        (step) => step.action === "conflict" || step.action === "blocked",
    );
    return (
        <DetailPanel label={t("agents.restore.previewHeading")}>
            <div className={card["summary"]}>
                <span className={card["path"]}>
                    {t("agents.restore.previewFor", {
                        time: formatTime(plan.createdAt, locale),
                    })}
                </span>
                <span className={card["fileState"]}>
                    {t(`agents.restore.states.${plan.state}`)}
                </span>
                {plan.providerLabel !== null && (
                    <span className={card["note"]}>
                        {t("agents.restore.provider", {
                            name: plan.providerLabel,
                        })}
                    </span>
                )}{" "}
            </div>
            <ul className={card["changes"]}>
                {plan.steps.map((step) => (
                    <RestorePlanStepRow key={step.ordinal} step={step} />
                ))}
            </ul>
            {stopped && (
                <p className={styles["hint"]}>
                    {t("agents.restore.conflictHint")}
                </p>
            )}
            {plan.restorable && (
                <>
                    <p className={styles["hint"]}>
                        {t("agents.restore.keyNotice")}
                    </p>
                    <div className={card["actions"]}>
                        <button
                            type="button"
                            className={buttons["primary"]}
                            aria-disabled={restoring}
                            onClick={onConfirm}
                        >
                            {restoring
                                ? t("agents.restore.restoring")
                                : t("agents.restore.confirm")}
                        </button>
                    </div>
                </>
            )}
            {!plan.restorable && (
                <p className={styles["hint"]}>
                    {t("agents.restore.nothingToRestore")}
                </p>
            )}
        </DetailPanel>
    );
}

/** One file of a restore preview: the action, its reason and the key owner. */
function RestorePlanStepRow({ step }: { step: RestorePlanStep }): JSX.Element {
    const { t } = useTranslation();
    return (
        <li className={card["change"]}>
            <span className={card["path"]}>{step.targetPath}</span>
            <span className={restore["action"]}>
                {t(`agents.restore.actions.${step.action}`)}
            </span>
            {step.reason !== null && (
                <span className={card["note"]}>
                    {reasonText(step.reason, t)}
                </span>
            )}
            {step.action === "restore" &&
                step.restoredProviderLabel !== null && (
                    <span className={card["note"]}>
                        {t("agents.restore.restoredOwnerNamed", {
                            name: step.restoredProviderLabel,
                        })}
                    </span>
                )}
            {step.action === "restore" &&
                step.restoredProviderLabel === null &&
                step.restoredProviderId !== null && (
                    <span className={card["note"]}>
                        {t("agents.restore.restoredOwnerRemoved", {
                            id: step.restoredProviderId,
                        })}
                    </span>
                )}
            {step.action === "restore" && step.restoredProviderId === null && (
                <span className={card["note"]}>
                    {t("agents.restore.restoredOwnerUnknown")}
                </span>
            )}
        </li>
    );
}

/** What one confirmed restore did: per file, plus copies it could not remove. */
function RestoreResultCard({
    result,
}: {
    result: RestoreOutcome;
}): JSX.Element {
    const { t } = useTranslation();
    return (
        <DetailPanel label={t("agents.restore.outcomeHeading")}>
            <div className={card["summary"]}>
                <span className={card["path"]}>
                    {t(`agents.restore.outcome.${result.state}`)}
                </span>
            </div>
            <ul className={card["notices"]}>
                {result.steps.map((step) => (
                    <li key={step.ordinal}>
                        {step.state === "restored"
                            ? t(
                                  step.action === "remove"
                                      ? "agents.restore.stepRemoved"
                                      : "agents.restore.stepRestored",
                                  { path: step.targetPath },
                              )
                            : t("agents.restore.stepProblem", {
                                  path: step.targetPath,
                                  error: stepErrorText(step, t),
                              })}
                    </li>
                ))}
            </ul>
            {result.leftovers.length > 0 && (
                <>
                    <p className={styles["hint"]}>
                        {t("agents.restore.leftoversHeading")}
                    </p>
                    <ul className={card["notices"]}>
                        {result.leftovers.map((path) => (
                            <li key={path}>{path}</li>
                        ))}
                    </ul>
                    <p className={styles["hint"]}>
                        {t("agents.restore.leftoverHint")}
                    </p>
                </>
            )}
            <p className={styles["hint"]}>{t("agents.restore.resultHint")}</p>
        </DetailPanel>
    );
}

type Translator = TFunction;

/** Translated reason a preview will not carry a step out. */
function reasonText(reason: RestoreReasonCode, t: Translator): string {
    return t(`agents.restore.reasons.${reason}`);
}

/** Tokens the version confirmation sends: exactly the files it would change. */
function reviewedVersionFiles(
    plan: VersionRestorePlan,
): { revisionId: string; currentDigest: string | null }[] {
    return plan.files.flatMap((file) => {
        if (file.action !== "restore" && file.action !== "remove") return [];
        if (file.revisionId === null) return [];
        return [
            {
                revisionId: file.revisionId,
                currentDigest: file.currentDigest,
            },
        ];
    });
}

/** Tokens the confirmation sends back: exactly the steps it would change. */
function reviewedSteps(
    plan: ClaudeCodeRestorePlan,
): { ordinal: number; currentDigest: string | null }[] {
    return plan.steps
        .filter((step) => step.action === "restore" || step.action === "remove")
        .map((step) => ({
            ordinal: step.ordinal,
            currentDigest: step.currentDigest,
        }));
}

/**
 * Translated reason a step did not reach its previous content.
 *
 * A step that did not restore always carries its code; the boundary refuses
 * other shapes, and an unknown code must not render as an empty string.
 */
function stepErrorText(step: RestoreStepOutcome, t: Translator): string {
    const code: WriteErrorCode = step.errorCode ?? "outcome_unknown";
    return t(`agents.restore.reasons.${code}`);
}
