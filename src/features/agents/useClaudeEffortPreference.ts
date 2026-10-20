import { useEffect, useRef, useState } from "react";
import {
    ClaudePreferenceError,
    getClaudeEffortPreference,
    saveClaudeEffortPreference,
    type ClaudeEffortPreference,
    type ClaudePreferenceErrorCode,
} from "../../lib/desktop/claudePreference";
import type { ClaudeEffortLevel } from "../../lib/desktop/agents";

type Draft = {
    preference: ClaudeEffortPreference;
    draft: ClaudeEffortLevel | null;
};
/** Read and save are separate workflows: a failed save retains the user's draft. */
export type ClaudePreferenceState =
    | { kind: "inactive" }
    | { kind: "loading" }
    | { kind: "preview" }
    | { kind: "readError"; code: ClaudePreferenceErrorCode }
    | ({
          kind: "ready";
          outcome:
              | { kind: "idle" }
              | { kind: "saved" }
              | { kind: "error"; code: ClaudePreferenceErrorCode };
      } & Draft)
    | ({ kind: "saving" } & Draft);

/** A reusable starting draft, never a live source for an already-reviewed plan. */
export interface ClaudePreferenceEditor {
    state: ClaudePreferenceState;
    select: (value: ClaudeEffortLevel | null) => void;
    reload: () => void;
    save: () => Promise<void>;
}
function failure(error: unknown): ClaudePreferenceErrorCode {
    return error instanceof ClaudePreferenceError
        ? error.code
        : "operation_failed";
}

/** Synchronize one binding with private desktop storage.
 * Identity checks hide previous state immediately; generation checks ignore late
 * reads/saves after a switch, reload, unmount or StrictMode cleanup. The parent
 * serializes save/preview/apply so a remember action never writes Agent files.
 */
export function useClaudeEffortPreference(
    providerId: string | null,
    modelId: string | null,
): ClaudePreferenceEditor {
    const [answer, setAnswer] = useState<{
        providerId: string | null;
        modelId: string | null;
        state: ClaudePreferenceState;
    }>({ providerId, modelId, state: { kind: "loading" } });
    const sameBinding =
        answer.providerId === providerId && answer.modelId === modelId;
    // Reset this component's own state on identity changes, before committing a
    // frame. Otherwise A → B (still loading) → A could revive A's unsaved draft
    // while its new read is pending. No IPC or ref mutation happens in render.
    if (!sameBinding) {
        setAnswer({ providerId, modelId, state: { kind: "loading" } });
    }
    const [attempt, setAttempt] = useState(0);
    const generation = useRef(0);
    useEffect(() => {
        const current = ++generation.current;
        if (providerId === null || modelId === null) return;
        getClaudeEffortPreference({ providerId, modelId }).then(
            (result) => {
                if (generation.current !== current) return;
                setAnswer({
                    providerId,
                    modelId,
                    state:
                        result.kind === "preview"
                            ? { kind: "preview" }
                            : {
                                  kind: "ready",
                                  preference: result.preference,
                                  draft: result.preference.selectedEffort,
                                  outcome: { kind: "idle" },
                              },
                });
            },
            (error: unknown) => {
                if (generation.current === current)
                    setAnswer({
                        providerId,
                        modelId,
                        state: { kind: "readError", code: failure(error) },
                    });
            },
        );
        return () => {
            generation.current += 1;
        };
    }, [providerId, modelId, attempt]);
    const state: ClaudePreferenceState =
        providerId === null || modelId === null
            ? { kind: "inactive" }
            : sameBinding
              ? answer.state
              : { kind: "loading" };

    function select(value: ClaudeEffortLevel | null): void {
        if (providerId === null || modelId === null || state.kind !== "ready")
            return;
        setAnswer({
            providerId,
            modelId,
            state: { ...state, draft: value, outcome: { kind: "idle" } },
        });
    }
    function reload(): void {
        if (providerId === null || modelId === null || state.kind === "saving")
            return;
        // Invalidate an earlier request before React runs the replacement effect.
        generation.current += 1;
        setAnswer({ providerId, modelId, state: { kind: "loading" } });
        setAttempt((value) => value + 1);
    }
    async function save(): Promise<void> {
        if (
            providerId === null ||
            modelId === null ||
            state.kind !== "ready" ||
            state.draft === state.preference.selectedEffort
        )
            return;
        const current = generation.current;
        setAnswer({
            providerId,
            modelId,
            state: {
                kind: "saving",
                preference: state.preference,
                draft: state.draft,
            },
        });
        try {
            const saved = await saveClaudeEffortPreference({
                providerId,
                modelId,
                expectedModelDigest: state.preference.snapshot.contentDigest,
                expectedRevision: state.preference.revision,
                selectedEffort: state.draft,
            });
            if (generation.current !== current) return;
            setAnswer({
                providerId,
                modelId,
                state: {
                    kind: "ready",
                    preference: saved,
                    draft: saved.selectedEffort,
                    outcome: { kind: "saved" },
                },
            });
        } catch (error: unknown) {
            if (generation.current !== current) return;
            setAnswer({
                providerId,
                modelId,
                state: {
                    ...state,
                    outcome: { kind: "error", code: failure(error) },
                },
            });
        }
    }
    return { state, select, reload, save };
}
