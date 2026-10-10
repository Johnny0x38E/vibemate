import {
    useCallback,
    useEffect,
    useMemo,
    useRef,
    useState,
    type JSX,
    type ChangeEvent,
} from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { useNotify } from "../../components/Notifications";
import {
    browseUpstreamModelsPage,
    cancelProviderModelFetch,
    getProviderModelFetchStatus,
    listProviderModels,
    ModelRequestError,
    saveProviderModelSelections,
    type ModelErrorCode,
    type ProviderModel,
    type UpstreamBrowseModel,
} from "../../lib/desktop/models";
import { getProviderSecretStatus } from "../../lib/desktop/providerSecrets";
import { ModelCheckbox } from "../../components/ModelCheckbox";
import buttons from "./providerButtons.module.css";
import styles from "./ProviderModels.module.css";

/** Search results cap; fuzzy search returns at most this many rows per query. */
export const MODEL_PAGE_SIZE = 50;

/** Matches Rust `MAX_MODEL_PAGE_SIZE`; internal chunks when reading all selected rows. */
const SAVED_CATALOG_CHUNK = 200;

/** Selected rows for tab labels and quick filters. */
export interface SelectedModelCount {
    count: number;
    hasMore: boolean;
}

const SEARCH_DEBOUNCE_MS = 300;

type ViewMode = "selected" | "upstream";

type ListState =
    | { kind: "loading" }
    | { kind: "preview" }
    | { kind: "error"; code: ModelErrorCode }
    | {
          kind: "ready";
          items: ProviderModel[];
          totalMatches: number | null;
      };

type UpstreamListState =
    | { kind: "idle" }
    | { kind: "loading" }
    | { kind: "error"; code: ModelErrorCode }
    | { kind: "ready"; items: UpstreamBrowseModel[] };

type KeyState =
    | { kind: "loading" }
    | { kind: "preview" }
    | { kind: "error"; code: ModelErrorCode }
    | {
          kind: "ready";
          configured: boolean;
      };

type SaveState =
    | { kind: "idle" }
    | { kind: "running" }
    | { kind: "error"; code: ModelErrorCode };

/** Inputs for {@link ProviderModels}. */
export interface ProviderModelsProps {
    providerId: string;
    hidden?: boolean;
    layout?: "section" | "tabPanel";
    onBusyChange?: (busy: boolean) => void;
    onDirtyChange?: (dirty: boolean) => void;
    onSelectedCountChange?: (count: SelectedModelCount | null) => void;
}

function errorCode(error: unknown): ModelErrorCode {
    if (error instanceof ModelRequestError) return error.code;
    return "operation_failed";
}

function mergeModels(
    existing: ProviderModel[],
    incoming: ProviderModel[],
): ProviderModel[] {
    const seen = new Set(existing.map((model) => model.modelId));
    const appended = incoming.filter((model) => !seen.has(model.modelId));
    return [...existing, ...appended];
}

function mergeUpstream(
    existing: UpstreamBrowseModel[],
    incoming: UpstreamBrowseModel[],
): UpstreamBrowseModel[] {
    const seen = new Set(existing.map((model) => model.modelId));
    const appended = incoming.filter((model) => !seen.has(model.modelId));
    return [...existing, ...appended];
}

function setsEqual(a: ReadonlySet<string>, b: ReadonlySet<string>): boolean {
    if (a.size !== b.size) return false;
    for (const id of a) {
        if (!b.has(id)) return false;
    }
    return true;
}

type SelectedCatalogLoad =
    | { kind: "preview" }
    | { kind: "desktop"; items: ProviderModel[]; ids: Set<string> };

/** Read every persisted selected row (paged in Rust, one list in the UI). */
async function listAllSelected(
    providerId: string,
): Promise<SelectedCatalogLoad> {
    let after: string | null = null;
    let items: ProviderModel[] = [];
    for (;;) {
        const chunk = await listProviderModels({
            providerId,
            after,
            limit: SAVED_CATALOG_CHUNK,
            filter: "selected",
        });
        if (chunk.kind === "preview") {
            return { kind: "preview" };
        }
        items = mergeModels(items, chunk.page.items);
        after = chunk.page.nextCursor;
        if (after === null) break;
    }
    return {
        kind: "desktop",
        items,
        ids: new Set(items.map((model) => model.modelId)),
    };
}

