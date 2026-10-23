import { Dialog } from "@base-ui/react/dialog";
import {
    useEffect,
    useId,
    useRef,
    useState,
    type JSX,
    type RefObject,
} from "react";
import { useTranslation } from "react-i18next";
import { FieldSelect } from "../../components/FieldSelect";
import { Icon } from "../../components/Icon";
import {
    installMarketplaceSkill,
    listMarketplaceSkills,
    previewMarketplaceInstall,
    searchMarketplaceSkills,
    SkillRequestError,
    type MarketplaceBoard,
    type MarketplaceSkill,
    type SkillErrorCode,
    type SkillInstallPreview,
    type SkillInstallReport,
} from "../../lib/desktop/skills";
import buttons from "../../components/actionButtons.module.css";
import editForm from "../../components/editForm.module.css";
import dialog from "./SkillInstallDialog.module.css";
import styles from "./SkillMarketplaceDialog.module.css";

type CatalogState =
    | { kind: "idle" | "loading" }
    | { kind: "ready"; skills: MarketplaceSkill[] }
    | { kind: "preview" }
    | { kind: "error"; code: SkillErrorCode };

type InstallState =
    | { kind: "idle" | "reviewing" | "installing" }
    | { kind: "reviewed"; preview: SkillInstallPreview }
    | { kind: "installed"; report: SkillInstallReport }
    | { kind: "error"; code: SkillErrorCode; uncertain: boolean };

/** Props for {@link SkillMarketplaceDialog}. */
export interface SkillMarketplaceDialogProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    /** Called once after a successful install so the list can be re-read. */
    onInstalled: (report: SkillInstallReport) => void;
    /** The header button that should regain focus after the dialog closes. */
    returnFocus?: RefObject<HTMLElement | null>;
}

function errorCode(error: unknown): SkillErrorCode {
    return error instanceof SkillRequestError ? error.code : "operation_failed";
}

/** Failures that may have changed something beyond this window's knowledge. */
function uncertain(code: SkillErrorCode): boolean {
    return ["operation_failed", "invalid_response", "outcome_unknown"].includes(
        code,
    );
}

/**
 * Browse and install skills from the skills.sh catalog.
 *
 * Leaderboards and search only return GitHub-installable rows. Review shows the
 * exact Git HEAD command; Install is the only action that contacts GitHub and
 * writes the library. Non-GitHub catalog hosts are refused by Rust.
 */
