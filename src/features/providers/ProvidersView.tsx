import { useCallback, useEffect, useRef, useState, type JSX } from "react";
import { useTranslation } from "react-i18next";
import { Icon } from "../../components/Icon";
import { useNotify } from "../../components/Notifications";
import {
    listProviders,
    listProviderTemplates,
    MAX_PROVIDER_PAGE_SIZE,
    ProviderRequestError,
    type ProviderErrorCode,
    type ProviderPageResult,
    type ProviderProtocol,
    type ProviderRecord,
    type ProviderTemplate,
} from "../../lib/desktop/providers";
import { ProviderForm } from "./ProviderForm";
import { ProviderIcon } from "./ProviderIcon";
import { ProviderKeys } from "./ProviderKeys";
import { ProviderPage } from "./ProviderPage";
import buttons from "./providerButtons.module.css";
import styles from "./ProvidersView.module.css";

/** Rows per page; small enough to render instantly, well under the Rust cap. */
export const PROVIDER_PAGE_SIZE = 20;

type TemplatesState =
    | { kind: "loading" | "preview" }
    | { kind: "ready"; templates: ProviderTemplate[] }
    | { kind: "error"; code: ProviderErrorCode };

type MoreState =
    { kind: "idle" | "loading" } | { kind: "error"; code: ProviderErrorCode };

type ListState =
    | { kind: "loading" | "preview" }
    | { kind: "error"; code: ProviderErrorCode }
    | {
          kind: "ready";
          items: ProviderRecord[];
          nextCursor: string | null;
          more: MoreState;
      };

/**
 * Where keyboard focus goes on the list page after the render that removes the
 * focused control (returning from a secondary page, a retry, or the last "Load
 * more"). Without this, focus would fall back to the document body.
 */
type FocusTarget = { kind: "heading" | "add" } | { kind: "edit"; id: string };

/**
 * Which view of the Providers page is shown. Secondary views replace the list
 * inside this page (no router): the form for a new configuration, and the
 * detail page of one saved configuration. The edited record is captured on
 * open, so a list reload cannot replace it.
 */
type ProvidersPage =
    | { kind: "list" }
    | { kind: "create" }
    | { kind: "edit"; record: ProviderRecord };

function errorCode(error: unknown): ProviderErrorCode {
    return error instanceof ProviderRequestError
        ? error.code
        : "operation_failed";
}

function listFromResult(result: ProviderPageResult): ListState {
    return result.kind === "preview"
        ? { kind: "preview" }
        : {
              kind: "ready",
              items: result.page.items,
              nextCursor: result.page.nextCursor,
              more: { kind: "idle" },
          };
}

/** Append rows by `id`; names may repeat, so they never identify a row. */
function mergeRows(
    items: readonly ProviderRecord[],
    rows: readonly ProviderRecord[],
): ProviderRecord[] {
    const merged = new Map(items.map((item) => [item.id, item]));
    for (const row of rows) merged.set(row.id, row);
    return [...merged.values()];
}

/**
 * The Providers page. The list view shows saved, non-secret configurations as
 * rows (name first, then provider, protocol and URL, then a short ID) with
 * cursor paging and a single primary "New" action. "New" opens the form, whose
 * first field picks the provider and which also takes the required API key; a
 * row's "Edit" opens its detail page, grouped like the Settings page: "Basic
 * information", then "Keys" / 「密钥」 (`ProviderKeys`), so later groups (models) do not
 * lengthen one form. The list never shows keys or a "needs a key" state.
 *
 * Saved settings are not used to connect anywhere yet, and the page never
 * claims otherwise. Browser preview shows that nothing can be read or saved.
 * List data stays in this component while a secondary view is open, so going
 * back does not re-read it. Buttons that may hold focus use `aria-disabled`,
 * never `disabled`; their handlers check the state.
 */
