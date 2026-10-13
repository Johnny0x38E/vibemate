import {
    act,
    cleanup,
    fireEvent,
    render,
    screen,
    waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import {
    getProviderModelSnapshot,
    updateProviderModelAlias,
    ModelRequestError,
    type ModelSnapshot,
} from "../../lib/desktop/models";
import { ModelAliasEditor } from "./ModelAliasEditor";

vi.mock(import("../../lib/desktop/models"), async (original) => ({
    ...(await original()),
    getProviderModelSnapshot: vi.fn(),
    updateProviderModelAlias: vi.fn(),
}));
const get = vi.mocked(getProviderModelSnapshot);
const update = vi.mocked(updateProviderModelAlias);
const snapshot: ModelSnapshot = {
    contentDigest: "a".repeat(64),
    model: {
        providerId: "0123456789abcdef0123456789abcdef",
        modelId: "vendor/model",
        source: "fetched",
        selected: true,
        alias: "Old alias",
        upstreamName: "Upstream",
        contextWindow: 1000,
        maxOutputTokens: null,
        inputModalities: ["text"],
        outputModalities: null,
        supportedEndpoints: null,
        capabilities: {
            thinking: { state: "unknown", source: null },
            imageInput: { state: "unknown", source: null },
            toolCalls: { state: "unknown", source: null },
        },
        routeSupport: "not_applicable",
        resolvedProtocol: "chat_completions",
        upstreamState: "listed",
        lastSeenAtMs: 100,
        missingSinceMs: null,
        createdAtMs: 100,
        updatedAtMs: 100,
    },
};

async function setup() {
    const i18n = await createAppI18n("en");
    const saved = vi.fn();
    const close = vi.fn();
    const view = render(
        <StrictMode>
            <I18nextProvider i18n={i18n}>
                <ModelAliasEditor
                    providerId={snapshot.model.providerId}
                    modelId={snapshot.model.modelId}
                    onSaved={saved}
                    onClose={close}
                    onBusyChange={vi.fn()}
                    onSavingChange={vi.fn()}
                />
            </I18nextProvider>
        </StrictMode>,
    );
    return { ...view, saved, close, i18n };
}

async function input() {
    const field = screen.getByRole<HTMLInputElement>("textbox", {
        name: "Alias (optional)",
    });
    await waitFor(() => {
        expect(field.readOnly).toBe(false);
    });
    return field;
}

beforeEach(() => {
    get.mockReset().mockResolvedValue(snapshot);
    update.mockReset();
});
afterEach(cleanup);

test("loads the saved alias, focuses it and saves exactly that model and snapshot", async () => {
    const { saved } = await setup();
    const field = await input();
    // Focus follows the ready phase through an effect, so allow the commit to flush.
    await waitFor(() => {
        expect(document.activeElement).toBe(field);
    });
    expect(field.value).toBe("Old alias");
    fireEvent.change(field, { target: { value: "My model" } });
    const next = {
        ...snapshot,
        contentDigest: "b".repeat(64),
        model: { ...snapshot.model, alias: "My model" },
    };
    update.mockResolvedValue(next);
    await userEvent.setup().keyboard("{Enter}");
    await waitFor(() => {
        expect(saved).toHaveBeenCalledWith(next.model);
    });
    expect(update).toHaveBeenCalledWith({
        providerId: snapshot.model.providerId,
        modelId: "vendor/model",
        expectedDigest: snapshot.contentDigest,
        alias: "My model",
    });
});

test("clears an alias and prevents duplicate saves or cancellation while saving", async () => {
    const { saved, close } = await setup();
    fireEvent.change(await input(), { target: { value: "  " } });
    let finish: ((value: ModelSnapshot) => void) | undefined;
    update.mockImplementation(
        () =>
            new Promise((resolve) => {
                finish = resolve;
            }),
    );
    const form = screen.getByRole("form");
    fireEvent.submit(form);
    fireEvent.submit(form);
    expect(update).toHaveBeenCalledTimes(1);
    expect(update).toHaveBeenCalledWith(
        expect.objectContaining({ alias: null }),
    );
    expect(
        screen.getByRole<HTMLButtonElement>("button", { name: "Cancel" })
            .disabled,
    ).toBe(true);
    fireEvent.keyDown(form, { key: "Escape" });
    expect(close).not.toHaveBeenCalled();
    const next = { ...snapshot, model: { ...snapshot.model, alias: null } };
    await act(async () => {
        finish?.(next);
        await Promise.resolve();
    });
    expect(saved).toHaveBeenCalledWith(next.model);
});

test("requires explicit reload after a conflict and uses the new digest on retry", async () => {
    await setup();
    fireEvent.change(await input(), { target: { value: "My draft" } });
    update.mockRejectedValueOnce(new ModelRequestError("model_changed"));
    fireEvent.click(screen.getByRole("button", { name: "Save alias" }));
    expect((await screen.findByRole("alert")).textContent).toContain("changed");
    expect(
        screen.getByRole<HTMLButtonElement>("button", { name: "Save alias" })
            .disabled,
    ).toBe(true);
    const next = {
        ...snapshot,
        contentDigest: "b".repeat(64),
        model: { ...snapshot.model, alias: "Other window" },
    };
    get.mockResolvedValueOnce(next);
    fireEvent.click(
        screen.getByRole("button", {
            name: "Reload saved alias and discard draft",
        }),
    );
    expect((await input()).value).toBe("Other window");
    fireEvent.change(await input(), { target: { value: "Confirmed" } });
    update.mockResolvedValue({
        ...next,
        model: { ...next.model, alias: "Confirmed" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save alias" }));
    await waitFor(() => {
        expect(update).toHaveBeenLastCalledWith(
            expect.objectContaining({
                expectedDigest: next.contentDigest,
                alias: "Confirmed",
            }),
        );
    });
});

test("unknown writes stay blocked through failed recovery reads", async () => {
    await setup();
    await input();
    update.mockRejectedValueOnce(new ModelRequestError("invalid_response"));
    fireEvent.click(screen.getByRole("button", { name: "Save alias" }));
    expect((await screen.findByRole("alert")).textContent).toContain(
        "save result is unknown",
    );
    get.mockRejectedValueOnce(new ModelRequestError("read_failed"));
    fireEvent.click(
        screen.getByRole("button", {
            name: "Reload saved alias and discard draft",
        }),
    );
    await waitFor(() => {
        expect(screen.getByRole("alert").textContent).toContain(
            "could not be read",
        );
    });
    expect(
        screen.getByRole<HTMLButtonElement>("button", { name: "Save alias" })
            .disabled,
    ).toBe(true);
    get.mockResolvedValueOnce(snapshot);
    fireEvent.click(
        screen.getByRole("button", {
            name: "Reload saved alias and discard draft",
        }),
    );
    await input();
    expect(
        screen.getByRole<HTMLButtonElement>("button", { name: "Save alias" })
            .disabled,
    ).toBe(false);
    expect(update).toHaveBeenCalledTimes(1);
});

test("invalid aliases preserve the draft and language changes preserve input", async () => {
    const { i18n } = await setup();
    fireEvent.change(await input(), { target: { value: "Invalid draft" } });
    update.mockRejectedValueOnce(new ModelRequestError("model_alias_invalid"));
    fireEvent.click(screen.getByRole("button", { name: "Save alias" }));
    expect((await screen.findByRole("alert")).textContent).toContain(
        "display name is not valid",
    );
    expect((await input()).value).toBe("Invalid draft");
    await act(async () => {
        await i18n.changeLanguage("zh-CN");
    });
    const field = screen.getByRole<HTMLInputElement>("textbox", {
        name: "别名（可选）",
    });
    expect(field.value).toBe("Invalid draft");
    expect(
        screen.getByRole<HTMLButtonElement>("button", { name: "保存别名" })
            .disabled,
    ).toBe(false);
});

test("ignores a save reply after the editor unmounts", async () => {
    const { saved, unmount } = await setup();
    await input();
    let finish: ((value: ModelSnapshot) => void) | undefined;
    update.mockImplementation(
        () =>
            new Promise((resolve) => {
                finish = resolve;
            }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Save alias" }));
    unmount();
    await act(async () => {
        finish?.(snapshot);
        await Promise.resolve();
    });
    expect(saved).not.toHaveBeenCalled();
});
