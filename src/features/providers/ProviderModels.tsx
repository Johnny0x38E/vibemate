import {
    useCallback,
    useEffect,
    useRef,
    useState,
    type JSX,
    type ChangeEvent,
} from "react";
import { useTranslation } from "react-i18next";
import { useNotify } from "../../components/Notifications";
import {
    cancelProviderModelFetch,
    fetchProviderModels,
    getProviderModelFetchStatus,
    listProviderModels,
    ModelRequestError,
    setProviderModelsSelected,
    type ModelErrorCode,
    type ModelFetchStatus,
    type ModelFetchSummary,
    type ProviderModel,
} from "../../lib/desktop/models";
import { getProviderSecretStatus } from "../../lib/desktop/providerSecrets";
import { ModelCheckbox } from "../../components/ModelCheckbox";
import buttons from "./providerButtons.module.css";
import styles from "./ProviderModels.module.css";

/** Rows per request; matches the desktop boundary tests and stays under Rust's cap. */
export const MODEL_PAGE_SIZE = 50;

/** Selected rows for tab labels and quick filters. */
export interface SelectedModelCount {
    count: number;
    hasMore: boolean;
}

const SEARCH_DEBOUNCE_MS = 300;

type MoreState =
    { kind: "idle" | "loading" } | { kind: "error"; code: ModelErrorCode };

type ListState =
    | { kind: "loading" }
    | { kind: "preview" }
    | { kind: "error"; code: ModelErrorCode }
    | {
          kind: "ready";
          items: ProviderModel[];
          nextCursor: string | null;
          totalMatches: number | null;
          more: MoreState;
      };

type FetchStatusState =
    | { kind: "loading" }
    | { kind: "preview" }
    | { kind: "ready"; status: ModelFetchStatus }
    | { kind: "error"; code: ModelErrorCode };

type KeyState =
    | { kind: "loading" }
    | { kind: "preview" }
    | { kind: "error"; code: ModelErrorCode }
    | {
          kind: "ready";
          configured: boolean;
      };

type FetchActionState =
    | { kind: "idle" }
    | { kind: "running" }
    | { kind: "error"; code: ModelErrorCode };

