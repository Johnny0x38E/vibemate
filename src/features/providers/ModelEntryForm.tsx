import { Dialog } from "@base-ui/react/dialog";
import {
    useCallback,
    useEffect,
    useId,
    useRef,
    useState,
    type JSX,
    type RefObject,
} from "react";
import { useTranslation } from "react-i18next";
import {
    addManualProviderModel,
    getProviderModelSnapshot,
    listProviderModels,
    updateProviderModelEdit,
    EFFORT_LEVELS,
    ModelRequestError,
    type CapabilityOverride,
    type CapabilityOverrides,
    type EffortLevel,
    type ModelCapabilities,
    type ModelErrorCode,
    type ModelSnapshot,
    type ProviderModel,
    type ThinkingEffort,
} from "../../lib/desktop/models";
import { EditFormRow } from "../../components/EditFormRow";
import { ModelCheckbox } from "../../components/ModelCheckbox";
import { EFFORT_CHOICES, EffortLevelSelect } from "./EffortLevelSelect";
import { TokenStopSlider } from "./TokenStopSlider";
import { nearestTokenStop } from "./tokenStops";
import {
    capabilityDeclaration,
    effectiveCapabilityState,
    hasCapabilityDeclarations,
    NO_CAPABILITY_DECLARATIONS,
    UNKNOWN_CAPABILITIES,
    type ModelCapabilityName,
} from "./modelCapabilities";
import editForm from "../../components/editForm.module.css";
import buttons from "../../components/actionButtons.module.css";
import styles from "./ModelEntryForm.module.css";

/** Fields shared by both modes of {@link ModelEntryForm}. */
interface ModelEntrySharedProps {
    providerId: string;
    /** The parent blocks the form while another operation owns the section. */
    blocked: boolean;
    /** Receives the saved row; the parent closes the dialog and returns focus. */
    onSaved: (model: ProviderModel) => void;
    /** Close without saving. Escape uses the same path when a write is not in flight. */
    onDismiss: () => void;
    /** Reports in-flight reads and writes for the parent's busy guard. */
    onBusyChange: (busy: boolean) => void;
    /** Element that opened the dialog; focus returns here when it closes. */
    returnFocus?: RefObject<HTMLElement | null>;
}

/** Inputs for {@link ModelEntryForm}: creation or editing, never both. */
export type ModelEntryFormProps =
    | (ModelEntrySharedProps & { mode: "create" })
    | (ModelEntrySharedProps & {
          mode: "edit";
          /** Identity of the saved row; model IDs are never renamed. */
          modelId: string;
      });

type Phase =
    | "loading" // edit: reading the saved snapshot
    | "ready"
    | "saving"
    | "checking" // create: verifying an unknown write by exact model ID
    | "uncertain" // unknown write outcome
    | "conflict"; // edit: stale snapshot, or a read that could not be recovered

const CAPABILITY_NAMES: readonly ModelCapabilityName[] = [
    "thinking",
    "imageInput",
    "toolCalls",
];

/** Levels to show. "Any gateway level" arrives as every choice selected. */
function levelsFromEffort(effort: ThinkingEffort | null): EffortLevel[] {
    if (effort === null) return [];
    if (effort.kind === "any") return [...EFFORT_CHOICES];
    return EFFORT_CHOICES.filter((level) => effort.levels.includes(level));
}

function effortPayload(
    thinkingChecked: boolean,
    levels: readonly EffortLevel[],
): ThinkingEffort | null {
    if (!thinkingChecked || levels.length === 0) return null;
    return {
        kind: "listed",
        levels: EFFORT_LEVELS.filter((level) => levels.includes(level)),
        defaultLevel: null,
    };
}

/**
 * Dialog for one model entry, in two modes: **create** saves a new manual row
 * through Rust, **edit** updates an existing row's alias, capability
 * declarations, context window, maximum output and thinking effort behind the
 * snapshot's content digest. The three limit fields are the row's final state:
 * an empty field clears a value a catalog fetch had written. Unknown write
 * outcomes are never reported as saved. Unmounting ignores late replies. This
 * component never downloads a catalog or reads a credential.
 */
