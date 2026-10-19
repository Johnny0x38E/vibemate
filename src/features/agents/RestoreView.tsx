import { useEffect, useId, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { DetailPanel } from "../../components/DetailForm";
import buttons from "../../components/actionButtons.module.css";
import { IconButton } from "../../components/IconButton";
import listActions from "../../components/listActions.module.css";
import {
    RestoreRequestError,
    type ApplyState as JournalApplyState,
    type ConfigurableAgent,
    type RestoreErrorCode,
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
    type VersionRestoreOutcome,
    type VersionRestorePlan,
} from "../../lib/desktop/versionRestore";
import styles from "./AgentList.module.css";
import card from "./ConfigPreview.module.css";
import restore from "./RestoreView.module.css";
import { ResolutionReviewCard } from "./ResolutionReviewCard";
import {
    VersionRestorePlanInline,
    VersionRestoreResultCard,
} from "./VersionRestoreCard";
import { CurrentConfigurationDialog } from "./VersionTextDialog";
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
          /** Loaded independently so the current version can always be first. */
          currentEntry: HistoryEntry | null;
          /** Newest first; every multi-file batch stays inside one entry. */
          entries: HistoryEntry[];
          /** Seek position for the next page, or null at the end. */
          nextCursor: HistoryCursor | null;
          loadingMore: boolean;
          /** A failed "load more" keeps the entries already read. */
          moreError: RestoreErrorCode | null;
      };

/** Interrupted-write review state, independent of configuration selection. */
type PreviewState =
    | { kind: "idle" }
    | { kind: "inspecting"; operationId: string }
    | { kind: "resolution"; review: ResolutionReview }
    | { kind: "resolutionError"; code: RestoreErrorCode }
    | { kind: "preview" };

/** Recording an observation never restores configuration files. */
type ApplyState =
    | { kind: "idle" }
    | { kind: "resolving" }
    | { kind: "resolved"; result: ResolutionOutcome }
    | { kind: "resolutionError"; code: RestoreErrorCode };