export function ProvidersView({
    hidden = false,
}: {
    hidden?: boolean;
}): JSX.Element {
    const { t } = useTranslation();
    const notify = useNotify();
    const [templates, setTemplates] = useState<TemplatesState>({
        kind: "loading",
    });
    const [templatesAttempt, setTemplatesAttempt] = useState(0);
    const [list, setList] = useState<ListState>({ kind: "loading" });
    const [listAttempt, setListAttempt] = useState(0);
    const [page, setPage] = useState<ProvidersPage>({ kind: "list" });
    const [formBusy, setFormBusy] = useState(false);
    const [keysBusy, setKeysBusy] = useState(false);
    // Each list read takes a new generation; a result from an older generation
    // (superseded by a reload, or after unmount) is discarded.
    const listGeneration = useRef(0);
    const loadingMore = useRef(false);
    const section = useRef<HTMLElement>(null);
    const focusTarget = useRef<FocusTarget | null>(null);

    // Runs after every render: apply a requested focus move once its target
    // exists. A hidden page never takes focus from the visible one.
    useEffect(() => {
        const target = focusTarget.current;
        focusTarget.current = null;
        const root = section.current;
        if (target === null || hidden || root === null) return;
        const selector =
            target.kind === "edit"
                ? `[data-provider-id="${target.id}"]`
                : `[data-focus="${target.kind}"]`;
        (
            root.querySelector<HTMLElement>(selector) ??
            root.querySelector<HTMLElement>('[data-focus="heading"]')
        )?.focus();
    });

    useEffect(() => {
        // Cleanup prevents late IPC, including StrictMode's discarded first
        // request, from replacing the current state.
        let active = true;
        listProviderTemplates().then(
            (result) => {
                if (active)
                    setTemplates(
                        result.kind === "preview"
                            ? { kind: "preview" }
                            : { kind: "ready", templates: result.templates },
                    );
            },
            (error: unknown) => {
                if (active)
                    setTemplates({ kind: "error", code: errorCode(error) });
            },
        );
        return () => {
            active = false;
        };
    }, [templatesAttempt]);

    useEffect(() => {
        listGeneration.current += 1;
        const generation = listGeneration.current;
        listProviders({ after: null, limit: PROVIDER_PAGE_SIZE }).then(
            (result) => {
                if (listGeneration.current === generation)
                    setList(listFromResult(result));
            },
            (error: unknown) => {
                if (listGeneration.current === generation)
                    setList({ kind: "error", code: errorCode(error) });
            },
        );
        return () => {
            listGeneration.current += 1;
        };
    }, [listAttempt]);

    /**
     * Re-read from the first page. The limit covers the rows already shown plus
     * one, so a newly created row (sorted last) is visible when it fits a page.
     * Resolves `true` only when a desktop page was read and applied.
     *
     * `focusId` names a just-saved row whose Edit button should hold focus once
     * this read is applied (the page heading if the row is not shown). It is
     * only re-applied while focus is still where the save left it, and a
     * superseded read applies nothing.
     */
    async function refreshList(focusId?: string): Promise<boolean> {
        listGeneration.current += 1;
        const generation = listGeneration.current;
        const shown = list.kind === "ready" ? list.items.length : 0;
        const limit = Math.min(
            MAX_PROVIDER_PAGE_SIZE,
            Math.max(PROVIDER_PAGE_SIZE, shown + 1),
        );
        try {
            const result = await listProviders({ after: null, limit });
            if (listGeneration.current !== generation) return false;
            keepSavedFocus(focusId);
            setList(listFromResult(result));
            return result.kind === "desktop";
        } catch (error: unknown) {
            if (listGeneration.current === generation) {
                keepSavedFocus(focusId);
                setList({ kind: "error", code: errorCode(error) });
            }
            return false;
        }
    }

    /**
     * Before a re-read replaces the rows, request focus on the saved row again,
     * unless the user has moved focus since. If the read drops the row (an
     * error, or it is no longer on the loaded pages), focus falls back to the
     * heading instead of the document body.
     */
    function keepSavedFocus(focusId: string | undefined): void {
        if (focusId === undefined) return;
        const active = document.activeElement;
        const untouched =
            active === null ||
            active === document.body ||
            active.getAttribute("data-focus") === "heading" ||
            active.getAttribute("data-provider-id") === focusId;
        if (untouched) focusTarget.current = { kind: "edit", id: focusId };
    }

    async function loadMore(): Promise<void> {
        if (list.kind !== "ready" || list.nextCursor === null) return;
        if (loadingMore.current) return;
        loadingMore.current = true;
        const generation = listGeneration.current;
        const after = list.nextCursor;
        setList({ ...list, more: { kind: "loading" } });
        try {
            const result = await listProviders({
                after,
                limit: PROVIDER_PAGE_SIZE,
            });
            if (listGeneration.current !== generation) return;
            if (result.kind === "preview") {
                setList({ kind: "preview" });
                return;
            }
            // The button disappears on the last page; continue at the first new row.
            const moreFocused =
                document.activeElement?.getAttribute("data-focus") === "more";
            if (moreFocused && result.page.nextCursor === null) {
                const first = result.page.items[0];
                focusTarget.current = first
                    ? { kind: "edit", id: first.id }
                    : { kind: "heading" };
            }
            setList((previous) =>
                previous.kind === "ready"
                    ? {
                          kind: "ready",
                          items: mergeRows(previous.items, result.page.items),
                          nextCursor: result.page.nextCursor,
                          more: { kind: "idle" },
                      }
                    : previous,
            );
        } catch (error: unknown) {
            if (listGeneration.current !== generation) return;
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

    /** Show a returned record now; a create is appended only on the last page. */
    function showRecord(record: ProviderRecord): void {
        setList((previous) => {
            if (previous.kind !== "ready") return previous;
            const known = previous.items.some((item) => item.id === record.id);
            if (!known && previous.nextCursor !== null) return previous;
            return { ...previous, items: mergeRows(previous.items, [record]) };
        });
    }

    // Stable so the form's busy effect does not re-run on every render.
    const reportBusy = useCallback((busy: boolean) => {
        setFormBusy(busy);
    }, []);
    const reportKeysBusy = useCallback((busy: boolean) => {
        setKeysBusy(busy);
    }, []);

    function open(next: ProvidersPage): void {
        // A secondary page focuses its own title when it opens.
        setPage(next);
    }

    /** Back to the list, focusing the control that started this flow. */
    function backToList(): void {
        focusTarget.current =
            page.kind === "edit"
                ? { kind: "edit", id: page.record.id }
                : { kind: "add" };
        setPage({ kind: "list" });
    }

    function handleSaved(record: ProviderRecord): void {
        // Focus moves to the saved row's Edit button, which shows the values Rust
        // stored; a new row that is not on the loaded pages focuses the heading.
        focusTarget.current = { kind: "edit", id: record.id };
        setPage({ kind: "list" });
        showRecord(record);
        // A brief app-level notification names what Rust stored; it is announced
        // without taking focus and does not stay on the list.
        notify(t("providers.saved", { name: record.displayName }));
        // Confirm against storage; refreshList handles its own failure and
        // keeps focus on the row once the re-read is applied.
        void refreshList(record.id);
    }

    const preview = templates.kind === "preview" || list.kind === "preview";
    const availableTemplates =
        templates.kind === "ready" ? templates.templates : [];
    const addBlocked = availableTemplates.length === 0;
    const editBlocked = templates.kind !== "ready";
    const brandFor = (record: ProviderRecord): string =>
        availableTemplates.find((entry) => entry.kind === record.kind)
            ?.brandName ?? record.kind;
    const protocolLabel = (protocol: ProviderProtocol): string =>
        t(`providers.protocols.${protocol}`);

    function secondaryPage(): JSX.Element | null {
        switch (page.kind) {
            case "list":
                return null;
            case "create":
            case "edit":
                return (
                    <ProviderPage
                        // Keyed by page and instance: each opens fresh and takes focus.
                        key={page.kind === "edit" ? page.record.id : "create"}
                        title={
                            page.kind === "edit"
                                ? t("providers.form.editTitle", {
                                      name: page.record.displayName,
                                  })
                                : t("providers.form.createTitle")
                        }
                        onBack={backToList}
                        backBlocked={formBusy || keysBusy}
                    >
                        <ProviderForm
                            templates={availableTemplates}
                            mode={page}
                            onSaved={handleSaved}
                            onRecordLoaded={showRecord}
                            onRefresh={refreshList}
                            onCancel={backToList}
                            onBusyChange={reportBusy}
                            hidden={hidden}
                        />
                        {/* A separate group with its own form: replacing the key
                            never saves the basic information or leaves the page.
                            Going back unmounts it, which discards a typed key. */}
                        {page.kind === "edit" && (
                            <ProviderKeys
                                providerId={page.record.id}
                                onBusyChange={reportKeysBusy}
                                hidden={hidden}
                            />
                        )}
                    </ProviderPage>
                );
        }
    }

    function listPage(): JSX.Element {
        return (
            <>
                <div className={styles["header"]}>
                    <h1
                        className={styles["title"]}
                        data-focus="heading"
                        tabIndex={-1}
                    >
                        {t("desktop.nav.providers")}
                    </h1>
                    {!preview && (
                        <button
                            className={[buttons["primary"], buttons["withIcon"]]
                                .filter(
                                    (value): value is string =>
                                        value !== undefined,
                                )
                                .join(" ")}
                            type="button"
                            data-focus="add"
                            aria-disabled={addBlocked}
                            onClick={() => {
                                if (!addBlocked) open({ kind: "create" });
                            }}
                        >
                            {/* Decorative; the text names the button. */}
                            <Icon name="plus" />
                            {t("providers.add")}
                        </button>
                    )}
                </div>
                <p className={styles["note"]}>{t("providers.note")}</p>

                {preview && (
                    <p className={styles["notice"]} role="status">
                        {t("providers.preview")}
                    </p>
                )}

                {!preview && templates.kind === "error" && (
                    <div className={styles["notice"]}>
                        <p className={styles["error"]} role="alert">
                            {t(`providers.errors.${templates.code}`)}
                        </p>
                        <button
                            className={buttons["secondary"]}
                            type="button"
                            onClick={() => {
                                // The retry button disappears while reading.
                                focusTarget.current = { kind: "heading" };
                                setTemplates({ kind: "loading" });
                                setTemplatesAttempt((previous) => previous + 1);
                            }}
                        >
                            {t("providers.retryTemplates")}
                        </button>
                    </div>
                )}

                {list.kind === "loading" && (
                    <p className={styles["notice"]} role="status">
                        {t("providers.loading")}
                    </p>
                )}

                {list.kind === "error" && (
                    <div className={styles["notice"]}>
                        <p className={styles["error"]} role="alert">
                            {t(`providers.errors.${list.code}`)}
                        </p>
                        <button
                            className={buttons["secondary"]}
                            type="button"
                            onClick={() => {
                                focusTarget.current = { kind: "heading" };
                                setList({ kind: "loading" });
                                setListAttempt((previous) => previous + 1);
                            }}
                        >
                            {t("providers.retry")}
                        </button>
                    </div>
                )}

                {list.kind === "ready" && list.items.length === 0 && (
                    <p className={styles["notice"]}>{t("providers.empty")}</p>
                )}

                {list.kind === "ready" && list.items.length > 0 && (
                    <ul
                        className={styles["list"]}
                        aria-label={t("providers.list.label")}
                    >
                        {list.items.map((record) => {
                            const shortId = record.id.slice(0, 8);
                            return (
                                <li className={styles["item"]} key={record.id}>
                                    <div className={styles["details"]}>
                                        <h2 className={styles["name"]}>
                                            <ProviderIcon kind={record.kind} />
                                            <span
                                                className={styles["nameText"]}
                                            >
                                                {record.displayName}
                                            </span>
                                        </h2>
                                        <p className={styles["summary"]}>
                                            {t("providers.list.summary", {
                                                brand: brandFor(record),
                                                protocol: protocolLabel(
                                                    record.protocol,
                                                ),
                                            })}
                                        </p>
                                        <p className={styles["url"]}>
                                            {record.baseUrl}
                                        </p>
                                        <p className={styles["id"]}>
                                            {t("providers.list.id", {
                                                id: shortId,
                                            })}
                                        </p>
                                    </div>
                                    <button
                                        className={[
                                            buttons["text"],
                                            buttons["withIcon"],
                                        ]
                                            .filter(
                                                (value): value is string =>
                                                    value !== undefined,
                                            )
                                            .join(" ")}
                                        type="button"
                                        data-provider-id={record.id}
                                        aria-disabled={editBlocked}
                                        aria-label={t(
                                            "providers.list.editLabel",
                                            {
                                                name: record.displayName,
                                                id: shortId,
                                            },
                                        )}
                                        onClick={() => {
                                            if (!editBlocked)
                                                open({ kind: "edit", record });
                                        }}
                                    >
                                        {/* Decorative; aria-label names the row. */}
                                        <Icon name="edit" />
                                        {t("providers.list.edit")}
                                    </button>
                                </li>
                            );
                        })}
                    </ul>
                )}

                {list.kind === "ready" && list.more.kind === "error" && (
                    <p className={styles["error"]} role="alert">
                        {t(`providers.errors.${list.more.code}`)}
                    </p>
                )}

                {list.kind === "ready" && list.nextCursor !== null && (
                    <div className={styles["more"]}>
                        <button
                            className={buttons["secondary"]}
                            type="button"
                            data-focus="more"
                            aria-disabled={list.more.kind === "loading"}
                            onClick={() => {
                                void loadMore();
                            }}
                        >
                            {list.more.kind === "loading"
                                ? t("providers.loadingMore")
                                : t("providers.loadMore")}
                        </button>
                    </div>
                )}
            </>
        );
    }

    return (
        <section className={styles["view"]} hidden={hidden} ref={section}>
            {page.kind === "list" ? listPage() : secondaryPage()}
        </section>
    );
}
