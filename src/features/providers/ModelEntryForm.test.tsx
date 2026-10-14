import {
    act,
    cleanup,
    fireEvent,
    render,
    screen,
    waitFor,
    within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import {
    addManualProviderModel,
    getProviderModelSnapshot,
    listProviderModels,
    updateProviderModelEdit,
    ModelRequestError,
    type ModelSnapshot,
    type ProviderModel,
} from "../../lib/desktop/models";
import { ModelEntryForm } from "./ModelEntryForm";

vi.mock(import("../../lib/desktop/models"), async (original) => ({
    ...(await original()),
    addManualProviderModel: vi.fn(),
    getProviderModelSnapshot: vi.fn(),
    listProviderModels: vi.fn(),
    updateProviderModelEdit: vi.fn(),
}));
const add = vi.mocked(addManualProviderModel);
const get = vi.mocked(getProviderModelSnapshot);
const list = vi.mocked(listProviderModels);
const update = vi.mocked(updateProviderModelEdit);

const ID = "0123456789abcdef0123456789abcdef";
const row: ProviderModel = {
    providerId: ID,
    modelId: "vendor/model",
    source: "manual",
    selected: true,
    alias: null,
    upstreamName: null,
    contextWindow: null,
    maxOutputTokens: null,
    inputModalities: null,
    outputModalities: null,
    supportedEndpoints: null,
    capabilities: {
        thinking: { state: "unknown", source: null },
        imageInput: { state: "unknown", source: null },
        toolCalls: { state: "unknown", source: null },
    },
    capabilityOverrides: { thinking: null, imageInput: null, toolCalls: null },
    thinkingEffort: null,
    routeSupport: "unknown",
    resolvedProtocol: null,
    upstreamState: "never_listed",
    lastSeenAtMs: null,
    missingSinceMs: null,
    createdAtMs: 1000,
    updatedAtMs: 1000,
};

async function setupCreate(locale: "en" | "zh-CN" = "en", blocked = false) {
    const i18n = await createAppI18n(locale);
    const saved = vi.fn();
    const view = render(
        <StrictMode>
            <I18nextProvider i18n={i18n}>
                <ModelEntryForm
                    mode="create"
                    providerId={ID}
                    blocked={blocked}
                    onSaved={saved}
                    onDismiss={vi.fn()}
                    onBusyChange={vi.fn()}
                />
            </I18nextProvider>
        </StrictMode>,
    );
    return { ...view, saved, i18n };
}

async function setupEdit(locale: "en" | "zh-CN" = "en") {
    const i18n = await createAppI18n(locale);
    const saved = vi.fn();
    const cancel = vi.fn();
    const view = render(
        <StrictMode>
            <I18nextProvider i18n={i18n}>
                <ModelEntryForm
                    mode="edit"
                    providerId={ID}
                    blocked={false}
                    modelId={row.modelId}
                    onSaved={saved}
                    onDismiss={cancel}
                    onBusyChange={vi.fn()}
                />
            </I18nextProvider>
        </StrictMode>,
    );
    return { ...view, saved, cancel, i18n };
}

function field(name: string): HTMLInputElement {
    return screen.getByLabelText<HTMLInputElement>(name);
}

function checkbox(name: string): HTMLElement {
    return screen.getByRole("checkbox", { name });
}

function checked(name: string): boolean {
    return checkbox(name).getAttribute("aria-checked") === "true";
}

function page(items: ProviderModel[]) {
    return {
        kind: "desktop" as const,
        page: { items, nextCursor: null, totalMatches: items.length },
    };
}

beforeEach(() => {
    add.mockReset().mockResolvedValue(row);
    get.mockReset().mockResolvedValue({
        model: row,
        contentDigest: "a".repeat(64),
    });
    list.mockReset();
    update.mockReset();
});
afterEach(cleanup);

test("creates a manual row with declarations and clears the form", async () => {
    const { saved } = await setupCreate();
    fireEvent.change(field("Model ID"), {
        target: { value: " vendor/model " },
    });
    fireEvent.change(field("Alias"), {
        target: { value: "Work" },
    });
    fireEvent.click(checkbox("Reasoning"));
    expect(checked("Reasoning")).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Add and save" }));
    await waitFor(() => {
        expect(saved).toHaveBeenCalledWith(row);
    });
    expect(add).toHaveBeenCalledWith({
        providerId: ID,
        modelId: " vendor/model ",
        alias: "Work",
        capabilityOverrides: {
            thinking: "supported",
            imageInput: null,
            toolCalls: null,
        },
        contextWindow: null,
        maxOutputTokens: null,
        thinkingEffort: null,
    });
    // The form resets for the next creation, declarations included.
    expect(field("Model ID").value).toBe("");
    expect(field("Alias").value).toBe("");
    expect(checked("Reasoning")).toBe(false);
});

test("saves preset limits and reasoning levels only after reasoning is checked", async () => {
    const user = userEvent.setup();
    await setupCreate();
    expect(screen.getAllByText("Not set").length).toBeGreaterThan(0);
    expect(screen.queryByText("providers.models.parameters.unset")).toBeNull();
    fireEvent.change(field("Model ID"), { target: { value: "vendor/model" } });
    expect(
        screen.queryByRole("combobox", { name: "Reasoning effort" }),
    ).toBeNull();
    fireEvent.click(checkbox("Reasoning"));
    const effort = screen.getByRole("combobox", { name: "Reasoning effort" });
    await user.click(effort);
    const choices = [
        "none",
        "minimal",
        "low",
        "medium",
        "high",
        "xhigh",
        "max",
    ];
    const list = await screen.findByRole("listbox");
    expect(
        within(list)
            .getAllByRole("option")
            .map((option) => option.getAttribute("data-value")),
    ).toEqual(choices);
    await user.click(within(list).getByRole("option", { name: "none" }));
    expect(effort.getAttribute("aria-expanded")).toBe("true");
    await user.click(within(list).getByRole("option", { name: "max" }));
    expect(screen.getByRole("button", { name: "Remove none" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Remove none" }));
    expect(screen.queryByRole("button", { name: "Remove none" })).toBeNull();
    expect(screen.getByRole("button", { name: "Remove max" })).toBeTruthy();
    fireEvent.change(screen.getByRole("slider", { name: "Context window" }), {
        target: { value: "2" },
    });
    fireEvent.change(screen.getByRole("slider", { name: "Maximum output" }), {
        target: { value: "5" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add and save" }));
    await waitFor(() => {
        expect(add).toHaveBeenCalledWith(
            expect.objectContaining({
                contextWindow: 128 * 1024,
                maxOutputTokens: 1024 * 1024,
                thinkingEffort: {
                    kind: "listed",
                    levels: ["max"],
                    defaultLevel: null,
                },
            }),
        );
    });
});

test("normalizes an empty optional alias without guessing model validation", async () => {
    await setupCreate();
    fireEvent.change(field("Model ID"), { target: { value: "vendor/model" } });
    fireEvent.change(field("Alias"), { target: { value: "  " } });
    fireEvent.click(screen.getByRole("button", { name: "Add and save" }));
    await waitFor(() => {
        expect(add).toHaveBeenCalledWith(
            expect.objectContaining({ alias: null }),
        );
    });
});

test("blocks duplicate submits and ignores a write reply after unmount", async () => {
    const { saved, unmount } = await setupCreate();
    fireEvent.change(field("Model ID"), { target: { value: "vendor/model" } });
    let finish: ((value: ProviderModel) => void) | undefined;
    add.mockImplementation(
        () =>
            new Promise((resolve) => {
                finish = resolve;
            }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Add and save" }));
    fireEvent.click(screen.getByRole("button", { name: "Add and save" }));
    expect(add).toHaveBeenCalledTimes(1);
    unmount();
    await act(async () => {
        finish?.(row);
        await Promise.resolve();
    });
    expect(saved).not.toHaveBeenCalled();
});

test("unknown writes require a successful exact-ID check before another creation", async () => {
    const { saved } = await setupCreate();
    fireEvent.change(field("Model ID"), { target: { value: "vendor/model" } });
    add.mockRejectedValueOnce(new ModelRequestError("invalid_response"));
    fireEvent.click(screen.getByRole("button", { name: "Add and save" }));
    expect((await screen.findByRole("alert")).textContent).toContain(
        "save result is unknown",
    );
    // A fuzzy look-alike proves nothing, and the exact row on a later page does.
    list.mockResolvedValueOnce({
        kind: "desktop",
        page: {
            items: [{ ...row, modelId: "vendor/model-2" }],
            nextCursor: "next",
            totalMatches: 2,
        },
    });
    list.mockResolvedValueOnce(page([row]));
    fireEvent.click(screen.getByRole("button", { name: "Check saved models" }));
    await waitFor(() => {
        expect(saved).toHaveBeenCalledWith(row);
    });
    expect(list).toHaveBeenCalledTimes(2);
});

test("a completed check without the row permits retrying the creation", async () => {
    const { saved } = await setupCreate();
    fireEvent.change(field("Model ID"), { target: { value: "vendor/model" } });
    add.mockRejectedValueOnce(new ModelRequestError("operation_failed"));
    fireEvent.click(screen.getByRole("button", { name: "Add and save" }));
    await screen.findByRole("alert");
    list.mockResolvedValueOnce(page([]));
    fireEvent.click(screen.getByRole("button", { name: "Check saved models" }));
    await waitFor(() => {
        expect(
            screen
                .getByRole("button", { name: "Add and save" })
                .getAttribute("aria-disabled"),
        ).toBe("false");
    });
    expect(saved).not.toHaveBeenCalled();
    add.mockResolvedValueOnce(row);
    fireEvent.click(screen.getByRole("button", { name: "Add and save" }));
    await waitFor(() => {
        expect(saved).toHaveBeenCalledWith(row);
    });
});

test("blocked state makes no write even on form submission", async () => {
    await setupCreate("en", true);
    fireEvent.change(field("Model ID"), { target: { value: "vendor/model" } });
    fireEvent.click(screen.getByRole("button", { name: "Add and save" }));
    expect(add).not.toHaveBeenCalled();
});

test("loads the saved row, focuses the alias and saves exactly that snapshot", async () => {
    const { saved } = await setupEdit();
    const alias = field("Alias");
    await waitFor(() => {
        expect(alias.readOnly).toBe(false);
    });
    // Focus follows the ready phase through an effect.
    await waitFor(() => {
        expect(document.activeElement).toBe(alias);
    });
    expect(screen.getByText("vendor/model")).toBeTruthy();
    expect(screen.queryByRole("textbox", { name: "Model ID" })).toBeNull();
    fireEvent.change(alias, { target: { value: "My model" } });
    const next: ModelSnapshot = {
        contentDigest: "b".repeat(64),
        model: { ...row, alias: "My model" },
    };
    update.mockResolvedValue(next);
    await userEvent.setup().keyboard("{Enter}");
    await waitFor(() => {
        expect(saved).toHaveBeenCalledWith(next.model);
    });
    expect(update).toHaveBeenCalledWith({
        providerId: ID,
        modelId: "vendor/model",
        expectedDigest: "a".repeat(64),
        alias: "My model",
        capabilityOverrides: {
            thinking: null,
            imageInput: null,
            toolCalls: null,
        },
        contextWindow: null,
        maxOutputTokens: null,
        thinkingEffort: null,
    });
});

test("every checkbox interaction declares and restore default clears the declarations", async () => {
    const declared: ModelSnapshot = {
        contentDigest: "a".repeat(64),
        model: {
            ...row,
            capabilities: {
                thinking: {
                    state: "supported",
                    source: "openrouter_model_reasoning",
                },
                imageInput: { state: "unknown", source: null },
                toolCalls: { state: "unknown", source: null },
            },
        },
    };
    get.mockResolvedValue(declared);
    const { saved } = await setupEdit();
    await waitFor(() => {
        expect(field("Alias").readOnly).toBe(false);
    });
    // Catalog support shows as checked; nothing is user-declared yet.
    expect(checked("Reasoning")).toBe(true);
    expect(
        screen.getByRole<HTMLButtonElement>("button", {
            name: "Restore default",
        }).disabled,
    ).toBe(true);
    // Checking and clearing are both declarations, in both directions.
    fireEvent.click(checkbox("Image input"));
    fireEvent.click(checkbox("Image input"));
    fireEvent.click(checkbox("Tool calls"));
    expect(checked("Image input")).toBe(false);
    expect(checked("Tool calls")).toBe(true);
    update.mockResolvedValue({
        ...declared,
        contentDigest: "b".repeat(64),
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => {
        expect(update).toHaveBeenCalledWith(
            expect.objectContaining({
                capabilityOverrides: {
                    thinking: null,
                    imageInput: "unsupported",
                    toolCalls: "supported",
                },
            }),
        );
    });
    // Restore default removes the declarations again: the catalog state comes
    // back and the action disables itself until something is declared.
    fireEvent.click(checkbox("Reasoning"));
    fireEvent.click(screen.getByRole("button", { name: "Restore default" }));
    expect(checked("Reasoning")).toBe(true);
    expect(
        screen.getByRole<HTMLButtonElement>("button", {
            name: "Restore default",
        }).disabled,
    ).toBe(true);
    expect(saved).toHaveBeenCalled();
});

test("a stale snapshot requires a reload and the reloaded draft saves", async () => {
    const { saved } = await setupEdit();
    await waitFor(() => {
        expect(field("Alias").readOnly).toBe(false);
    });
    fireEvent.change(field("Alias"), {
        target: { value: "My draft" },
    });
    update.mockRejectedValueOnce(new ModelRequestError("model_changed"));
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect((await screen.findByRole("alert")).textContent).toContain("changed");
    expect(
        screen
            .getByRole<HTMLButtonElement>("button", {
                name: "Save",
            })
            .getAttribute("aria-disabled"),
    ).toBe("true");
    const next = {
        contentDigest: "b".repeat(64),
        model: { ...row, alias: "Other window" },
    };
    get.mockResolvedValueOnce(next);
    fireEvent.click(
        screen.getByRole("button", {
            name: "Reload saved model and discard draft",
        }),
    );
    await waitFor(() => {
        expect(field("Alias").readOnly).toBe(false);
    });
    expect(field("Alias").value).toBe("Other window");
    fireEvent.change(field("Alias"), {
        target: { value: "Confirmed" },
    });
    update.mockResolvedValue({
        ...next,
        model: { ...next.model, alias: "Confirmed" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => {
        expect(update).toHaveBeenLastCalledWith(
            expect.objectContaining({
                expectedDigest: next.contentDigest,
                alias: "Confirmed",
            }),
        );
    });
    expect(saved).toHaveBeenCalled();
});

test("unknown writes stay blocked through failed recovery reads", async () => {
    await setupEdit();
    await waitFor(() => {
        expect(field("Alias").readOnly).toBe(false);
    });
    update.mockRejectedValueOnce(new ModelRequestError("invalid_response"));
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect((await screen.findByRole("alert")).textContent).toContain(
        "save result is unknown",
    );
    get.mockRejectedValueOnce(new ModelRequestError("read_failed"));
    fireEvent.click(
        screen.getByRole("button", {
            name: "Reload saved model and discard draft",
        }),
    );
    await waitFor(() => {
        expect(screen.getByRole("alert").textContent).toContain(
            "could not be read",
        );
    });
    // The failed recovery read freezes the form: saving is offered but rejected,
    // and only another reload can revive it.
    expect(
        screen
            .getByRole("button", { name: "Save" })
            .getAttribute("aria-disabled"),
    ).toBe("true");
    expect(
        screen.getByRole("button", {
            name: "Reload saved model and discard draft",
        }),
    ).toBeTruthy();
    expect(update).toHaveBeenCalledTimes(1);
});

test("invalid aliases preserve the draft and language changes preserve input", async () => {
    const { i18n } = await setupEdit();
    await waitFor(() => {
        expect(field("Alias").readOnly).toBe(false);
    });
    fireEvent.change(field("Alias"), {
        target: { value: "Invalid draft" },
    });
    update.mockRejectedValueOnce(new ModelRequestError("model_alias_invalid"));
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect((await screen.findByRole("alert")).textContent).toContain(
        "display name is not valid",
    );
    expect(field("Alias").value).toBe("Invalid draft");
    await act(async () => {
        await i18n.changeLanguage("zh-CN");
    });
    const zhAlias = screen.getByLabelText<HTMLInputElement>("别名");
    expect(zhAlias.placeholder).toBe("可选");
    expect(zhAlias.value).toBe("Invalid draft");
    expect(
        screen
            .getByRole<HTMLButtonElement>("button", { name: "保存" })
            .getAttribute("aria-disabled"),
    ).toBe("false");
    expect(screen.getAllByText("未设置").length).toBeGreaterThan(0);
});

test("Escape cancels an edit but never while a save is in flight", async () => {
    const { cancel } = await setupEdit();
    await waitFor(() => {
        expect(field("Alias").readOnly).toBe(false);
    });
    let finish: ((value: ModelSnapshot) => void) | undefined;
    update.mockImplementation(
        () =>
            new Promise((resolve) => {
                finish = resolve;
            }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    fireEvent.keyDown(field("Alias"), { key: "Escape" });
    expect(cancel).not.toHaveBeenCalled();
    await act(async () => {
        finish?.({ model: row, contentDigest: "b".repeat(64) });
        await Promise.resolve();
    });
    fireEvent.keyDown(field("Alias"), { key: "Escape" });
    expect(cancel).toHaveBeenCalledTimes(1);
});

test("ignores a save reply after the form unmounts", async () => {
    const { saved, unmount } = await setupEdit();
    await waitFor(() => {
        expect(field("Alias").readOnly).toBe(false);
    });
    let finish: ((value: ModelSnapshot) => void) | undefined;
    update.mockImplementation(
        () =>
            new Promise((resolve) => {
                finish = resolve;
            }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    unmount();
    await act(async () => {
        finish?.({ model: row, contentDigest: "b".repeat(64) });
        await Promise.resolve();
    });
    expect(saved).not.toHaveBeenCalled();
});