export function ModelEntryForm(props: ModelEntryFormProps): JSX.Element {
    const { t } = useTranslation();
    const {
        providerId,
        blocked,
        onSaved,
        onDismiss,
        onBusyChange,
        returnFocus,
    } = props;
    const editedId = props.mode === "edit" ? props.modelId : null;
    const editing = editedId !== null;

    const id = useId();
    const modelInput = useRef<HTMLInputElement>(null);
    const aliasInput = useRef<HTMLInputElement>(null);
    // While the reasoning menu is open, Escape closes that menu rather than the dialog.
    const effortMenuOpen = useRef(false);
    const reportEffortMenu = useCallback((open: boolean) => {
        effortMenuOpen.current = open;
    }, []);
    // One counter owns every read and write reply: it is bumped when an edit
    // (re)loads its snapshot and on unmount, so late replies are never applied.
    const generation = useRef(0);
    const pending = useRef(false);
    const [modelId, setModelId] = useState(editedId ?? "");
    const [alias, setAlias] = useState("");
    const [contextWindow, setContextWindow] = useState<number | null>(null);
    const [maxOutput, setMaxOutput] = useState<number | null>(null);
    const [levels, setLevels] = useState<EffortLevel[]>([]);
    const [overrides, setOverrides] = useState<CapabilityOverrides>(
        NO_CAPABILITY_DECLARATIONS,
    );
    const [evidence, setEvidence] =
        useState<ModelCapabilities>(UNKNOWN_CAPABILITIES);
    const [digest, setDigest] = useState<string | null>(null);
    const [phase, setPhase] = useState<Phase>(editing ? "loading" : "ready");
    const [error, setError] = useState<ModelErrorCode | null>(null);
    const [reload, setReload] = useState(0);
    const busy =
        phase === "loading" || phase === "saving" || phase === "checking";
    const uncertain = phase === "uncertain" || phase === "checking";
    // A conflicted edit freezes its fields until the snapshot is reloaded.
    const locked = blocked || busy || uncertain || phase === "conflict";
    const idError =
        error === "model_id_invalid" || error === "model_already_exists";
    const aliasError = error === "model_alias_invalid";
    const canDismiss = !busy && !uncertain && phase !== "conflict";
    const thinkingChecked =
        effectiveCapabilityState(
            evidence.thinking,
            capabilityDeclaration(overrides, "thinking"),
        ) === "supported";

    // Edit mode reads its snapshot (values plus change token) on every mount and
    // explicit reload. Creation needs no read. The "loading" phase is entered by
    // the initial state or by the reload button, never here: effects must not
    // synchronously re-trigger renders.
    useEffect(() => {
        if (editedId === null) return;
        const current = ++generation.current;
        void getProviderModelSnapshot({ providerId, modelId: editedId })
            .then((loaded: ModelSnapshot) => {
                if (current !== generation.current) return;
                setModelId(loaded.model.modelId);
                setAlias(loaded.model.alias ?? "");
                setContextWindow(nearestTokenStop(loaded.model.contextWindow));
                setMaxOutput(nearestTokenStop(loaded.model.maxOutputTokens));
                setLevels(levelsFromEffort(loaded.model.thinkingEffort));
                setOverrides(loaded.model.capabilityOverrides);
                setEvidence(loaded.model.capabilities);
                setDigest(loaded.contentDigest);
                setError(null);
                setPhase("ready");
            })
            .catch((reason: unknown) => {
                if (current !== generation.current) return;
                setError(
                    reason instanceof ModelRequestError
                        ? reason.code
                        : "operation_failed",
                );
                setPhase("conflict");
            });
    }, [providerId, editedId, reload]);

    // Unmounting invalidates every in-flight reply and releases the parent's
    // busy guard, so a hidden form cannot block the section forever.
    useEffect(() => {
        return () => {
            generation.current += 1;
            onBusyChange(false);
        };
    }, [onBusyChange]);

    // Report in-flight reads and writes to the parent's busy guard.
    useEffect(() => {
        onBusyChange(busy);
    }, [busy, onBusyChange]);

    // In edit mode the alias field is the first editable control, so it takes
    // focus once the snapshot arrives, not on every keystroke.
    useEffect(() => {
        if (editing && phase === "ready") aliasInput.current?.focus();
    }, [editing, phase]);

    // Escape closes the dialog from any field. A write in flight, an unknown
    // outcome, or a stale snapshot keeps the dialog open so the draft is not lost.
    // An open reasoning menu consumes the first Escape; the ref is set by that menu.
    useEffect(() => {
        function onKey(event: KeyboardEvent): void {
            if (event.key !== "Escape" || !canDismiss || effortMenuOpen.current)
                return;
            event.preventDefault();
            onDismiss();
        }
        document.addEventListener("keydown", onKey);
        return () => {
            document.removeEventListener("keydown", onKey);
        };
    }, [canDismiss, onDismiss]);

    // Every checkbox interaction is a user declaration in both directions:
    // checking declares "supported", clearing declares "not supported". Only
    // "restore default" removes a declaration again.
    function declare(
        name: ModelCapabilityName,
        value: CapabilityOverride,
    ): void {
        setOverrides((current) => ({
            thinking: name === "thinking" ? value : current.thinking,
            imageInput: name === "imageInput" ? value : current.imageInput,
            toolCalls: name === "toolCalls" ? value : current.toolCalls,
        }));
        if (name === "thinking" && value === "unsupported") setLevels([]);
        setError(null);
    }

    function saved(model: ProviderModel): void {
        setPhase("ready");
        setError(null);
        if (!editing) {
            setModelId("");
            setAlias("");
            setContextWindow(null);
            setMaxOutput(null);
            setLevels([]);
            setOverrides(NO_CAPABILITY_DECLARATIONS);
        }
        onSaved(model);
    }

    function setEffortLevels(next: readonly EffortLevel[]): void {
        setLevels(EFFORT_CHOICES.filter((level) => next.includes(level)));
        setError(null);
    }

    async function submit(): Promise<void> {
        if (locked || pending.current) return;
        pending.current = true;
        const current = generation.current;
        setPhase("saving");
        setError(null);
        const thinking = effortPayload(thinkingChecked, levels);
        try {
            const model = editing
                ? (
                      await updateProviderModelEdit({
                          providerId,
                          // Phase "ready" in edit mode guarantees both values.
                          modelId: editedId,
                          expectedDigest: digest ?? "",
                          alias: alias.trim() === "" ? null : alias,
                          capabilityOverrides: overrides,
                          contextWindow,
                          maxOutputTokens: maxOutput,
                          thinkingEffort: thinking,
                      })
                  ).model
                : await addManualProviderModel({
                      providerId,
                      modelId,
                      alias: alias.trim() === "" ? null : alias,
                      capabilityOverrides: overrides,
                      contextWindow,
                      maxOutputTokens: maxOutput,
                      thinkingEffort: thinking,
                  });
            if (current !== generation.current) return;
            saved(model);
        } catch (reason: unknown) {
            if (current !== generation.current) return;
            const code =
                reason instanceof ModelRequestError
                    ? reason.code
                    : "operation_failed";
            setError(code);
            setPhase(
                code === "operation_failed" || code === "invalid_response"
                    ? "uncertain"
                    : code === "model_changed"
                      ? "conflict"
                      : "ready",
            );
        } finally {
            pending.current = false;
        }
    }

    // Creation resolves an unknown write by reading every cursor page and
    // matching the exact model ID; a fuzzy look-alike proves nothing.
    async function checkResult(): Promise<void> {
        if (blocked || pending.current) return;
        pending.current = true;
        const current = generation.current;
        setPhase("checking");
        try {
            let after: string | null = null;
            for (;;) {
                const result = await listProviderModels({
                    providerId,
                    after,
                    limit: 200,
                    filter: "all",
                });
                if (current !== generation.current) return;
                if (result.kind === "preview") {
                    setPhase("uncertain");
                    return;
                }
                const found = result.page.items.find(
                    (row) => row.modelId === modelId.trim(),
                );
                if (found !== undefined) {
                    if (found.source === "manual" && found.selected) {
                        saved(found);
                    } else {
                        setError("model_already_exists");
                        setPhase("ready");
                    }
                    return;
                }
                after = result.page.nextCursor;
                if (after === null) break;
            }
            setPhase("ready");
        } catch {
            if (current !== generation.current) return;
            setPhase("uncertain");
        } finally {
            pending.current = false;
        }
    }

    return (
        <Dialog.Root
            open
            onOpenChange={(nextOpen) => {
                if (!nextOpen && canDismiss) onDismiss();
            }}
        >
            <Dialog.Portal>
                <Dialog.Backdrop className={styles["backdrop"]} />
                <Dialog.Popup
                    className={styles["popup"]}
                    initialFocus={() =>
                        editing ? aliasInput.current : modelInput.current
                    }
                    finalFocus={() => returnFocus?.current ?? null}
                >
                    <Dialog.Title className={styles["title"]}>
                        {editing
                            ? t("providers.models.editor.title", {
                                  modelId: editedId,
                              })
                            : t("providers.models.manual.title")}
                    </Dialog.Title>
                    <form
                        className={styles["form"]}
                        aria-label={
                            editing
                                ? t("providers.models.editor.title", {
                                      modelId: editedId,
                                  })
                                : t("providers.models.manual.title")
                        }
                        aria-busy={busy}
                        onSubmit={(event) => {
                            event.preventDefault();
                            void submit();
                        }}
                    >
                        {editing ? (
                            <EditFormRow
                                className={styles["dialogRow"]}
                                label={t("providers.models.manual.modelId")}
                            >
                                <p className={styles["modelIdText"]}>
                                    {modelId}
                                </p>
                            </EditFormRow>
                        ) : (
                            <EditFormRow
                                className={styles["dialogRow"]}
                                label={t("providers.models.manual.modelId")}
                                htmlFor={`${id}-model`}
                            >
                                <input
                                    ref={modelInput}
                                    className={editForm["input"]}
                                    id={`${id}-model`}
                                    value={modelId}
                                    readOnly={locked}
                                    aria-disabled={locked}
                                    aria-invalid={idError}
                                    aria-describedby={
                                        idError ? `${id}-error` : undefined
                                    }
                                    autoComplete="off"
                                    spellCheck={false}
                                    onChange={(event) => {
                                        setModelId(event.currentTarget.value);
                                        setError(null);
                                    }}
                                />
                            </EditFormRow>
                        )}
                        <EditFormRow
                            className={styles["dialogRow"]}
                            label={t("providers.models.manual.alias")}
                            htmlFor={`${id}-alias`}
                        >
                            <input
                                ref={aliasInput}
                                className={editForm["input"]}
                                id={`${id}-alias`}
                                placeholder={t(
                                    "providers.models.manual.aliasPlaceholder",
                                )}
                                value={alias}
                                readOnly={locked}
                                aria-disabled={locked}
                                aria-invalid={aliasError}
                                aria-describedby={
                                    aliasError ? `${id}-error` : undefined
                                }
                                autoComplete="off"
                                onChange={(event) => {
                                    setAlias(event.currentTarget.value);
                                    setError(null);
                                }}
                            />
                        </EditFormRow>
                        <EditFormRow
                            className={styles["dialogRow"]}
                            label={t(
                                "providers.models.parameters.contextWindow",
                            )}
                            htmlFor={`${id}-context`}
                        >
                            <TokenStopSlider
                                id={`${id}-context`}
                                value={contextWindow}
                                disabled={locked}
                                onChange={setContextWindow}
                            />
                        </EditFormRow>
                        <EditFormRow
                            className={styles["dialogRow"]}
                            label={t("providers.models.parameters.maxOutput")}
                            htmlFor={`${id}-output`}
                        >
                            <TokenStopSlider
                                id={`${id}-output`}
                                value={maxOutput}
                                disabled={locked}
                                onChange={setMaxOutput}
                            />
                        </EditFormRow>
                        <EditFormRow
                            className={styles["dialogRow"]}
                            label={t(
                                "providers.models.parameters.capabilities",
                            )}
                        >
                            <div className={styles["capabilities"]}>
                                {CAPABILITY_NAMES.map((name) => {
                                    const declaration = capabilityDeclaration(
                                        overrides,
                                        name,
                                    );
                                    return (
                                        <div
                                            key={name}
                                            className={styles["capability"]}
                                        >
                                            <ModelCheckbox
                                                checked={
                                                    effectiveCapabilityState(
                                                        evidence[name],
                                                        declaration,
                                                    ) === "supported"
                                                }
                                                disabled={locked}
                                                ariaLabel={t(
                                                    `providers.models.capabilities.${name}`,
                                                )}
                                                onCheckedChange={(next) => {
                                                    declare(
                                                        name,
                                                        next
                                                            ? "supported"
                                                            : "unsupported",
                                                    );
                                                }}
                                            />
                                            <span
                                                className={
                                                    styles["capabilityName"]
                                                }
                                            >
                                                {t(
                                                    `providers.models.capabilities.${name}`,
                                                )}
                                            </span>
                                        </div>
                                    );
                                })}
                            </div>
                        </EditFormRow>
                        {thinkingChecked && (
                            <EditFormRow
                                className={styles["dialogRow"]}
                                label={t("providers.models.parameters.effort")}
                                htmlFor={`${id}-effort`}
                            >
                                <EffortLevelSelect
                                    id={`${id}-effort`}
                                    value={levels}
                                    disabled={locked}
                                    onChange={setEffortLevels}
                                    onOpenChange={reportEffortMenu}
                                />
                            </EditFormRow>
                        )}
                        <div className={styles["actions"]}>
                            <button
                                className={buttons["secondary"]}
                                type="button"
                                disabled={
                                    locked ||
                                    !hasCapabilityDeclarations(overrides)
                                }
                                onClick={() => {
                                    setOverrides(NO_CAPABILITY_DECLARATIONS);
                                    if (evidence.thinking.state !== "supported")
                                        setLevels([]);
                                    setError(null);
                                }}
                            >
                                {t("providers.models.editor.restoreDefault")}
                            </button>
                            <button
                                className={buttons["primary"]}
                                type="submit"
                                aria-disabled={locked}
                            >
                                {t(
                                    editing
                                        ? "providers.models.editor.save"
                                        : "providers.models.manual.add",
                                )}
                            </button>
                            <button
                                className={buttons["secondary"]}
                                type="button"
                                disabled={!canDismiss}
                                onClick={onDismiss}
                            >
                                {t("providers.models.editor.cancel")}
                            </button>
                        </div>
                        <div className={styles["feedback"]}>
                            {phase === "loading" && (
                                <p role="status">
                                    {t("providers.models.loading")}
                                </p>
                            )}
                            {phase === "saving" && (
                                <p
                                    role="status"
                                    className={editForm["message"]}
                                >
                                    {t("providers.models.saving")}
                                </p>
                            )}
                            {error !== null && !uncertain && (
                                <p
                                    id={`${id}-error`}
                                    className={editForm["error"]}
                                    role="alert"
                                >
                                    {t(`providers.models.errors.${error}`)}
                                </p>
                            )}
                            {uncertain && (
                                <p className={editForm["error"]} role="alert">
                                    {t(
                                        editing
                                            ? "providers.models.editor.uncertain"
                                            : "providers.models.manual.uncertain",
                                    )}
                                </p>
                            )}
                            {uncertain && !editing && (
                                <button
                                    className={buttons["secondary"]}
                                    type="button"
                                    aria-disabled={blocked || busy}
                                    onClick={() => {
                                        void checkResult();
                                    }}
                                >
                                    {t("providers.models.manual.check")}
                                </button>
                            )}
                            {(phase === "conflict" ||
                                (uncertain && editing)) && (
                                <button
                                    className={buttons["secondary"]}
                                    type="button"
                                    onClick={() => {
                                        setPhase("loading");
                                        setError(null);
                                        setReload((value) => value + 1);
                                    }}
                                >
                                    {t("providers.models.editor.reload")}
                                </button>
                            )}
                        </div>
                    </form>
                </Dialog.Popup>
            </Dialog.Portal>
        </Dialog.Root>
    );
}