export function SkillMarketplaceDialog({
    open,
    onOpenChange,
    onInstalled,
    returnFocus,
}: SkillMarketplaceDialogProps): JSX.Element {
    const { t } = useTranslation();
    const prefix = useId();
    const [board, setBoard] = useState<MarketplaceBoard>("all_time");
    const [query, setQuery] = useState("");
    const [selected, setSelected] = useState<MarketplaceSkill | null>(null);
    const [catalog, setCatalog] = useState<CatalogState>({ kind: "idle" });
    const [install, setInstall] = useState<InstallState>({ kind: "idle" });
    const busy =
        catalog.kind === "loading" ||
        install.kind === "reviewing" ||
        install.kind === "installing";
    const mutationPending = useRef(false);
    const active = useRef(true);
    const loadGeneration = useRef(0);
    const searchInput = useRef<HTMLInputElement>(null);
    const confirmButton = useRef<HTMLButtonElement>(null);
    const doneButton = useRef<HTMLButtonElement>(null);

    useEffect(() => {
        active.current = true;
        return () => {
            active.current = false;
        };
    }, []);

    useEffect(() => {
        if (install.kind === "reviewed") confirmButton.current?.focus();
        if (install.kind === "installed") doneButton.current?.focus();
    }, [install.kind]);

    // Open loads the all-time leaderboard; close abandons an unfinished draft.
    // A running installer is not cancelled by close; its result is still reported.
    // State updates stay inside the timer so the effect only schedules work.
    useEffect(() => {
        if (!open) {
            if (mutationPending.current) return;
            const timer = window.setTimeout(() => {
                loadGeneration.current++;
                setBoard("all_time");
                setQuery("");
                setSelected(null);
                setCatalog({ kind: "idle" });
                setInstall({ kind: "idle" });
            }, 0);
            return () => {
                clearTimeout(timer);
            };
        }
        const generation = ++loadGeneration.current;
        const timer = window.setTimeout(() => {
            setCatalog({ kind: "loading" });
            setSelected(null);
            setInstall({ kind: "idle" });
            void listMarketplaceSkills("all_time")
                .then((result) => {
                    if (
                        !active.current ||
                        generation !== loadGeneration.current
                    )
                        return;
                    setCatalog(
                        result.kind === "preview"
                            ? { kind: "preview" }
                            : { kind: "ready", skills: result.skills },
                    );
                })
                .catch((error: unknown) => {
                    if (
                        active.current &&
                        generation === loadGeneration.current
                    ) {
                        setCatalog({ kind: "error", code: errorCode(error) });
                    }
                });
        }, 0);
        return () => {
            clearTimeout(timer);
        };
    }, [open]);

    async function loadBoard(next: MarketplaceBoard): Promise<void> {
        const generation = ++loadGeneration.current;
        setCatalog({ kind: "loading" });
        setSelected(null);
        setInstall({ kind: "idle" });
        try {
            const result = await listMarketplaceSkills(next);
            if (!active.current || generation !== loadGeneration.current)
                return;
            setCatalog(
                result.kind === "preview"
                    ? { kind: "preview" }
                    : { kind: "ready", skills: result.skills },
            );
        } catch (error: unknown) {
            if (active.current && generation === loadGeneration.current) {
                setCatalog({ kind: "error", code: errorCode(error) });
            }
        }
    }

    async function search(): Promise<void> {
        const trimmed = query.trim();
        if (busy || mutationPending.current || trimmed.length < 2) return;
        const generation = ++loadGeneration.current;
        setCatalog({ kind: "loading" });
        setSelected(null);
        setInstall({ kind: "idle" });
        try {
            const result = await searchMarketplaceSkills(trimmed);
            if (!active.current || generation !== loadGeneration.current)
                return;
            setCatalog(
                result.kind === "preview"
                    ? { kind: "preview" }
                    : { kind: "ready", skills: result.skills },
            );
        } catch (error: unknown) {
            if (active.current && generation === loadGeneration.current) {
                setCatalog({ kind: "error", code: errorCode(error) });
            }
        }
    }

    function choose(row: MarketplaceSkill): void {
        if (busy || mutationPending.current) return;
        setSelected(row);
        setInstall({ kind: "idle" });
    }

    async function review(): Promise<void> {
        if (busy || mutationPending.current || selected === null) return;
        mutationPending.current = true;
        setInstall({ kind: "reviewing" });
        try {
            const result = await previewMarketplaceInstall({
                source: selected.source,
                skillId: selected.skillId,
            });
            if (!active.current) return;
            setInstall(
                result.kind === "preview"
                    ? {
                          kind: "error",
                          code: "desktop_required",
                          uncertain: false,
                      }
                    : { kind: "reviewed", preview: result.preview },
            );
        } catch (error: unknown) {
            if (active.current) {
                const code = errorCode(error);
                setInstall({ kind: "error", code, uncertain: uncertain(code) });
            }
        } finally {
            mutationPending.current = false;
        }
    }

    async function installSelected(): Promise<void> {
        if (mutationPending.current || selected === null) return;
        mutationPending.current = true;
        setInstall({ kind: "installing" });
        try {
            const report = await installMarketplaceSkill({
                source: selected.source,
                skillId: selected.skillId,
            });
            if (!active.current) return;
            setInstall({ kind: "installed", report });
            onInstalled(report);
        } catch (error: unknown) {
            if (active.current) {
                const code = errorCode(error);
                setInstall({ kind: "error", code, uncertain: uncertain(code) });
            }
        } finally {
            mutationPending.current = false;
        }
    }

    const boardOptions = [
        { value: "all_time", label: t("skills.marketplace.boardAllTime") },
        { value: "trending", label: t("skills.marketplace.boardTrending") },
        { value: "hot", label: t("skills.marketplace.boardHot") },
    ];

    return (
        <Dialog.Root
            open={open}
            onOpenChange={(nextOpen) => {
                if (!nextOpen && mutationPending.current) return;
                onOpenChange(nextOpen);
            }}
        >
            <Dialog.Portal>
                <Dialog.Backdrop className={dialog["backdrop"]} />
                <Dialog.Popup
                    className={[dialog["popup"], styles["wide"]].join(" ")}
                    initialFocus={searchInput}
                    finalFocus={() => returnFocus?.current ?? null}
                >
                    <Dialog.Title className={dialog["title"]}>
                        {t("skills.marketplace.title")}
                    </Dialog.Title>
                    <Dialog.Description className={dialog["body"]}>
                        {t("skills.marketplace.description")}
                    </Dialog.Description>

                    {install.kind === "installed" ? (
                        <InstallResult report={install.report} />
                    ) : (
                        <>
                            <div className={styles["toolbar"]}>
                                <label
                                    className={dialog["label"]}
                                    htmlFor={`${prefix}-board`}
                                >
                                    {t("skills.marketplace.board")}
                                </label>
                                <FieldSelect
                                    id={`${prefix}-board`}
                                    value={board}
                                    options={boardOptions}
                                    blocked={busy}
                                    size="fill"
                                    onChange={(value) => {
                                        const next: MarketplaceBoard =
                                            value === "trending"
                                                ? "trending"
                                                : value === "hot"
                                                  ? "hot"
                                                  : "all_time";
                                        setBoard(next);
                                        setQuery("");
                                        void loadBoard(next);
                                    }}
                                />
                                <label
                                    className={dialog["label"]}
                                    htmlFor={`${prefix}-search`}
                                >
                                    {t("skills.marketplace.search")}
                                </label>
                                <div className={styles["searchRow"]}>
                                    <input
                                        ref={searchInput}
                                        id={`${prefix}-search`}
                                        className={editForm["input"]}
                                        value={query}
                                        readOnly={busy}
                                        autoComplete="off"
                                        spellCheck={false}
                                        placeholder={t(
                                            "skills.marketplace.searchPlaceholder",
                                        )}
                                        onChange={(event) => {
                                            setQuery(event.currentTarget.value);
                                        }}
                                        onKeyDown={(event) => {
                                            if (event.key === "Enter") {
                                                event.preventDefault();
                                                void search();
                                            }
                                        }}
                                    />
                                    <button
                                        type="button"
                                        className={buttons["secondary"]}
                                        aria-disabled={
                                            busy || query.trim().length < 2
                                        }
                                        onClick={() => {
                                            void search();
                                        }}
                                    >
                                        {t("skills.marketplace.searchAction")}
                                    </button>
                                </div>
                            </div>

                            {catalog.kind === "loading" && (
                                <p className={dialog["hint"]} role="status">
                                    {t("skills.marketplace.loading")}
                                </p>
                            )}
                            {catalog.kind === "preview" && (
                                <p className={dialog["hint"]} role="status">
                                    {t("skills.marketplace.desktopRequired")}
                                </p>
                            )}
                            {catalog.kind === "error" && (
                                <p className={dialog["error"]} role="alert">
                                    {t(`skills.errors.${catalog.code}`)}
                                </p>
                            )}
                            {catalog.kind === "ready" &&
                                catalog.skills.length === 0 && (
                                    <p className={dialog["hint"]} role="status">
                                        {t("skills.marketplace.empty")}
                                    </p>
                                )}
                            {catalog.kind === "ready" &&
                                catalog.skills.length > 0 && (
                                    <ul
                                        className={styles["list"]}
                                        aria-label={t(
                                            "skills.marketplace.listLabel",
                                        )}
                                    >
                                        {catalog.skills.map((row) => {
                                            const activeRow =
                                                selected?.id === row.id;
                                            return (
                                                <li key={row.id}>
                                                    <button
                                                        type="button"
                                                        className={[
                                                            styles["row"],
                                                            activeRow
                                                                ? styles[
                                                                      "rowSelected"
                                                                  ]
                                                                : "",
                                                        ]
                                                            .filter(Boolean)
                                                            .join(" ")}
                                                        aria-pressed={activeRow}
                                                        disabled={busy}
                                                        onClick={() => {
                                                            choose(row);
                                                        }}
                                                    >
                                                        <span
                                                            className={
                                                                styles[
                                                                    "rowName"
                                                                ]
                                                            }
                                                        >
                                                            {row.name}
                                                        </span>
                                                        <span
                                                            className={
                                                                styles[
                                                                    "rowMeta"
                                                                ]
                                                            }
                                                        >
                                                            {t(
                                                                "skills.marketplace.rowMeta",
                                                                {
                                                                    source: row.source,
                                                                    skill: row.skillId,
                                                                    installs:
                                                                        row.installs,
                                                                },
                                                            )}
                                                        </span>
                                                    </button>
                                                </li>
                                            );
                                        })}
                                    </ul>
                                )}

                            {selected !== null &&
                                install.kind !== "reviewed" && (
                                    <p className={dialog["hint"]} role="status">
                                        {t("skills.marketplace.selected", {
                                            name: selected.name,
                                            source: selected.source,
                                        })}
                                    </p>
                                )}

                            {install.kind === "reviewed" && (
                                <div className={dialog["review"]}>
                                    <p className={dialog["reviewHeading"]}>
                                        {t("skills.marketplace.reviewHeading")}
                                    </p>
                                    <dl className={dialog["facts"]}>
                                        <dt>
                                            {t("skills.install.factSource")}
                                        </dt>
                                        <dd>{install.preview.sourceUrl}</dd>
                                        <dt>
                                            {t("skills.install.factRevision")}
                                        </dt>
                                        <dd>{install.preview.revision}</dd>
                                    </dl>
                                    <pre className={dialog["command"]}>
                                        <code>{install.preview.command}</code>
                                    </pre>
                                    <p className={dialog["hint"]}>
                                        {t("skills.marketplace.warning")}
                                    </p>
                                </div>
                            )}

                            {install.kind === "error" && (
                                <div className={dialog["review"]}>
                                    <p className={dialog["error"]} role="alert">
                                        {t(`skills.errors.${install.code}`)}
                                    </p>
                                    {install.uncertain && (
                                        <p className={dialog["hint"]}>
                                            {t("skills.reloadUnknown")}
                                        </p>
                                    )}
                                </div>
                            )}

                            {install.kind === "reviewing" && (
                                <p className={dialog["hint"]} role="status">
                                    {t("skills.install.reviewing")}
                                </p>
                            )}
                            {install.kind === "installing" && (
                                <p className={dialog["hint"]} role="status">
                                    {t("skills.marketplace.installing")}
                                </p>
                            )}

                            <div className={dialog["actions"]}>
                                <button
                                    type="button"
                                    className={buttons["secondary"]}
                                    disabled={busy}
                                    onClick={() => {
                                        onOpenChange(false);
                                    }}
                                >
                                    {t("skills.marketplace.cancel")}
                                </button>
                                {install.kind === "reviewed" ? (
                                    <button
                                        ref={confirmButton}
                                        type="button"
                                        className={buttons["primary"]}
                                        aria-disabled={busy}
                                        onClick={() => {
                                            void installSelected();
                                        }}
                                    >
                                        <Icon name="check" />
                                        {t("skills.marketplace.confirm")}
                                    </button>
                                ) : (
                                    <button
                                        type="button"
                                        className={buttons["primary"]}
                                        aria-disabled={
                                            busy || selected === null
                                        }
                                        onClick={() => {
                                            void review();
                                        }}
                                    >
                                        {t("skills.marketplace.review")}
                                    </button>
                                )}
                            </div>
                        </>
                    )}

                    {install.kind === "installed" && (
                        <div className={dialog["actions"]}>
                            <button
                                ref={doneButton}
                                type="button"
                                className={buttons["primary"]}
                                onClick={() => {
                                    onOpenChange(false);
                                }}
                            >
                                {t("skills.marketplace.done")}
                            </button>
                        </div>
                    )}
                </Dialog.Popup>
            </Dialog.Portal>
        </Dialog.Root>
    );
}

