import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { useClaudeEffortPreference } from "./useClaudeEffortPreference";
import {
    ClaudePreferenceError,
    getClaudeEffortPreference,
    saveClaudeEffortPreference,
    type ClaudeEffortPreference,
} from "../../lib/desktop/claudePreference";
import {
    effortModelFixture,
    effortPreferenceFixture,
} from "../../test/claudePreference";

vi.mock(import("../../lib/desktop/claudePreference"), async (original) => ({
    ...(await original()),
    getClaudeEffortPreference: vi.fn(),
    saveClaudeEffortPreference: vi.fn(),
}));
const read = vi.mocked(getClaudeEffortPreference);
const remember = vi.mocked(saveClaudeEffortPreference);
const first = effortPreferenceFixture();
const A = first.snapshot.model.providerId;
const B = "f".repeat(32);
const modelId = first.snapshot.model.modelId;
function deferred<T>(): { promise: Promise<T>; resolve: (value: T) => void } {
    let resolve: (value: T) => void = () => {
        throw new Error("not initialized");
    };
    const promise = new Promise<T>((res) => {
        resolve = res;
    });
    return { promise, resolve };
}
beforeEach(() => {
    read.mockReset().mockImplementation(({ providerId, modelId }) =>
        Promise.resolve({
            kind: "desktop",
            preference: effortPreferenceFixture(
                effortModelFixture(providerId, modelId),
            ),
        }),
    );
    remember.mockReset();
});
afterEach(cleanup);

test("selecting a draft is not a save; remembering sends both snapshot tokens", async () => {
    remember.mockResolvedValue({
        ...first,
        selectedEffort: "max",
        revision: "a".repeat(32),
    });
    const { result } = renderHook(() => useClaudeEffortPreference(A, modelId));
    await waitFor(() => {
        expect(result.current.state).toMatchObject({
            kind: "ready",
            draft: null,
        });
    });
    act(() => {
        result.current.select("max");
    });
    expect(remember).not.toHaveBeenCalled();
    await act(async () => {
        await result.current.save();
    });
    expect(remember).toHaveBeenCalledWith({
        providerId: A,
        modelId,
        expectedModelDigest: first.snapshot.contentDigest,
        expectedRevision: null,
        selectedEffort: "max",
    });
    expect(result.current.state).toMatchObject({
        kind: "ready",
        draft: "max",
        outcome: { kind: "saved" },
    });
});

test("read failure remains an error rather than inherited defaults and has a reload path", async () => {
    read.mockRejectedValueOnce(new ClaudePreferenceError("read_failed"));
    const { result } = renderHook(() => useClaudeEffortPreference(A, modelId));
    await waitFor(() => {
        expect(result.current.state).toEqual({
            kind: "readError",
            code: "read_failed",
        });
    });
    act(() => {
        result.current.reload();
    });
    await waitFor(() => {
        expect(result.current.state).toMatchObject({
            kind: "ready",
            draft: null,
        });
    });
    expect(remember).not.toHaveBeenCalled();
});

test("a late preference read from another account never replaces the current starting choice", async () => {
    const delayed = deferred<{
        kind: "desktop";
        preference: ClaudeEffortPreference;
    }>();
    const other = {
        ...effortPreferenceFixture(effortModelFixture(B, modelId)),
        selectedEffort: "low" as const,
        revision: "b".repeat(32),
    };
    read.mockImplementation(({ providerId }) =>
        providerId === A
            ? delayed.promise
            : Promise.resolve({ kind: "desktop", preference: other }),
    );
    const { result, rerender } = renderHook(
        ({ providerId }) => useClaudeEffortPreference(providerId, modelId),
        { initialProps: { providerId: A } },
    );
    rerender({ providerId: B });
    await waitFor(() => {
        expect(result.current.state).toMatchObject({
            kind: "ready",
            draft: "low",
        });
    });
    await act(async () => {
        delayed.resolve({
            kind: "desktop",
            preference: {
                ...first,
                selectedEffort: "max",
                revision: "a".repeat(32),
            },
        });
        await delayed.promise;
    });
    expect(result.current.state).toMatchObject({
        kind: "ready",
        draft: "low",
        preference: { snapshot: { model: { providerId: B } } },
    });
});