function selectionBlocked(
    routeSupport: ProviderModel["routeSupport"],
    source?: ProviderModel["source"],
): boolean {
    if (source === "manual") return false;
    return routeSupport === "unsupported";
}

function displayTitle(model: {
    modelId: string;
    alias?: string | null;
    upstreamName?: string | null;
}): string {
    const trimmed = model.alias?.trim();
    if (trimmed) return trimmed;
    if (model.upstreamName?.trim()) return model.upstreamName.trim();
    return model.modelId;
}

function formatMissingDate(ms: number, locale: string): string {
    return new Intl.DateTimeFormat(locale, { dateStyle: "medium" }).format(
        new Date(ms),
    );
}

/** One centered footer line under the list (selected end, upstream scroll/load/end). */
function resolveEndOfListMessage(
    t: TFunction,
    viewMode: ViewMode,
    list: ListState,
    debouncedQuery: string,
    upstream: UpstreamListState,
    upstreamMorePages: boolean,
    browseActive: boolean,
): string | null {
    if (viewMode === "selected") {
        if (
            list.kind !== "ready" ||
            debouncedQuery !== "" ||
            list.items.length === 0
        ) {
            return null;
        }
        return t("providers.models.allSelectedLoaded");
    }
    if (upstream.kind !== "ready" || upstream.items.length === 0) {
        return null;
    }
    if (upstreamMorePages) {
        if (browseActive) {
            return t("providers.models.upstreamLoadingMore");
        }
        return t("providers.models.upstreamScrollForMore");
    }
    return t("providers.models.upstreamAllLoaded");
}

/**
 * Models tab (P12.c.5): selected list by default, upstream browse via Fetch models,
 * draft checkboxes until Save.
 */
