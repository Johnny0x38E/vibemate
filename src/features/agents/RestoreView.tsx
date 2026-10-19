import { useEffect, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { DetailPanel } from "../../components/DetailForm";
import buttons from "../../components/actionButtons.module.css";
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
    type HistoryCursor,
    type HistoryEntry,
    type HistoryRevisionState,
} from "../../lib/desktop/history";
import styles from "./AgentList.module.css";
import card from "./ConfigPreview.module.css";
import restore from "./RestoreView.module.css";
import { ResolutionReviewCard } from "./ResolutionReviewCard";
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
}: {
    agent?: ConfigurableAgent;
    onBusyChange?: (busy: boolean) => void;
    blocked?: boolean;
}): JSX.Element {
    const { t, i18n } = useTranslation();
    const [list, setList] = useState<ListState>({ kind: "loading" });
    const [preview, setPreview] = useState<PreviewState>({ kind: "idle" });
    const [apply, setApply] = useState<ApplyState>({ kind: "idle" });
    const [attempt, setAttempt] = useState(0);
    const pending = useRef(false);
    const busyCallback = useRef(onBusyChange);
    useEffect(() => {
        // Keep cleanup and late async completions bound to the current parent.
        busyCallback.current = onBusyChange;
    }, [onBusyChange]);
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
     * Ask Rust what undoing one recorded write would do.
     *
     * Read-only: the preview reads each file's change token and compares it with
     * what the write left. Nothing is written and no credential is read.
     */
    async function runPreview(operationId: string): Promise<void> {
        if (blocked || pending.current || readingMore) return;
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
        readingMore ||
        preview.kind === "loading" ||
        preview.kind === "inspecting" ||
        apply.kind === "applying" ||
        apply.kind === "resolving";

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
                {list.kind === "ready" && list.entries.length === 0 && (
                    <p className={styles["hint"]}>
                        {t("agents.restore.empty")}
                    </p>
                )}
                {list.kind === "ready" && list.entries.length > 0 && (
                    <>
                        <ul className={restore["rows"]}>
                            {list.entries.map((entry) => (
                                <OperationRow
                                    key={entry.entryKey}
                                    entry={entry}
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
                                        void runInspection(entry.operationId);
                                    }}
                                />
                            ))}
                        </ul>
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
        </>
    );
}

/** One journal entry: when it ran, what it did, and its restore entry. */
function OperationRow({
    entry,
    locale,
    busy,
    previewing,
    onPreview,
    inspecting,
    onInspect,
}: {
    entry: HistoryEntry;
    /** Language used for the timestamp; the app language decides it. */
    locale: string;
    /** Another history read or restore action is in flight. */
    busy: boolean;
    /** This row's preview is being built. */
    previewing: boolean;
    onPreview: () => void;
    inspecting: boolean;
    onInspect: () => void;
}): JSX.Element {
    const { t } = useTranslation();
    const state = entryState(entry);
    return (
        <li className={restore["row"]}>
            <div className={restore["rowSummary"]}>
                <span className={restore["state"]}>
                    {t(`agents.restore.intents.${entry.intent}`)}
                    {" · "}
                    {entryStateText(state, t)}
                </span>
                <span className={restore["time"]}>
                    {formatTime(entry.createdAt, locale)}
                </span>
            </div>
            <ul className={restore["files"]}>
                {entryFiles(entry).map((file) => (
                    <li key={file.key} className={restore["file"]}>
                        <span className={card["path"]}>{file.targetPath}</span>
                        {file.note !== null && (
                            <span className={restore["stepState"]}>
                                {fileNoteText(file.note, t)}
                            </span>
                        )}
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
            {entry.requiresResolution ? (
                <>
                    <p className={styles["hint"]}>
                        {t("agents.resolution.required")}
                    </p>
                    <div className={card["actions"]}>
                        <button
                            type="button"
                            className={buttons["secondary"]}
                            aria-disabled={busy}
                            onClick={onInspect}
                        >
                            {inspecting
                                ? t("agents.resolution.inspecting")
                                : t("agents.resolution.inspect")}
                        </button>
                    </div>
                </>
            ) : entry.intent === "apply" ? (
                // The paginated reader never reads target files, so restore
                // eligibility is answered by the read-only preview. A write with
                // nothing left to undo reports that instead of hiding the row.
                <div className={card["actions"]}>
                    <button
                        type="button"
                        className={buttons["secondary"]}
                        aria-disabled={busy}
                        onClick={onPreview}
                    >
                        {previewing
                            ? t("agents.restore.previewing")
                            : t("agents.restore.preview")}
                    </button>
                </div>
            ) : null}
        </li>
    );
}

/** One file row, carrying either the legacy rollback or the batch result. */
type EntryFileNote =
    | { group: "stepStates"; state: RestoreState }
    | { group: "revisionStates"; state: HistoryRevisionState };

/** Files an entry changed, with the state that belongs to each row. */
function entryFiles(entry: HistoryEntry): {
    key: string;
    targetPath: string;
    note: EntryFileNote | null;
}[] {
    if (entry.intent === "apply") {
        return entry.sourceSteps.map((step) => ({
            key: `step:${String(step.ordinal)}`,
            targetPath: step.targetPath,
            note:
                step.restoreState === null
                    ? null
                    : { group: "stepStates", state: step.restoreState },
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

/** Locale-aware timestamp for one recorded write. */
function formatTime(ms: number, locale: string): string {
    return new Intl.DateTimeFormat(locale, {
        dateStyle: "medium",
        timeStyle: "short",
    }).format(new Date(ms));
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
