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
    installSkill,
    previewSkillInstall,
    SkillRequestError,
    type SkillErrorCode,
    type SkillFetchMethod,
    type SkillInstallPreview,
    type SkillInstallReport,
    type SkillInstallSource,
} from "../../lib/desktop/skills";
import buttons from "../../components/actionButtons.module.css";
import editForm from "../../components/editForm.module.css";
import dialog from "./SkillInstallDialog.module.css";

type InstallState =
    | { kind: "idle" | "reviewing" | "installing" }
    | { kind: "reviewed"; preview: SkillInstallPreview }
    | { kind: "installed"; report: SkillInstallReport }
    | { kind: "error"; code: SkillErrorCode; uncertain: boolean };

/** Props for {@link SkillInstallDialog}. */
export interface SkillInstallDialogProps {
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
 * Acquire one skill from a reviewed GitHub source.
 *
 * The form never contacts the network. **Review command** validates the fields
 * and shows the exact command; **Install** is the only action that runs the
 * external installer. The installer works in a temporary directory, and only
 * validated `SKILL.md` folders are copied into the library.
 */
export function SkillInstallDialog({
    open,
    onOpenChange,
    onInstalled,
    returnFocus,
}: SkillInstallDialogProps): JSX.Element {
    const { t } = useTranslation();
    const prefix = useId();
    const [method, setMethod] = useState<SkillFetchMethod>("git");
    const [repository, setRepository] = useState("");
    const [revision, setRevision] = useState("");
    const [source, setSource] = useState("");
    const [skill, setSkill] = useState("");
    const [state, setState] = useState<InstallState>({ kind: "idle" });
    const busy = state.kind === "reviewing" || state.kind === "installing";
    const mutationPending = useRef(false);
    const active = useRef(true);
    const repositoryInput = useRef<HTMLInputElement>(null);
    const confirmButton = useRef<HTMLButtonElement>(null);
    const doneButton = useRef<HTMLButtonElement>(null);

    useEffect(() => {
        active.current = true;
        return () => {
            active.current = false;
        };
    }, []);

    // Closing abandons an unfinished draft. A running installer cannot be
    // cancelled by closing the dialog; its result is still reported.
    useEffect(() => {
        if (open) return;
        if (mutationPending.current) return;
        setState({ kind: "idle" });
        setMethod("git");
        setRepository("");
        setRevision("");
        setSource("");
        setSkill("");
    }, [open]);

    // Reviewing replaces the focused button with the confirmation, and a result
    // replaces it with Done. Move focus explicitly so keyboard users are not
    // dropped on the dialog container.
    useEffect(() => {
        if (state.kind === "reviewed") confirmButton.current?.focus();
        if (state.kind === "installed") doneButton.current?.focus();
    }, [state.kind]);

    function edit(update: () => void): void {
        if (state.kind === "reviewing" || state.kind === "installing") return;
        update();
        setState({ kind: "idle" });
    }

    function currentSource(): SkillInstallSource {
        const selector = skill.trim() === "" ? null : skill.trim();
        return method === "git"
            ? {
                  method: "git",
                  source: {
                      repository: repository.trim(),
                      revision: revision.trim(),
                      skill: selector,
                  },
              }
            : {
                  method: "npx",
                  source: {
                      source: source.trim(),
                      // Rust chooses the installer version it reviewed.
                      installerVersion: null,
                      skill: selector,
                  },
              };
    }

    const requiredMissing =
        method === "git"
            ? repository.trim() === "" || revision.trim() === ""
            : source.trim() === "";

    async function review(): Promise<void> {
        if (busy || mutationPending.current || requiredMissing) return;
        mutationPending.current = true;
        setState({ kind: "reviewing" });
        try {
            const result = await previewSkillInstall(currentSource());
            if (!active.current) return;
            setState(
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
                setState({ kind: "error", code, uncertain: uncertain(code) });
            }
        } finally {
            mutationPending.current = false;
        }
    }

    async function install(): Promise<void> {
        if (mutationPending.current) return;
        mutationPending.current = true;
        setState({ kind: "installing" });
        try {
            const report = await installSkill(currentSource());
            if (!active.current) return;
            setState({ kind: "installed", report });
            onInstalled(report);
        } catch (error: unknown) {
            if (active.current) {
                const code = errorCode(error);
                setState({ kind: "error", code, uncertain: uncertain(code) });
            }
        } finally {
            mutationPending.current = false;
        }
    }

    const methodOptions = [
        { value: "git", label: t("skills.install.methodGit") },
        { value: "npx", label: t("skills.install.methodNpx") },
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
                    className={dialog["popup"]}
                    initialFocus={repositoryInput}
                    finalFocus={() => returnFocus?.current ?? null}
                >
                    <Dialog.Title className={dialog["title"]}>
                        {t("skills.install.title")}
                    </Dialog.Title>
                    <Dialog.Description className={dialog["body"]}>
                        {t("skills.install.description")}
                    </Dialog.Description>

                    {state.kind === "installed" ? (
                        <InstallResult report={state.report} />
                    ) : (
                        <>
                            <div className={dialog["fields"]}>
                                <label
                                    className={dialog["label"]}
                                    htmlFor={`${prefix}-method`}
                                >
                                    {t("skills.install.method")}
                                </label>
                                <FieldSelect
                                    id={`${prefix}-method`}
                                    value={method}
                                    options={methodOptions}
                                    blocked={busy}
                                    size="fill"
                                    onChange={(value) => {
                                        const next =
                                            value === "npx" ? "npx" : "git";
                                        edit(() => {
                                            setMethod(next);
                                        });
                                    }}
                                />
                                {method === "git" ? (
                                    <>
                                        <label
                                            className={dialog["label"]}
                                            htmlFor={`${prefix}-repository`}
                                        >
                                            {t("skills.install.repository")}
                                        </label>
                                        <input
                                            ref={repositoryInput}
                                            id={`${prefix}-repository`}
                                            className={editForm["input"]}
                                            value={repository}
                                            readOnly={busy}
                                            autoComplete="off"
                                            spellCheck={false}
                                            placeholder={t(
                                                "skills.install.repositoryPlaceholder",
                                            )}
                                            onChange={(event) => {
                                                const value =
                                                    event.currentTarget.value;
                                                edit(() => {
                                                    setRepository(value);
                                                });
                                            }}
                                        />
                                        <label
                                            className={dialog["label"]}
                                            htmlFor={`${prefix}-revision`}
                                        >
                                            {t("skills.install.revision")}
                                        </label>
                                        <input
                                            id={`${prefix}-revision`}
                                            className={editForm["input"]}
                                            value={revision}
                                            readOnly={busy}
                                            autoComplete="off"
                                            spellCheck={false}
                                            placeholder={t(
                                                "skills.install.revisionPlaceholder",
                                            )}
                                            onChange={(event) => {
                                                const value =
                                                    event.currentTarget.value;
                                                edit(() => {
                                                    setRevision(value);
                                                });
                                            }}
                                        />
                                    </>
                                ) : (
                                    <>
                                        <label
                                            className={dialog["label"]}
                                            htmlFor={`${prefix}-source`}
                                        >
                                            {t("skills.install.source")}
                                        </label>
                                        <input
                                            id={`${prefix}-source`}
                                            className={editForm["input"]}
                                            value={source}
                                            readOnly={busy}
                                            autoComplete="off"
                                            spellCheck={false}
                                            placeholder={t(
                                                "skills.install.sourcePlaceholder",
                                            )}
                                            onChange={(event) => {
                                                const value =
                                                    event.currentTarget.value;
                                                edit(() => {
                                                    setSource(value);
                                                });
                                            }}
                                        />
                                    </>
                                )}
                                <label
                                    className={dialog["label"]}
                                    htmlFor={`${prefix}-skill`}
                                >
                                    {t("skills.install.skill")}
                                </label>
                                <input
                                    id={`${prefix}-skill`}
                                    className={editForm["input"]}
                                    value={skill}
                                    readOnly={busy}
                                    autoComplete="off"
                                    spellCheck={false}
                                    placeholder={t(
                                        "skills.install.skillPlaceholder",
                                    )}
                                    onChange={(event) => {
                                        const value = event.currentTarget.value;
                                        edit(() => {
                                            setSkill(value);
                                        });
                                    }}
                                />
                            </div>

                            {state.kind === "reviewed" && (
                                <div className={dialog["review"]}>
                                    <p className={dialog["reviewHeading"]}>
                                        {t("skills.install.reviewHeading")}
                                    </p>
                                    <dl className={dialog["facts"]}>
                                        <dt>
                                            {t("skills.install.factSource")}
                                        </dt>
                                        <dd>{state.preview.sourceUrl}</dd>
                                        <dt>
                                            {t(
                                                state.preview.method === "git"
                                                    ? "skills.install.factRevision"
                                                    : "skills.install.factInstaller",
                                            )}
                                        </dt>
                                        <dd>
                                            {state.preview.method === "git"
                                                ? state.preview.revision
                                                : `${state.preview.installer}@${state.preview.installerVersion ?? ""}`}
                                        </dd>
                                    </dl>
                                    <pre className={dialog["command"]}>
                                        <code>{state.preview.command}</code>
                                    </pre>
                                    <p className={dialog["hint"]}>
                                        {t("skills.install.warning")}
                                    </p>
                                </div>
                            )}

                            {state.kind === "error" && (
                                <div className={dialog["review"]}>
                                    <p className={dialog["error"]} role="alert">
                                        {t(`skills.errors.${state.code}`)}
                                    </p>
                                    {state.uncertain && (
                                        <p className={dialog["hint"]}>
                                            {t("skills.reloadUnknown")}
                                        </p>
                                    )}
                                </div>
                            )}

                            {state.kind === "reviewing" && (
                                <p className={dialog["hint"]} role="status">
                                    {t("skills.install.reviewing")}
                                </p>
                            )}
                            {state.kind === "installing" && (
                                <p className={dialog["hint"]} role="status">
                                    {t("skills.install.installing")}
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
                                    {t("skills.install.cancel")}
                                </button>
                                {state.kind === "reviewed" ? (
                                    <button
                                        ref={confirmButton}
                                        type="button"
                                        className={buttons["primary"]}
                                        aria-disabled={busy}
                                        onClick={() => {
                                            void install();
                                        }}
                                    >
                                        <Icon name="check" />
                                        {t("skills.install.confirm")}
                                    </button>
                                ) : (
                                    <button
                                        type="button"
                                        className={buttons["primary"]}
                                        aria-disabled={busy || requiredMissing}
                                        onClick={() => {
                                            void review();
                                        }}
                                    >
                                        {t("skills.install.review")}
                                    </button>
                                )}
                            </div>
                        </>
                    )}

                    {state.kind === "installed" && (
                        <div className={dialog["actions"]}>
                            <button
                                ref={doneButton}
                                type="button"
                                className={buttons["primary"]}
                                onClick={() => {
                                    onOpenChange(false);
                                }}
                            >
                                {t("skills.install.done")}
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
                <dt>
                    {t(
                        report.method === "git"
                            ? "skills.install.factResolvedRevision"
                            : "skills.install.factInstaller",
                    )}
                </dt>
                <dd>
                    {report.method === "git"
                        ? report.revision
                        : `${report.installer}@${report.installerVersion ?? ""}`}
                </dd>
                <dt>{t("skills.install.factExit")}</dt>
                <dd>{report.exitStatus}</dd>
            </dl>
            <pre className={dialog["command"]}>
                <code>{report.command}</code>
            </pre>
        </div>
    );
}