export function ProviderModels({
    providerId,
    hidden = false,
    layout = "section",
    onBusyChange,
    onDirtyChange,
    onSelectedCountChange,
}: ProviderModelsProps): JSX.Element {
    const { t, i18n } = useTranslation();
    const notify = useNotify();
    const [viewMode, setViewMode] = useState<ViewMode>("selected");
    const [searchInput, setSearchInput] = useState("");
    const [debouncedQuery, setDebouncedQuery] = useState("");
    const [list, setList] = useState<ListState>({ kind: "loading" });
    const [upstream, setUpstream] = useState<UpstreamListState>({
        kind: "idle",
    });
    const [upstreamNextOffset, setUpstreamNextOffset] = useState<number | null>(
        null,
    );
    const [upstreamMorePages, setUpstreamMorePages] = useState(false);
    const [baselineSelected, setBaselineSelected] = useState<
        ReadonlySet<string>
    >(() => new Set());
    const [draftSelected, setDraftSelected] = useState<ReadonlySet<string>>(
        () => new Set(),
    );
    const [keyState, setKeyState] = useState<KeyState>({ kind: "loading" });
    const [browseRunning, setBrowseRunning] = useState(false);
    const [serverBrowseRunning, setServerBrowseRunning] = useState(false);
    const [saveState, setSaveState] = useState<SaveState>({ kind: "idle" });

    const loadGeneration = useRef(0);
    const browseGeneration = useRef(0);
    const listScrollRef = useRef<HTMLDivElement>(null);
    const loadMoreSentinelRef = useRef<HTMLDivElement>(null);
    const upstreamLoadingMore = useRef(false);
    /** Last upstream search query we already started a full reload for. */
    const upstreamSearchQueryRef = useRef(debouncedQuery);

    const dirty = useMemo(
        () => !setsEqual(baselineSelected, draftSelected),
        [baselineSelected, draftSelected],
    );
    const dirtyRef = useRef(dirty);
    useEffect(() => {
        dirtyRef.current = dirty;
    }, [dirty]);

    const refreshSelectedCount = useCallback(async (): Promise<void> => {
        if (hidden) return;
        try {
            const loaded = await listAllSelected(providerId);
            if (loaded.kind === "preview") {
                onSelectedCountChange?.(null);
                return;
            }
            const next: SelectedModelCount = {
                count: loaded.ids.size,
                hasMore: false,
            };
            onSelectedCountChange?.(next);
        } catch {
            onSelectedCountChange?.(null);
        }
    }, [providerId, hidden, onSelectedCountChange]);

    useEffect(() => {
        const handle = window.setTimeout(() => {
            setDebouncedQuery(searchInput.trim());
        }, SEARCH_DEBOUNCE_MS);
        return () => {
            clearTimeout(handle);
        };
    }, [searchInput]);

    useEffect(() => {
        onDirtyChange?.(!hidden && dirty);
    }, [onDirtyChange, dirty, hidden]);

    const loadKeyStatus = useCallback(async (): Promise<void> => {
        try {
            const result = await getProviderSecretStatus(providerId);
            if (hidden) return;
            if (result.kind === "preview") {
                setKeyState({ kind: "preview" });
                return;
            }
            setKeyState({
                kind: "ready",
                configured: result.status.state === "set",
            });
        } catch {
            if (hidden) return;
            setKeyState({ kind: "ready", configured: false });
        }
    }, [providerId, hidden]);

    const syncBaselineFromLoaded = useCallback((ids: ReadonlySet<string>) => {
        setBaselineSelected(ids);
        setDraftSelected(new Set(ids));
    }, []);

    const loadSelectedList = useCallback(async (): Promise<void> => {
        const generation = ++loadGeneration.current;
        setList({ kind: "loading" });
        try {
            if (debouncedQuery) {
                const result = await listProviderModels({
                    providerId,
                    after: null,
                    limit: MODEL_PAGE_SIZE,
                    filter: "selected",
                    query: debouncedQuery,
                });
                if (hidden || generation !== loadGeneration.current) return;
                if (result.kind === "preview") {
                    setList({ kind: "preview" });
                    return;
                }
                setList({
                    kind: "ready",
                    items: result.page.items,
                    totalMatches: result.page.totalMatches,
                });
                return;
            }
            const loaded = await listAllSelected(providerId);
            if (hidden || generation !== loadGeneration.current) return;
            if (loaded.kind === "preview") {
                setList({ kind: "preview" });
                return;
            }
            if (!dirtyRef.current) {
                syncBaselineFromLoaded(loaded.ids);
            }
            setList({
                kind: "ready",
                items: loaded.items,
                totalMatches: null,
            });
            void refreshSelectedCount();
        } catch (error: unknown) {
            if (hidden || generation !== loadGeneration.current) return;
            setList({ kind: "error", code: errorCode(error) });
        }
    }, [
        providerId,
        debouncedQuery,
        hidden,
        refreshSelectedCount,
        syncBaselineFromLoaded,
    ]);

    const refreshBrowseRunning = useCallback(async (): Promise<void> => {
        try {
            const result = await getProviderModelFetchStatus(providerId);
            if (hidden) return;
            if (result.kind === "desktop") {
                setServerBrowseRunning(result.status.running);
            }
        } catch {
            if (hidden) return;
            setServerBrowseRunning(false);
        }
    }, [providerId, hidden]);

    const loadUpstreamPage = useCallback(
        async (offset: number | null, append: boolean): Promise<void> => {
            const generation = ++browseGeneration.current;
            const query =
                debouncedQuery.length > 0 ? debouncedQuery : undefined;
            if (!append) {
                setUpstream({ kind: "loading" });
            }
            setBrowseRunning(true);
            try {
                const page = await browseUpstreamModelsPage({
                    providerId,
                    offset,
                    ...(query !== undefined ? { query } : {}),
                });
                if (hidden || generation !== browseGeneration.current) return;
                setUpstream((current) => {
                    const previous =
                        append && current.kind === "ready" ? current.items : [];
                    return {
                        kind: "ready",
                        items: mergeUpstream(previous, page.items),
                    };
                });
                setUpstreamNextOffset(page.nextOffset);
                setUpstreamMorePages(page.morePages);
            } catch (error: unknown) {
                if (hidden || generation !== browseGeneration.current) return;
                if (!append) {
                    setUpstream({ kind: "error", code: errorCode(error) });
                } else {
                    notify({
                        tone: "error",
                        message: t(
                            `providers.models.errors.${errorCode(error)}`,
                        ),
                    });
                }
            } finally {
                if (generation === browseGeneration.current) {
                    setBrowseRunning(false);
                    void refreshBrowseRunning();
                }
            }
        },
        [providerId, hidden, notify, t, refreshBrowseRunning, debouncedQuery],
    );

    useEffect(() => {
        if (hidden || viewMode !== "upstream") return;
        if (upstream.kind === "idle") {
            upstreamSearchQueryRef.current = debouncedQuery;
            return;
        }
        if (upstreamSearchQueryRef.current === debouncedQuery) return;
        upstreamSearchQueryRef.current = debouncedQuery;
        setUpstreamNextOffset(null);
        setUpstreamMorePages(false);
        const handle = window.setTimeout(() => {
            void loadUpstreamPage(null, false);
        }, 0);
        return () => {
            clearTimeout(handle);
        };
    }, [hidden, viewMode, debouncedQuery, upstream.kind, loadUpstreamPage]);

    useEffect(() => {
        if (hidden) {
            loadGeneration.current += 1;
            browseGeneration.current += 1;
            void cancelProviderModelFetch(providerId);
            return;
        }
        const handle = window.setTimeout(() => {
            void loadKeyStatus();
            void refreshSelectedCount();
            void refreshBrowseRunning();
        }, 0);
        return () => {
            clearTimeout(handle);
        };
    }, [
        hidden,
        providerId,
        loadKeyStatus,
        refreshSelectedCount,
        refreshBrowseRunning,
    ]);

    useEffect(() => {
        if (hidden || viewMode !== "selected") return;
        // Defer so the effect body does not synchronously enter list loading state.
        const handle = window.setTimeout(() => {
            void loadSelectedList();
        }, 0);
        return () => {
            clearTimeout(handle);
        };
    }, [hidden, viewMode, loadSelectedList]);

    useEffect(() => {
        if (
            hidden ||
            viewMode !== "upstream" ||
            upstream.kind !== "ready" ||
            !upstreamMorePages ||
            upstreamNextOffset === null
        ) {
            return;
        }
        const root = listScrollRef.current;
        const sentinel = loadMoreSentinelRef.current;
        if (root === null || sentinel === null) return;

        const observer = new IntersectionObserver(
            (entries) => {
                const visible = entries.some((entry) => entry.isIntersecting);
                if (
                    !visible ||
                    upstreamLoadingMore.current ||
                    browseRunning ||
                    serverBrowseRunning
                ) {
                    return;
                }
                upstreamLoadingMore.current = true;
                void loadUpstreamPage(upstreamNextOffset, true).finally(() => {
                    upstreamLoadingMore.current = false;
                });
            },
            { root, rootMargin: "120px" },
        );
        observer.observe(sentinel);
        return () => {
            observer.disconnect();
        };
    }, [
        hidden,
        viewMode,
        upstream,
        upstreamMorePages,
        upstreamNextOffset,
        browseRunning,
        serverBrowseRunning,
        loadUpstreamPage,
    ]);

    async function startUpstreamBrowse(): Promise<void> {
        if (browseRunning || serverBrowseRunning) return;
        const resetDraft = new Set(baselineSelected);
        setDraftSelected(resetDraft);
        setViewMode("upstream");
        setSearchInput("");
        setDebouncedQuery("");
        setUpstreamNextOffset(null);
        setUpstreamMorePages(false);
        upstreamSearchQueryRef.current = "";
        await loadUpstreamPage(null, false);
    }

    async function runCancelBrowse(): Promise<void> {
        try {
            await cancelProviderModelFetch(providerId);
        } catch {
            // Best-effort; status refresh shows the truth.
        }
        setBrowseRunning(false);
        await refreshBrowseRunning();
    }

    function toggleDraft(modelId: string, nextSelected: boolean): void {
        setDraftSelected((current) => {
            const next = new Set(current);
            if (nextSelected) next.add(modelId);
            else next.delete(modelId);
            return next;
        });
    }

    async function runSave(): Promise<void> {
        if (!dirty || saveState.kind === "running") return;
        setSaveState({ kind: "running" });
        const upstreamById = new Map<string, UpstreamBrowseModel>();
        if (upstream.kind === "ready") {
            for (const row of upstream.items) {
                upstreamById.set(row.modelId, row);
            }
        }
        const removeModelIds: string[] = [];
        for (const id of baselineSelected) {
            if (!draftSelected.has(id)) removeModelIds.push(id);
        }
        const add = [];
        for (const id of draftSelected) {
            if (baselineSelected.has(id)) continue;
            const row = upstreamById.get(id);
            add.push({
                modelId: id,
                upstreamName: row?.upstreamName ?? null,
                contextWindow: row?.contextWindow ?? null,
                maxOutputTokens: row?.maxOutputTokens ?? null,
                inputModalities: row?.inputModalities ?? null,
                outputModalities: row?.outputModalities ?? null,
                supportedEndpoints: row?.supportedEndpoints ?? null,
            });
        }
        try {
            await saveProviderModelSelections({
                providerId,
                removeModelIds,
                add,
            });
            setSaveState({ kind: "idle" });
            const nextBaseline = new Set(draftSelected);
            setBaselineSelected(nextBaseline);
            notify({
                tone: "success",
                message: t("providers.models.saveNotice"),
            });
            if (viewMode === "upstream") {
                setViewMode("selected");
                setUpstream({ kind: "idle" });
            }
            await loadSelectedList();
            await refreshSelectedCount();
        } catch (error: unknown) {
            setSaveState({ kind: "error", code: errorCode(error) });
        }
    }

    function onSearchChange(event: ChangeEvent<HTMLInputElement>): void {
        setSearchInput(event.currentTarget.value);
    }

    function confirmDiscardDraft(): boolean {
        if (!dirty) return true;
        return window.confirm(t("providers.models.unsavedLeave"));
    }

    const browseActive = browseRunning || serverBrowseRunning;
    const keyMissing = keyState.kind === "ready" && !keyState.configured;

    const upstreamItems = upstream.kind === "ready" ? upstream.items : [];

    const searchSummary =
        viewMode === "selected" &&
        list.kind === "ready" &&
        debouncedQuery !== "" &&
        list.totalMatches !== null
            ? t("providers.models.searchSummary", {
                  total: list.totalMatches,
                  shown: list.items.length,
              })
            : viewMode === "upstream" &&
                debouncedQuery !== "" &&
                upstream.kind === "ready"
              ? t("providers.models.searchSummaryUpstream", {
                    shown: upstreamItems.length,
                })
              : null;

    const sectionBusy =
        browseActive ||
        saveState.kind === "running" ||
        (viewMode === "selected" && list.kind === "loading") ||
        (viewMode === "upstream" && upstream.kind === "loading");

    useEffect(() => {
        onBusyChange?.(!hidden && sectionBusy);
    }, [onBusyChange, sectionBusy, hidden]);

    const emptyMessage = ((): string => {
        if (keyMissing) return t("providers.models.emptyNeedKey");
        if (viewMode === "selected") {
            if (debouncedQuery !== "")
                return t("providers.models.emptyFiltered");
            return t("providers.models.emptyNeedFetch");
        }
        if (upstream.kind === "error")
            return t("providers.models.emptyUpstream");
        return t("providers.models.emptyUpstream");
    })();

    const listHeadingId = "provider-models-list-heading";
    const listTitle =
        viewMode === "selected"
            ? t("providers.models.filter.selected")
            : t("providers.models.filter.all");
    const showListShell = list.kind !== "preview";

    const endOfListMessage = resolveEndOfListMessage(
        t,
        viewMode,
        list,
        debouncedQuery,
        upstream,
        upstreamMorePages,
        browseActive,
    );

    const listBody = (
        <>
            {viewMode === "selected" &&
                list.kind === "loading" &&
                !browseActive && (
                    <p className={styles["message"]} role="status">
                        {t("providers.models.loading")}
                    </p>
                )}

            {viewMode === "selected" && list.kind === "preview" && (
                <p className={styles["message"]} role="status">
                    {t("providers.models.preview")}
                </p>
            )}

            {viewMode === "selected" && list.kind === "error" && (
                <>
                    <p className={styles["error"]} role="alert">
                        {t(`providers.models.errors.${list.code}`)}
                    </p>
                    <button
                        type="button"
                        className={buttons["secondary"]}
                        onClick={() => {
                            void loadSelectedList();
                        }}
                    >
                        {t("providers.models.retry")}
                    </button>
                </>
            )}

            {viewMode === "upstream" && upstream.kind === "loading" && (
                <p className={styles["message"]} role="status">
                    {t("providers.models.upstreamLoading")}
                </p>
            )}

            {viewMode === "upstream" && upstream.kind === "error" && (
                <>
                    <p className={styles["error"]} role="alert">
                        {t(`providers.models.errors.${upstream.code}`)}
                    </p>
                    <button
                        type="button"
                        className={buttons["secondary"]}
                        onClick={() => {
                            void loadUpstreamPage(null, false);
                        }}
                    >
                        {t("providers.models.retry")}
                    </button>
                </>
            )}

            {((viewMode === "selected" &&
                list.kind === "ready" &&
                list.items.length === 0) ||
                (viewMode === "upstream" &&
                    upstream.kind === "ready" &&
                    upstreamItems.length === 0 &&
                    !browseActive)) && (
                <p className={styles["empty"]} role="status">
                    {emptyMessage}
                </p>
            )}

            {viewMode === "selected" &&
                list.kind === "ready" &&
                list.items.length > 0 && (
                    <ul
                        className={styles["list"]}
                        aria-labelledby={listHeadingId}
                    >
                        {list.items.map((model) => {
                            const checked = draftSelected.has(model.modelId);
                            const blocked = selectionBlocked(
                                model.routeSupport,
                                model.source,
                            );
                            return (
                                <li
                                    key={model.modelId}
                                    className={styles["item"]}
                                >
                                    <ModelCheckbox
                                        checked={checked}
                                        disabled={sectionBusy || blocked}
                                        ariaLabel={
                                            checked
                                                ? t(
                                                      "providers.models.deselectModel",
                                                      {
                                                          modelId:
                                                              model.modelId,
                                                      },
                                                  )
                                                : t(
                                                      "providers.models.selectModel",
                                                      {
                                                          modelId:
                                                              model.modelId,
                                                      },
                                                  )
                                        }
                                        onCheckedChange={(nextSelected) => {
                                            if (blocked) return;
                                            toggleDraft(
                                                model.modelId,
                                                nextSelected,
                                            );
                                        }}
                                    />
                                    <div className={styles["body"]}>
                                        <p className={styles["line"]}>
                                            <span
                                                className={styles["lineText"]}
                                            >
                                                <span
                                                    className={styles["name"]}
                                                >
                                                    {displayTitle(model)}
                                                </span>
                                                {displayTitle(model) !==
                                                    model.modelId && (
                                                    <span
                                                        className={
                                                            styles["modelId"]
                                                        }
                                                    >
                                                        {" · "}
                                                        {model.modelId}
                                                    </span>
                                                )}
                                                {model.source === "manual" && (
                                                    <span
                                                        className={
                                                            styles["manualTag"]
                                                        }
                                                    >
                                                        {" · "}
                                                        {t(
                                                            "providers.models.manualSource",
                                                        )}
                                                    </span>
                                                )}
                                            </span>
                                            {model.routeSupport ===
                                                "unsupported" && (
                                                <span
                                                    className={styles["badge"]}
                                                >
                                                    {t(
                                                        "providers.models.badges.routeUnsupported",
                                                    )}
                                                </span>
                                            )}
                                            {model.routeSupport ===
                                                "unknown" && (
                                                <span
                                                    className={styles["badge"]}
                                                >
                                                    {t(
                                                        "providers.models.badges.routeUnknown",
                                                    )}
                                                </span>
                                            )}
                                        </p>
                                        {model.upstreamState === "missing" &&
                                            model.missingSinceMs !== null && (
                                                <div
                                                    className={styles["badges"]}
                                                >
                                                    <span
                                                        className={
                                                            styles["badge"]
                                                        }
                                                    >
                                                        {t(
                                                            "providers.models.badges.upstreamMissing",
                                                            {
                                                                date: formatMissingDate(
                                                                    model.missingSinceMs,
                                                                    i18n.language,
                                                                ),
                                                            },
                                                        )}
                                                    </span>
                                                </div>
                                            )}
                                    </div>
                                </li>
                            );
                        })}
                    </ul>
                )}

            {viewMode === "upstream" &&
                upstream.kind === "ready" &&
                upstreamItems.length > 0 && (
                    <ul
                        className={styles["list"]}
                        aria-labelledby={listHeadingId}
                    >
                        {upstreamItems.map((model) => {
                            const checked = draftSelected.has(model.modelId);
                            const blocked = selectionBlocked(
                                model.routeSupport,
                            );
                            return (
                                <li
                                    key={model.modelId}
                                    className={styles["item"]}
                                >
                                    <ModelCheckbox
                                        checked={checked}
                                        disabled={sectionBusy || blocked}
                                        ariaLabel={
                                            checked
                                                ? t(
                                                      "providers.models.deselectModel",
                                                      {
                                                          modelId:
                                                              model.modelId,
                                                      },
                                                  )
                                                : t(
                                                      "providers.models.selectModel",
                                                      {
                                                          modelId:
                                                              model.modelId,
                                                      },
                                                  )
                                        }
                                        onCheckedChange={(nextSelected) => {
                                            if (blocked) return;
                                            toggleDraft(
                                                model.modelId,
                                                nextSelected,
                                            );
                                        }}
                                    />
                                    <div className={styles["body"]}>
                                        <p className={styles["line"]}>
                                            <span
                                                className={styles["lineText"]}
                                            >
                                                <span
                                                    className={styles["name"]}
                                                >
                                                    {displayTitle(model)}
                                                </span>
                                                {displayTitle(model) !==
                                                    model.modelId && (
                                                    <span
                                                        className={
                                                            styles["modelId"]
                                                        }
                                                    >
                                                        {" · "}
                                                        {model.modelId}
                                                    </span>
                                                )}
                                            </span>
                                            {model.routeSupport ===
                                                "unsupported" && (
                                                <span
                                                    className={styles["badge"]}
                                                >
                                                    {t(
                                                        "providers.models.badges.routeUnsupported",
                                                    )}
                                                </span>
                                            )}
                                        </p>
                                    </div>
                                </li>
                            );
                        })}
                    </ul>
                )}

            {viewMode === "upstream" && upstreamMorePages && (
                <div
                    ref={loadMoreSentinelRef}
                    className={styles["loadSentinel"]}
                    aria-hidden
                />
            )}

            {endOfListMessage !== null && (
                <p
                    className={styles["listFooter"]}
                    role="status"
                    aria-live="polite"
                >
                    {endOfListMessage}
                </p>
            )}
        </>
    );

    const sectionLabel =
        layout === "section" ? "provider-models-heading" : undefined;

    return (
        <section
            className={styles["section"]}
            data-layout={layout}
            aria-labelledby={sectionLabel}
            aria-label={
                layout === "tabPanel" ? t("providers.tabs.models") : undefined
            }
            aria-busy={sectionBusy}
        >
            {layout === "section" && (
                <h2 className={styles["title"]} id="provider-models-heading">
                    {t("providers.groups.models")}
                </h2>
            )}

            {saveState.kind === "error" && (
                <p className={styles["error"]} role="alert">
                    {t(`providers.models.errors.${saveState.code}`)}
                </p>
            )}

            {keyMissing && (
                <p className={styles["message"]} role="status">
                    {t("providers.models.needKey")}
                </p>
            )}

            <div className={styles["toolbarRow"]}>
                <div className={styles["searchField"]}>
                    <input
                        className={styles["searchInput"]}
                        id="provider-models-search"
                        type="search"
                        value={searchInput}
                        autoComplete="off"
                        spellCheck={false}
                        placeholder={t("providers.models.searchPlaceholder")}
                        aria-label={t("providers.models.searchPlaceholder")}
                        disabled={
                            list.kind === "preview" ||
                            keyMissing ||
                            keyState.kind === "loading" ||
                            (viewMode === "selected" &&
                                list.kind !== "ready" &&
                                list.kind !== "loading") ||
                            (viewMode === "upstream" &&
                                upstream.kind === "idle")
                        }
                        onChange={onSearchChange}
                    />
                </div>
                <div className={styles["toolbarActions"]}>
                    {viewMode === "selected" ? (
                        <button
                            type="button"
                            className={[
                                buttons["secondary"],
                                styles["toolbarButton"],
                            ].join(" ")}
                            aria-disabled={
                                browseActive ||
                                list.kind === "preview" ||
                                keyMissing ||
                                keyState.kind === "loading"
                            }
                            onClick={() => {
                                if (
                                    browseActive ||
                                    list.kind === "preview" ||
                                    keyMissing ||
                                    keyState.kind === "loading"
                                ) {
                                    return;
                                }
                                if (dirty && !confirmDiscardDraft()) return;
                                void startUpstreamBrowse();
                            }}
                        >
                            {t("providers.models.fetch")}
                        </button>
                    ) : (
                        <>
                            {browseActive && (
                                <button
                                    type="button"
                                    className={[
                                        buttons["secondary"],
                                        styles["toolbarButton"],
                                    ].join(" ")}
                                    onClick={() => {
                                        void runCancelBrowse();
                                    }}
                                >
                                    {t("providers.models.cancelFetch")}
                                </button>
                            )}
                            <button
                                type="button"
                                className={[
                                    buttons["secondary"],
                                    styles["toolbarButton"],
                                ].join(" ")}
                                aria-disabled={
                                    browseActive ||
                                    keyMissing ||
                                    keyState.kind === "loading"
                                }
                                onClick={() => {
                                    if (
                                        browseActive ||
                                        keyMissing ||
                                        keyState.kind === "loading"
                                    ) {
                                        return;
                                    }
                                    if (dirty && !confirmDiscardDraft()) return;
                                    setDraftSelected(new Set(baselineSelected));
                                    setViewMode("selected");
                                    setUpstream({ kind: "idle" });
                                    void loadSelectedList();
                                }}
                            >
                                {t("providers.models.backToSelected")}
                            </button>
                        </>
                    )}
                    <button
                        type="button"
                        className={[
                            buttons["primary"],
                            styles["toolbarButton"],
                        ].join(" ")}
                        aria-disabled={!dirty || saveState.kind === "running"}
                        onClick={() => {
                            if (!dirty || saveState.kind === "running") return;
                            void runSave();
                        }}
                    >
                        {saveState.kind === "running"
                            ? t("providers.models.saving")
                            : t("providers.models.save")}
                    </button>
                </div>
            </div>

            {searchSummary !== null && (
                <p className={styles["summary"]} role="status">
                    {searchSummary}
                </p>
            )}

            {layout === "tabPanel" ? (
                <div className={styles["listRegion"]}>
                    {showListShell && (
                        <h3 className={styles["listHeader"]} id={listHeadingId}>
                            {listTitle}
                        </h3>
                    )}
                    <div className={styles["listScroll"]} ref={listScrollRef}>
                        {showListShell ? (
                            <div className={styles["listShell"]}>
                                {listBody}
                            </div>
                        ) : (
                            listBody
                        )}
                    </div>
                </div>
            ) : showListShell ? (
                <>
                    <h3 className={styles["listHeader"]} id={listHeadingId}>
                        {listTitle}
                    </h3>
                    <div className={styles["listShell"]}>{listBody}</div>
                </>
            ) : (
                listBody
            )}
        </section>
    );
}