test("a failed CAS retains the draft until the user explicitly reloads a newer preference", async () => {
    remember.mockRejectedValue(new ClaudePreferenceError("preference_changed"));
    const { result } = renderHook(() => useClaudeEffortPreference(A, modelId));
    await waitFor(() => {
        expect(result.current.state.kind).toBe("ready");
    });
    act(() => {
        result.current.select("max");
    });
    await act(async () => {
        await result.current.save();
    });
    expect(result.current.state).toMatchObject({
        kind: "ready",
        draft: "max",
        outcome: { kind: "error", code: "preference_changed" },
    });
    read.mockResolvedValue({
        kind: "desktop",
        preference: {
            ...first,
            selectedEffort: "low",
            revision: "b".repeat(32),
        },
    });
    act(() => {
        result.current.reload();
    });
    await waitFor(() => {
        expect(result.current.state).toMatchObject({
            kind: "ready",
            draft: "low",
            outcome: { kind: "idle" },
        });
    });
});

test("returning before another binding finishes reading never revives an unsaved old draft", async () => {
    const returning = deferred<{
        kind: "desktop";
        preference: ClaudeEffortPreference;
    }>();
    const other = deferred<{
        kind: "desktop";
        preference: ClaudeEffortPreference;
    }>();
    let firstReads = 0;
    read.mockImplementation(({ providerId }) => {
        if (providerId === B) return other.promise;
        firstReads += 1;
        return firstReads === 1
            ? Promise.resolve({ kind: "desktop", preference: first })
            : returning.promise;
    });
    const { result, rerender } = renderHook(
        ({ providerId }) => useClaudeEffortPreference(providerId, modelId),
        { initialProps: { providerId: A } },
    );
    await waitFor(() => {
        expect(result.current.state.kind).toBe("ready");
    });
    act(() => {
        result.current.select("max");
    });
    rerender({ providerId: B });
    expect(result.current.state.kind).toBe("loading");
    rerender({ providerId: A });
    expect(result.current.state.kind).toBe("loading");
    await act(async () => {
        returning.resolve({ kind: "desktop", preference: first });
        await returning.promise;
    });
    expect(result.current.state).toMatchObject({ kind: "ready", draft: null });
    await act(async () => {
        other.resolve({
            kind: "desktop",
            preference: effortPreferenceFixture(effortModelFixture(B, modelId)),
        });
        await other.promise;
    });
    expect(result.current.state).toMatchObject({
        kind: "ready",
        draft: null,
        preference: { snapshot: { model: { providerId: A } } },
    });
});

test("a late save completion is ignored after a binding change", async () => {
    const delayed = deferred<ClaudeEffortPreference>();
    remember.mockReturnValue(delayed.promise);
    const { result, rerender } = renderHook(
        ({ providerId }) => useClaudeEffortPreference(providerId, modelId),
        { initialProps: { providerId: A } },
    );
    await waitFor(() => {
        expect(result.current.state.kind).toBe("ready");
    });
    act(() => {
        result.current.select("max");
    });
    let saving = Promise.resolve();
    act(() => {
        saving = result.current.save();
    });
    expect(result.current.state.kind).toBe("saving");
    rerender({ providerId: B });
    await waitFor(() => {
        expect(result.current.state).toMatchObject({
            kind: "ready",
            draft: null,
        });
    });
    await act(async () => {
        delayed.resolve({
            ...first,
            selectedEffort: "max",
            revision: "a".repeat(32),
        });
        await saving;
    });
    expect(result.current.state).toMatchObject({
        kind: "ready",
        draft: null,
        preference: { snapshot: { model: { providerId: B } } },
    });
});
