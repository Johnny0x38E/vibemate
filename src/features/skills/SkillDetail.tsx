import {
    useEffect,
    useId,
    useRef,
    useState,
    type SubmitEvent,
    type JSX,
} from "react";
import { useTranslation } from "react-i18next";
import { DetailFact, DetailPanel } from "../../components/DetailForm";
import { EditFormRow } from "../../components/EditFormRow";
import { skillSource } from "./skillSource";
import { ProviderPage } from "../providers/ProviderPage";
import { SecondaryPageActions } from "../../components/SecondaryPageActions";
import {
    setSkillSource,
    SkillRequestError,
    type SkillErrorCode,
    type SkillRecord,
} from "../../lib/desktop/skills";
import buttons from "../../components/actionButtons.module.css";
import detailForm from "../../components/detailForm.module.css";
import editForm from "../../components/editForm.module.css";
import styles from "./SkillLibrary.module.css";

type FormState =
    | { kind: "idle" | "saving" }
    | { kind: "error"; code: SkillErrorCode; uncertain: boolean };

function errorCode(error: unknown): SkillErrorCode {
    return error instanceof SkillRequestError ? error.code : "operation_failed";
}

/** Failures after which the stored revision may already differ from the input. */
function uncertain(code: SkillErrorCode): boolean {
    return [
        "operation_failed",
        "invalid_response",
        "outcome_unknown",
        "write_failed",
        "storage_unavailable",
        "revision_conflict",
    ].includes(code);
}

/**
 * Choose the unit for a skill's size and format the number. A skill can hold
 * tens of megabytes, so kilobytes alone would be hard to read; the unit text
 * comes from the translation resources.
 */
function formatSize(
    bytes: number,
    locale: string,
): { key: "skills.sizeKb" | "skills.sizeMb"; value: string } {
    const number = new Intl.NumberFormat(locale, {
        maximumFractionDigits: 1,
    });
    if (bytes >= 1024 * 1024) {
        return {
            key: "skills.sizeMb",
            value: number.format(bytes / (1024 * 1024)),
        };
    }
    return { key: "skills.sizeKb", value: number.format(bytes / 1024) };
}

/** Inputs for {@link SkillDetail}. */
export interface SkillDetailProps {
    record: SkillRecord;
    onSaved: (record: SkillRecord) => void;
    onCancel: () => void;
    /** Re-read the record after an unknown outcome. */
    onReload: () => void;
    /** Protect sidebar navigation while a save is pending or unresolved. */
    onNavigationBlockedChange?: (blocked: boolean) => void;
}

/**
 * Everything the library holds about one skill: the imported facts, and the
 * source fields the user can record or clear. A recorded address is metadata for
 * a later update check, so nothing is downloaded, cloned or executed here.
 * The content digest stays out of the page: comparing hashes is a machine task,
 * and the update check is where a result becomes something a user can act on.
 */