/** Inputs for {@link ProviderModels}. */
export interface ProviderModelsProps {
    providerId: string;
    /** When the parent page is hidden, in-flight reads and fetches are cancelled. */
    hidden?: boolean;
    /** Omit the section heading when this panel sits under the Models tab. */
    layout?: "section" | "tabPanel";
    /** Reports fetch or list work so the page can block navigation. */
    onBusyChange?: (busy: boolean) => void;
    /** Updates the Models tab label with how many models are selected. */
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

function replaceModel(
    items: ProviderModel[],
    updated: ProviderModel,
): ProviderModel[] {
    return items.map((model) =>
        model.modelId === updated.modelId ? updated : model,
    );
}

function selectionBlocked(model: ProviderModel): boolean {
    return model.source === "fetched" && model.routeSupport === "unsupported";
}

function displayTitle(model: ProviderModel): string {
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

/**
 * Models for one provider: fetch the upstream catalog into local storage,
 * browse or paginate it, and fuzzy-filter the saved rows (never the API).
 */
export function ProviderModels({
    providerId,
    hidden = false,
    layout = "section",
    onBusyChange,
    onSelectedCountChange,
}: ProviderModelsProps): JSX.Element {
    const { t, i18n } = useTranslation();
    const notify = useNotify();
    const [filter, setFilter] = useState<"all" | "selected">("all");
    const [searchInput, setSearchInput] = useState("");
    const [debouncedQuery, setDebouncedQuery] = useState("");
    const [list, setList] = useState<ListState>({ kind: "loading" });
    const [fetchStatus, setFetchStatus] = useState<FetchStatusState>({
        kind: "loading",
    });
    const [keyState, setKeyState] = useState<KeyState>({ kind: "loading" });
    const [fetchAction, setFetchAction] = useState<FetchActionState>({
        kind: "idle",
    });
    const [rowBusy, setRowBusy] = useState<ReadonlySet<string>>(
        () => new Set(),
    );
    const [rowErrors, setRowErrors] = useState<
        Readonly<Record<string, ModelErrorCode>>
    >(() => ({}));

    const loadGeneration = useRef(0);
    const fetchGeneration = useRef(0);
    const loadingMore = useRef(false);
    const wasPanelHidden = useRef(true);
    const [selectedSummary, setSelectedSummary] =
        useState<SelectedModelCount | null>(null);

    const refreshSelectedCount = useCallback(async (): Promise<void> => {
        if (hidden) return;
        try {
            const result = await listProviderModels({
                providerId,
                after: null,
                limit: MODEL_PAGE_SIZE,
                filter: "selected",
            });
            if (result.kind !== "desktop") {
                setSelectedSummary(null);
                onSelectedCountChange?.(null);
                return;
            }
            const next: SelectedModelCount = {
                count: result.page.items.length,
                hasMore: result.page.nextCursor !== null,
            };
            setSelectedSummary(next);
            onSelectedCountChange?.(next);
        } catch {
            setSelectedSummary(null);
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

    const refreshFetchStatus = useCallback(async (): Promise<void> => {
        const generation = fetchGeneration.current;
        try {
            const result = await getProviderModelFetchStatus(providerId);
            if (hidden || generation !== fetchGeneration.current) return;
            if (result.kind === "preview") {
                setFetchStatus({ kind: "preview" });
                return;
            }
            setFetchStatus({ kind: "ready", status: result.status });
        } catch (error: unknown) {
            if (hidden || generation !== fetchGeneration.current) return;
            setFetchStatus({ kind: "error", code: errorCode(error) });
        }
    }, [providerId, hidden]);

    const loadFirstPage = useCallback(async (): Promise<void> => {
        const generation = ++loadGeneration.current;
        setList({ kind: "loading" });
        setRowErrors({});
        try {
            const result = await listProviderModels({
                providerId,
                after: null,
                limit: MODEL_PAGE_SIZE,
                filter,
                ...(debouncedQuery ? { query: debouncedQuery } : {}),
            });
            if (hidden || generation !== loadGeneration.current) return;
            if (result.kind === "preview") {
                setList({ kind: "preview" });
                return;
            }
            setList({
                kind: "ready",
                items: result.page.items,
                nextCursor: result.page.nextCursor,
                totalMatches: result.page.totalMatches,
                more: { kind: "idle" },
            });
            void refreshSelectedCount();
        } catch (error: unknown) {
            if (hidden || generation !== loadGeneration.current) return;
            setList({ kind: "error", code: errorCode(error) });
        }
    }, [providerId, filter, debouncedQuery, hidden, refreshSelectedCount]);

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

    useEffect(() => {
        if (hidden) {
            loadGeneration.current += 1;
            fetchGeneration.current += 1;
            void cancelProviderModelFetch(providerId);
            return;
        }
        setFetchStatus({ kind: "loading" });
        setKeyState({ kind: "loading" });
        void refreshFetchStatus();
        void loadKeyStatus();
        void refreshSelectedCount();
    }, [
        hidden,
        providerId,
        refreshFetchStatus,
        loadKeyStatus,
        refreshSelectedCount,
    ]);

    useEffect(() => {
        if (hidden) return;
        void loadFirstPage();
    }, [hidden, loadFirstPage]);

    async function loadMore(): Promise<void> {
        if (list.kind !== "ready" || list.nextCursor === null) return;
        if (debouncedQuery !== "") return;
        if (loadingMore.current) return;
        loadingMore.current = true;
        const generation = loadGeneration.current;
        const after = list.nextCursor;
        setList({ ...list, more: { kind: "loading" } });
        try {
            const result = await listProviderModels({
                providerId,
                after,
                limit: MODEL_PAGE_SIZE,
                filter,
            });
            if (hidden || generation !== loadGeneration.current) return;
            if (result.kind === "preview") {
                setList({ kind: "preview" });
                return;
            }
            setList((previous) =>
                previous.kind === "ready"
                    ? {
                          kind: "ready",
                          items: mergeModels(previous.items, result.page.items),
                          nextCursor: result.page.nextCursor,
                          totalMatches: result.page.totalMatches,
                          more: { kind: "idle" },
                      }
                    : previous,
            );
        } catch (error: unknown) {
            if (hidden || generation !== loadGeneration.current) return;
            setList((previous) =>
                previous.kind === "ready"
                    ? {
                          ...previous,
                          more: { kind: "error", code: errorCode(error) },
                      }
                    : previous,
            );
        } finally {
            loadingMore.current = false;
        }
    }

    async function toggleSelection(
        model: ProviderModel,
        nextSelected: boolean,
    ): Promise<void> {
        if (selectionBlocked(model)) return;
        if (rowBusy.has(model.modelId)) return;
        if (list.kind !== "ready") return;
        if (model.selected === nextSelected) return;
        const previousItems = list.items;
        setList({
            ...list,
            items: replaceModel(list.items, {
                ...model,
                selected: nextSelected,
            }),
        });
        setRowErrors((errors) => {
            const next: Record<string, ModelErrorCode> = {};
            for (const [key, code] of Object.entries(errors)) {
                if (key !== model.modelId) next[key] = code;
            }
            return next;
        });
        setRowBusy((busy) => new Set(busy).add(model.modelId));

        try {
            const updated = await setProviderModelsSelected({
                providerId,
                modelIds: [model.modelId],
                selected: nextSelected,
            });
            const record = updated[0];
            if (record === undefined)
                throw new ModelRequestError("invalid_response");
            setList((current) =>
                current.kind === "ready"
                    ? {
                          ...current,
                          items: replaceModel(current.items, record),
                      }
                    : current,
            );
        } catch (error: unknown) {
            setList((current) =>
                current.kind === "ready"
                    ? { ...current, items: previousItems }
                    : current,
            );
            setRowErrors((errors) => ({
                ...errors,
                [model.modelId]: errorCode(error),
            }));
        } finally {
            setRowBusy((busy) => {
                const next = new Set(busy);
                next.delete(model.modelId);
                return next;
            });
            void refreshSelectedCount();
        }
    }

    useEffect(() => {
        if (wasPanelHidden.current && !hidden && selectedSummary !== null) {
            if (selectedSummary.count > 0) {
                setFilter("selected");
            }
        }
        wasPanelHidden.current = hidden;
    }, [hidden, selectedSummary]);

    function notifyFetchSummary(summary: ModelFetchSummary): void {
        if (summary.complete) {
            notify({
                tone: "success",
                message: t("providers.models.fetchNotice.complete", {
                    listed: summary.listed,
                }),
            });
            return;
        }
        const reasonKey =
            summary.incompleteReason === null
                ? "model_limit"
                : summary.incompleteReason;
        const tone = reasonKey === "empty_list" ? "error" : "success";
        notify({
            tone,
            message: t(`providers.models.fetchNotice.incomplete.${reasonKey}`, {
                listed: summary.listed,
            }),
        });
    }

    async function runFetch(): Promise<void> {
        if (fetchAction.kind === "running") return;
        setFetchAction({ kind: "running" });
        const generation = fetchGeneration.current;
        try {
            const summary = await fetchProviderModels(providerId);
            if (hidden || generation !== fetchGeneration.current) return;
            notifyFetchSummary(summary);
            setFetchAction({ kind: "idle" });
            await refreshFetchStatus();
            await loadFirstPage();
            await refreshSelectedCount();
        } catch (error: unknown) {
            if (hidden || generation !== fetchGeneration.current) return;
            setFetchAction({ kind: "error", code: errorCode(error) });
            await refreshFetchStatus();
        }
    }

    async function runCancelFetch(): Promise<void> {
        try {
            await cancelProviderModelFetch(providerId);
        } catch {
            // Cancellation is best-effort; status refresh shows the truth.
        }
        await refreshFetchStatus();
        if (fetchAction.kind === "running") {
            setFetchAction({ kind: "idle" });
        }
    }

    function onSearchChange(event: ChangeEvent<HTMLInputElement>): void {
        setSearchInput(event.currentTarget.value);
    }

    const serverFetchRunning =
        fetchStatus.kind === "ready" && fetchStatus.status.running;
    const fetchRunning = fetchAction.kind === "running" || serverFetchRunning;

    const hasFetchedBefore =
        fetchStatus.kind === "ready" && fetchStatus.status.lastFetch !== null;

    const keyMissing = keyState.kind === "ready" && !keyState.configured;

    /** Local fuzzy filter only applies after a catalog exists on disk. */
    const canFilterLocally =
        hasFetchedBefore ||
        (list.kind === "ready" && list.items.length > 0) ||
        (list.kind === "ready" &&
            debouncedQuery !== "" &&
            list.totalMatches !== null &&
            list.totalMatches > 0);

    const listBusy =
        list.kind === "loading" ||
        fetchRunning ||
        (list.kind === "ready" && list.more.kind === "loading");

    const searchSummary =
        list.kind === "ready" &&
        debouncedQuery !== "" &&
        list.totalMatches !== null
            ? t("providers.models.searchSummary", {
                  total: list.totalMatches,
                  shown: list.items.length,
              })
            : null;

    const emptyMessage = ((): string => {
        if (keyMissing) return t("providers.models.emptyNeedKey");
        if (!hasFetchedBefore) return t("providers.models.emptyNeedFetch");
        if (debouncedQuery !== "" || filter === "selected") {
            return t("providers.models.emptyFiltered");
        }
        return t("providers.models.empty");
    })();

    const sectionBusy = fetchRunning || list.kind === "loading";

    useEffect(() => {
        // Background list loads must not block the API tab or the back control.
        onBusyChange?.(!hidden && sectionBusy);
    }, [onBusyChange, sectionBusy, hidden]);

    const sectionLabel =
        layout === "section" ? "provider-models-heading" : undefined;

    const listBody = (
        <>
            {list.kind === "loading" && !fetchRunning && (
                <p className={styles["message"]} role="status">
                    {t("providers.models.loading")}
                </p>
            )}

            {list.kind === "preview" && (
                <p className={styles["message"]} role="status">
                    {t("providers.models.preview")}
                </p>
            )}

            {list.kind === "error" && (
                <>
                    <p className={styles["error"]} role="alert">
                        {t(`providers.models.errors.${list.code}`)}
                    </p>
                    <button
                        type="button"
                        className={buttons["secondary"]}
                        onClick={() => {
                            void loadFirstPage();
                        }}
                    >
                        {t("providers.models.retry")}
                    </button>
                </>
            )}

            {list.kind === "ready" && list.items.length === 0 && (
                <p className={styles["empty"]} role="status">
                    {emptyMessage}
                </p>
            )}

            {list.kind === "ready" && list.items.length > 0 && (
                <ul
                    className={styles["list"]}
                    aria-label={t("providers.models.listLabel")}
                >
                    {list.items.map((model) => {
                        const busy = rowBusy.has(model.modelId);
                        const blocked = selectionBlocked(model);
                        const rowError = rowErrors[model.modelId];
                        return (
                            <li key={model.modelId} className={styles["item"]}>
                                <ModelCheckbox
                                    checked={model.selected}
                                    disabled={listBusy || busy || blocked}
                                    ariaLabel={
                                        model.selected
                                            ? t(
                                                  "providers.models.deselectModel",
                                                  {
                                                      modelId: model.modelId,
                                                  },
                                              )
                                            : t(
                                                  "providers.models.selectModel",
                                                  {
                                                      modelId: model.modelId,
                                                  },
                                              )
                                    }
                                    onCheckedChange={(nextSelected) => {
                                        void toggleSelection(
                                            model,
                                            nextSelected,
                                        );
                                    }}
                                />
                                <div className={styles["body"]}>
                                    <p className={styles["line"]}>
                                        <span className={styles["lineText"]}>
                                            <span className={styles["name"]}>
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
                                            <span className={styles["badge"]}>
                                                {t(
                                                    "providers.models.badges.routeUnsupported",
                                                )}
                                            </span>
                                        )}
                                        {model.routeSupport === "unknown" && (
                                            <span className={styles["badge"]}>
                                                {t(
                                                    "providers.models.badges.routeUnknown",
                                                )}
                                            </span>
                                        )}
                                    </p>
                                    {model.upstreamState === "missing" &&
                                        model.missingSinceMs !== null && (
                                            <div className={styles["badges"]}>
                                                <span
                                                    className={styles["badge"]}
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
                                    {rowError !== undefined && (
                                        <p
                                            className={styles["rowError"]}
                                            role="alert"
                                        >
                                            {t(
                                                `providers.models.errors.${rowError}`,
                                            )}
                                        </p>
                                    )}
                                </div>
                            </li>
                        );
                    })}
                </ul>
            )}

            {list.kind === "ready" && list.more.kind === "error" && (
                <p className={styles["error"]} role="alert">
                    {t(`providers.models.errors.${list.more.code}`)}
                </p>
            )}

            {list.kind === "ready" &&
                debouncedQuery === "" &&
                list.nextCursor !== null && (
                    <div className={styles["more"]}>
                        <button
                            type="button"
                            className={buttons["secondary"]}
                            aria-disabled={list.more.kind === "loading"}
                            onClick={() => {
                                void loadMore();
                            }}
                        >
                            {list.more.kind === "loading"
                                ? t("providers.models.loadingMore")
                                : t("providers.models.loadMore")}
                        </button>
                    </div>
                )}
        </>
    );

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

            {fetchAction.kind === "error" && (
                <p className={styles["error"]} role="alert">
                    {t(`providers.models.errors.${fetchAction.code}`)}
                </p>
            )}

            {fetchStatus.kind === "error" && (
                <p className={styles["error"]} role="alert">
                    {t(`providers.models.errors.${fetchStatus.code}`)}
                </p>
            )}

            {keyMissing && (
                <p className={styles["message"]} role="status">
                    {t("providers.models.needKey")}
                </p>
            )}

            {fetchRunning && (
                <p className={styles["message"]} role="status">
                    {t("providers.models.fetchRunningStatus")}
                </p>
            )}

            <div className={styles["filterBlock"]}>
                <div className={styles["searchToolbar"]}>
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
                            !canFilterLocally ||
                            fetchRunning
                        }
                        onChange={onSearchChange}
                    />
                    <button
                        type="button"
                        className={buttons["primary"]}
                        aria-disabled={
                            fetchRunning ||
                            list.kind === "preview" ||
                            keyMissing ||
                            keyState.kind === "loading"
                        }
                        onClick={() => {
                            void runFetch();
                        }}
                    >
                        {fetchRunning
                            ? t("providers.models.fetching")
                            : t("providers.models.fetch")}
                    </button>
                    {fetchRunning && (
                        <button
                            type="button"
                            className={buttons["secondary"]}
                            onClick={() => {
                                void runCancelFetch();
                            }}
                        >
                            {t("providers.models.cancelFetch")}
                        </button>
                    )}
                </div>
                <div
                    className={styles["filter"]}
                    role="group"
                    aria-label={t("providers.models.filter.ariaLabel")}
                >
                    {(["all", "selected"] as const).map((value) => {
                        const label =
                            value === "selected" &&
                            selectedSummary !== null &&
                            selectedSummary.count > 0
                                ? selectedSummary.hasMore
                                    ? t(
                                          "providers.models.filter.selectedMany",
                                          { count: selectedSummary.count },
                                      )
                                    : t(
                                          "providers.models.filter.selectedCount",
                                          { count: selectedSummary.count },
                                      )
                                : t(`providers.models.filter.${value}`);
                        return (
                            <button
                                key={value}
                                type="button"
                                className={styles["filterButton"]}
                                aria-pressed={filter === value}
                                disabled={
                                    list.kind === "preview" || !canFilterLocally
                                }
                                onClick={() => {
                                    setFilter(value);
                                }}
                            >
                                {label}
                            </button>
                        );
                    })}
                </div>
            </div>

            {searchSummary !== null && (
                <p className={styles["summary"]} role="status">
                    {searchSummary}
                </p>
            )}

            {filter === "all" &&
                selectedSummary !== null &&
                selectedSummary.count > 0 && (
                    <div className={styles["selectedJump"]}>
                        <span className={styles["selectedJumpText"]}>
                            {selectedSummary.hasMore
                                ? t("providers.models.selectedSummaryMany", {
                                      count: selectedSummary.count,
                                  })
                                : t("providers.models.selectedSummary", {
                                      count: selectedSummary.count,
                                  })}
                        </span>
                        <button
                            type="button"
                            className={buttons["text"]}
                            onClick={() => {
                                setFilter("selected");
                            }}
                        >
                            {t("providers.models.showSelectedOnly")}
                        </button>
                    </div>
                )}

            {layout === "tabPanel" ? (
                <div className={styles["listRegion"]}>{listBody}</div>
            ) : (
                listBody
            )}
        </section>
    );
}