/** Written-version preview, separate from interrupted-operation inspection. */
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
 * Recorded configuration versions for one Agent, with its live version first.
 *
 * Rows identify original apply results, never the backups preceding those
 * writes or later recovery batches. Selection previews and restores the chosen
 * written bytes. Rust rechecks current digests and retained snapshots at
 * confirmation; the interface refreshes the matching row after every attempt.
 * Scope checks prevent one Agent's history from writing another Agent's files.
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
    onCurrentChange?: (current: HistoryCurrentConfig | null) => void;
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
    /** Which history row is expanded to show an inline version plan. */
    const [versionExpandedKey, setVersionExpandedKey] = useState<string | null>(
        null,
    );
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
                              currentEntry: result.page.currentEntry,
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
        currentCallback.current?.(list.current);
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
                currentEntry: result.page.currentEntry,
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
     * discards interrupted-operation inspection so only one card is on screen.
     */
    async function runVersionPreview(entryKey: string): Promise<void> {
        if (blocked || pending.current || readingMore) return;
        pending.current = true;
        busyCallback.current?.(true);
        setPreview({ kind: "idle" });
        setApply({ kind: "idle" });
        setVersionApply({ kind: "idle" });
        setVersionExpandedKey(entryKey);
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
        setVersionApply({ kind: "applying" });
        try {
            const result = await applyVersionRestore(
                agent,
                plan.entryKey,
                reviewedVersionFiles(plan),
            );
            if (!mounted.current) return;
            setVersionPlan({ kind: "idle" });
            setVersionExpandedKey(null);
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
                setVersionExpandedKey(null);
                setAttempt((current) => current + 1);
            }
            setVersionApply({ kind: "error", code });
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
        preview.kind === "inspecting" ||
        apply.kind === "resolving" ||
        versionPlan.kind === "loading" ||
        versionApply.kind === "applying";

    return (
        <>
            {/* Recheck is outside the history card (own row above), not inside DetailPanel. */}
            {list.kind === "ready" && (
                <div className={restore["toolbar"]}>
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
            )}
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
                            <CurrentConfigPin
                                current={list.current}
                                agent={agent}
                            />
                        )}
                        {list.entries.length === 0 &&
                        list.currentEntry === null ? (
                            <p className={styles["hint"]}>
                                {t("agents.restore.empty")}
                            </p>
                        ) : (
                            <ul className={restore["rows"]}>
                                {[
                                    ...(list.currentEntry === null
                                        ? []
                                        : [list.currentEntry]),
                                    ...list.entries.filter(
                                        (entry) =>
                                            entry.entryKey !==
                                            list.currentEntry?.entryKey,
                                    ),
                                ].map((entry) => (
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
                                            versionPlan.kind === "loading" &&
                                            versionPlan.entryKey ===
                                                entry.entryKey
                                        }
                                        onPreview={() => {
                                            void runVersionPreview(
                                                entry.entryKey,
                                            );
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
                                        versionPlan={
                                            versionPlan.kind === "ready" &&
                                            versionPlan.plan.entryKey ===
                                                entry.entryKey
                                                ? versionPlan.plan
                                                : null
                                        }
                                        versionRestoring={
                                            versionApply.kind === "applying" &&
                                            versionPlan.kind === "ready" &&
                                            versionPlan.plan.entryKey ===
                                                entry.entryKey
                                        }
                                        versionForceExpanded={
                                            versionExpandedKey ===
                                            entry.entryKey
                                        }
                                        onVersionPreview={() => {
                                            void runVersionPreview(
                                                entry.entryKey,
                                            );
                                        }}
                                        onVersionConfirm={() => {
                                            if (versionPlan.kind === "ready")
                                                void runVersionApply(
                                                    versionPlan.plan,
                                                );
                                        }}
                                        onVersionDismiss={() => {
                                            if (
                                                versionApply.kind === "applying"
                                            )
                                                return;
                                            setVersionPlan({ kind: "idle" });
                                            setVersionExpandedKey(null);
                                        }}
                                        onDeleted={() => {
                                            setPreview({ kind: "idle" });
                                            setVersionPlan({ kind: "idle" });
                                            setVersionApply({ kind: "idle" });
                                            setVersionExpandedKey(null);
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
                        {list.nextCursor !== null && (
                            <div className={card["actions"]}>
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
                            </div>
                        )}
                    </>
                )}
                {preview.kind === "resolutionError" && (
                    <p className={styles["error"]} role="alert">
                        {t(`agents.resolution.errors.${preview.code}`)}
                    </p>
                )}
            </DetailPanel>
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
            {apply.kind === "resolutionError" && (
                <p className={styles["error"]} role="alert">
                    {t(`agents.resolution.errors.${apply.code}`)}
                </p>
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
        </>
    );
}

/**
 * Page-level pin for the live configuration: label plus managed file paths.
 *
 * Match status and per-file readable labels stay out of this strip; history rows
 * still carry their own current-version mark when a retained entry matches.
 * Provider/model live on the Write segment instead. 「详细配置」 reads the live
 * files directly, so it stays available after an external edit no longer
 * matches a retained version; only an unreadable file disables it.
 */
export function CurrentConfigPin({
    current,
    agent = null,
}: {
    current: HistoryCurrentConfig;
    agent?: ConfigurableAgent | null;
}): JSX.Element {
    const { t } = useTranslation();
    const label = t("agents.restore.current.title");
    const [open, setOpen] = useState(false);
    const trigger = useRef<HTMLButtonElement | null>(null);
    const readablePaths = current.files
        .filter((file) => file.readable)
        .map((file) => file.targetPath);
    const canOpen = agent !== null && readablePaths.length > 0;
    return (
        <div className={restore["currentPin"]} role="region" aria-label={label}>
            <div className={restore["currentPinRow"]}>
                {current.files.length === 0 ? (
                    <span className={restore["currentMark"]}>{label}</span>
                ) : (
                    <div className={restore["currentPinBody"]}>
                        <span className={restore["currentMark"]}>{label}</span>
                        {current.files.map((file) => (
                            <span
                                key={file.targetPath}
                                className={[
                                    card["path"],
                                    restore["currentPath"],
                                ].join(" ")}
                            >
                                {file.targetPath}
                            </span>
                        ))}
                    </div>
                )}
                <button
                    ref={trigger}
                    type="button"
                    className={[
                        buttons["secondary"],
                        restore["currentDetails"],
                    ].join(" ")}
                    aria-disabled={!canOpen}
                    onClick={() => {
                        if (!canOpen) return;
                        setOpen(true);
                    }}
                >
                    {t("agents.version.textTitle")}
                </button>
            </div>
            {open && agent !== null && readablePaths.length > 0 && (
                <CurrentConfigurationDialog
                    agent={agent}
                    files={readablePaths}
                    onClose={() => {
                        setOpen(false);
                    }}
                    returnFocus={() =>
                        trigger.current?.isConnected ? trigger.current : null
                    }
                />
            )}
        </div>
    );
}

/**
 * Concise `{providerLabel} | {modelLabel}` line at the top of the Write segment.
 *
 * Both sides must resolve from HistoryCurrentConfig; otherwise hide the row.
 * Never paints dual「未写入」or empty-shell dashes.
 */
export function WriteCurrentSummary({
    providerLabel,
    modelLabel,
}: {
    providerLabel: string | null;
    modelLabel: string | null;
}): JSX.Element | null {
    const { t } = useTranslation();
    if (providerLabel === null || modelLabel === null) return null;
    return (
        <div className={restore["writeSummary"]} role="status">
            {t("agents.summary.providerModel", {
                provider: providerLabel,
                model: modelLabel,
            })}
        </div>
    );
}

/** One journal entry: compact summary by default; version plan expands inline. */
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
    versionPlan,
    versionRestoring,
    versionForceExpanded,
    onVersionPreview,
    onVersionConfirm,
    onVersionDismiss,
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
    /** This row's written-version preview is being built. */
    previewing: boolean;
    onPreview: () => void;
    inspecting: boolean;
    onInspect: () => void;
    /** Inline version plan for this row, when reviewed. */
    versionPlan: VersionRestorePlan | null;
    versionRestoring: boolean;
    /** Parent forces expand while a version plan is open on this row. */
    versionForceExpanded: boolean;
    onVersionPreview: () => void;
    onVersionConfirm: () => void;
    onVersionDismiss: () => void;
    onDeleted: () => void;
    onDeleteBusy: (busy: boolean) => void;
}): JSX.Element {
    const { t } = useTranslation();
    const expandedBodyId = useId();
    const [expanded, setExpanded] = useState(false);
    const showExpand =
        (entry.requiresResolution && expanded) ||
        versionForceExpanded ||
        versionPlan !== null;
    const state = entryState(entry);
    const fileCount = entry.sourceSteps.length;
    const canExpand = fileCount > 0 || entry.revisions.length > 0;

    /** Full collapse: clear local expand and any parent plan / force-expand. */
    function collapseRow(): void {
        setExpanded(false);
        if (versionForceExpanded || versionPlan !== null) {
            onVersionDismiss();
        }
    }

    function toggleExpand(): void {
        if (!canExpand) return;
        if (showExpand) {
            collapseRow();
            return;
        }
        if (entry.requiresResolution) {
            setExpanded(true);
        } else {
            onVersionPreview();
        }
    }
    const deleteBlock = historyDeleteBlockReason(entry);
    const providerText =
        entry.ownerLabel !== null
            ? entry.ownerLabel
            : entry.ownerRef !== null
              ? t("agents.restore.providerRemoved")
              : null;

    return (
        <li className={restore["row"]}>
            <div className={restore["rowSummary"]}>
                {canExpand ? (
                    <div className={restore["rowToggle"]}>
                        <IconButton
                            icon="chevron"
                            borderless
                            label={
                                showExpand
                                    ? t("agents.restore.collapseRow")
                                    : t("agents.restore.expandRow")
                            }
                            expanded={showExpand}
                            controls={expandedBodyId}
                            onClick={toggleExpand}
                        />
                        <span className={restore["rowMain"]}>
                            <span className={restore["state"]}>
                                {t(`agents.restore.intents.${entry.intent}`)}
                                {" · "}
                                {entryStateText(state, t)}
                                {isCurrent && (
                                    <>
                                        {" · "}
                                        <span
                                            className={restore["currentMark"]}
                                        >
                                            {t("agents.restore.current.mark")}
                                        </span>
                                    </>
                                )}
                            </span>
                            {providerText !== null && (
                                <span className={restore["meta"]}>
                                    {providerText}
                                </span>
                            )}
                            <span className={restore["time"]}>
                                {formatTime(entry.createdAt, locale)}
                            </span>
                            {fileCount > 0 && (
                                <span className={restore["meta"]}>
                                    {t("agents.restore.fileCount", {
                                        count: fileCount,
                                    })}
                                </span>
                            )}
                        </span>
                    </div>
                ) : (
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
                            <span className={restore["meta"]}>
                                {providerText}
                            </span>
                        )}
                        <span className={restore["time"]}>
                            {formatTime(entry.createdAt, locale)}
                        </span>
                    </div>
                )}
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
                    ) : entry.revisions.length > 0 ? null : entry.intent ===
                      "apply" ? (
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
            {showExpand && (
                <div id={expandedBodyId} className={restore["expandedBody"]}>
                    {entry.requiresResolution && (
                        <p className={styles["hint"]}>
                            {t("agents.resolution.required")}
                        </p>
                    )}
                    {versionPlan !== null && (
                        <VersionRestorePlanInline
                            plan={versionPlan}
                            restoring={versionRestoring}
                            onConfirm={onVersionConfirm}
                            onDismiss={collapseRow}
                        />
                    )}
                </div>
            )}
        </li>
    );
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

type Translator = TFunction;

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