/** The result summary stays inside the dialog until the user closes it. */
function InstallResult({
    report,
}: {
    report: SkillInstallReport;
}): JSX.Element {
    const { t } = useTranslation();
    return (
        <div className={dialog["review"]}>
            <p className={dialog["reviewHeading"]} role="status">
                {report.imported.length === 0
                    ? t("skills.install.resultNone")
                    : t("skills.install.resultImported", {
                          count: report.imported.length,
                      })}
            </p>
            {report.skipped.length > 0 && (
                <>
                    <p className={dialog["hint"]}>
                        {t("skills.install.resultSkipped", {
                            count: report.skipped.length,
                        })}
                    </p>
                    <ul className={dialog["skipped"]}>
                        {report.skipped.map((row, index) => (
                            <li key={`${row.folder}-${String(index)}`}>
                                {t("skills.reportItem", {
                                    folder: row.folder,
                                    reason: t(`skills.errors.${row.code}`),
                                })}
                            </li>
                        ))}
                    </ul>
                </>
            )}
            <dl className={dialog["facts"]}>
                <dt>{t("skills.install.factSource")}</dt>
                <dd>{report.sourceUrl}</dd>
                <dt>{t("skills.install.factResolvedRevision")}</dt>
                <dd>{report.revision}</dd>
                <dt>{t("skills.install.factExit")}</dt>
                <dd>{report.exitStatus}</dd>
            </dl>
            <pre className={dialog["command"]}>
                <code>{report.command}</code>
            </pre>
        </div>
    );
}