export function SkillDetail({
    record,
    onSaved,
    onCancel,
    onReload,
    onNavigationBlockedChange,
}: SkillDetailProps): JSX.Element {
    const { t, i18n } = useTranslation();
    const prefix = useId();
    const formId = `${prefix}-form`;
    const [url, setUrl] = useState(record.sourceUrl ?? "");
    const [revision, setRevision] = useState(record.sourceRevision ?? "");
    const [state, setState] = useState<FormState>({ kind: "idle" });
    const active = useRef(true);
    const mutationPending = useRef(false);
    const busy = state.kind === "saving";
    const size = formatSize(record.byteSize, i18n.language);
    const source = skillSource(record);
    const sourceText =
        source.kind === "address"
            ? `${source.url}${
                  source.revision === null
                      ? ""
                      : ` · ${t("skills.sourceRevision", { revision: source.revision })}`
              }`
            : t(
                  source.kind === "local"
                      ? "skills.sourceLocal"
                      : "skills.sourceUnknownValue",
              );
    // A revision is only meaningful together with an address; the backend rejects
    // the pair, and the field explains it before a request is sent.
    const revisionWithoutUrl = url.trim() === "" && revision.trim() !== "";

    useEffect(() => {
        active.current = true;
        return () => {
            active.current = false;
        };
    }, []);
    useEffect(() => {
        onNavigationBlockedChange?.(
            state.kind === "saving" ||
                (state.kind === "error" && state.uncertain),
        );
        return () => {
            onNavigationBlockedChange?.(false);
        };
    }, [onNavigationBlockedChange, state]);
    async function submit(event: SubmitEvent<HTMLFormElement>): Promise<void> {
        event.preventDefault();
        if (busy || mutationPending.current || revisionWithoutUrl) return;
        const trimmedUrl = url.trim();
        mutationPending.current = true;
        setState({ kind: "saving" });
        try {
            const saved = await setSkillSource({
                id: record.id,
                expectedRevision: record.revision,
                url: trimmedUrl === "" ? null : trimmedUrl,
                revision: trimmedUrl === "" ? null : revision.trim() || null,
            });
            if (active.current) onSaved(saved);
        } catch (error: unknown) {
            if (active.current) {
                const code = errorCode(error);
                setState({ kind: "error", code, uncertain: uncertain(code) });
            }
        } finally {
            mutationPending.current = false;
        }
    }
    const date = (milliseconds: number): string =>
        new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" }).format(
            new Date(milliseconds),
        );
    return (
        <ProviderPage
            title={t("skills.detailTitle")}
            onBack={onCancel}
            backBlocked={busy}
            backAriaLabel={t("skills.backLabel")}
            scrollContent
        >
            {/* Cards are grouped with the same 10px gap as the Agent details. */}
            <div className={styles["detailGroup"]}>
                <DetailPanel label={t("skills.factsTitle")}>
                    <DetailFact
                        label={t("skills.factName")}
                        value={record.name}
                    />
                    <div className={detailForm["row"]}>
                        <span className={detailForm["label"]}>
                            {t("skills.factDescription")}
                        </span>
                        <p className={styles["longValue"]}>
                            {record.description}
                        </p>
                    </div>
                    <DetailFact
                        label={t("skills.factFiles")}
                        value={t("skills.files", { count: record.fileCount })}
                    />
                    <DetailFact
                        label={t("skills.factSize")}
                        value={t(size.key, { size: size.value })}
                    />
                    <DetailFact
                        label={t("skills.factOrigin")}
                        value={
                            record.originPath ?? t("skills.factOriginUnknown")
                        }
                    />
                    <DetailFact
                        label={t("skills.factSource")}
                        value={sourceText}
                    />
                    <DetailFact
                        label={t("skills.factImportedAt")}
                        value={date(record.importedAtMs)}
                    />
                    <DetailFact
                        label={t("skills.factUpdatedAt")}
                        value={date(record.updatedAtMs)}
                    />
                </DetailPanel>
                <form
                    className={styles["sourceForm"]}
                    id={formId}
                    aria-label={t("skills.sourceTitle")}
                    onSubmit={(event) => {
                        void submit(event);
                    }}
                >
                    <div
                        className={[
                            editForm["group"],
                            editForm["groupFlush"],
                        ].join(" ")}
                    >
                        <EditFormRow
                            label={t("skills.urlLabel")}
                            htmlFor={`${prefix}-url`}
                        >
                            <input
                                id={`${prefix}-url`}
                                className={editForm["input"]}
                                value={url}
                                readOnly={busy}
                                autoComplete="off"
                                spellCheck={false}
                                placeholder={t("skills.urlPlaceholder")}
                                onChange={(event) => {
                                    setUrl(event.currentTarget.value);
                                }}
                            />
                        </EditFormRow>
                        <EditFormRow
                            label={t("skills.revisionLabel")}
                            htmlFor={`${prefix}-revision`}
                        >
                            <input
                                id={`${prefix}-revision`}
                                className={editForm["input"]}
                                value={revision}
                                readOnly={busy}
                                autoComplete="off"
                                spellCheck={false}
                                placeholder={t("skills.revisionPlaceholder")}
                                aria-invalid={revisionWithoutUrl}
                                // Only the validation message describes this field.
                                aria-describedby={
                                    revisionWithoutUrl
                                        ? `${prefix}-revision-hint`
                                        : undefined
                                }
                                onChange={(event) => {
                                    setRevision(event.currentTarget.value);
                                }}
                            />
                        </EditFormRow>
                    </div>
                    {/* A revision without an address cannot be saved, and this
                    is the only hint the source fields need. */}
                    {revisionWithoutUrl && (
                        <p
                            id={`${prefix}-revision-hint`}
                            className={styles["hint"]}
                        >
                            {t("skills.revisionNeedsUrl")}
                        </p>
                    )}
                    {state.kind === "error" && (
                        <>
                            <p className={styles["error"]} role="alert">
                                {t(`skills.errors.${state.code}`)}
                            </p>
                            {state.uncertain && (
                                <p className={styles["hint"]}>
                                    {t("skills.reloadUnknown")}
                                </p>
                            )}
                        </>
                    )}
                    <p className={styles["hint"]}>{t("skills.notDeployed")}</p>
                    <SecondaryPageActions>
                        <div className={styles["sourceActions"]}>
                            {state.kind === "error" && state.uncertain && (
                                <button
                                    className={buttons["secondary"]}
                                    type="button"
                                    onClick={onReload}
                                >
                                    {t("skills.reload")}
                                </button>
                            )}
                            <button
                                className={buttons["primary"]}
                                type="submit"
                                form={formId}
                                aria-disabled={busy || revisionWithoutUrl}
                            >
                                {busy ? t("skills.saving") : t("skills.save")}
                            </button>
                        </div>
                    </SecondaryPageActions>
                </form>
            </div>
        </ProviderPage>
    );
}
