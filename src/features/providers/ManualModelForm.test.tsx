import {
    act,
    cleanup,
    fireEvent,
    render,
    screen,
    waitFor,
} from "@testing-library/react";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import {
    addManualProviderModel,
    listProviderModels,
    ModelRequestError,
    type ProviderModel,
} from "../../lib/desktop/models";
import { ManualModelForm } from "./ManualModelForm";

vi.mock(import("../../lib/desktop/models"), async (importOriginal) => ({
    ...(await importOriginal()),
    addManualProviderModel: vi.fn(),
    listProviderModels: vi.fn(),
}));
const add = vi.mocked(addManualProviderModel);
const list = vi.mocked(listProviderModels);
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
    routeSupport: "unknown",
    upstreamState: "never_listed",
    lastSeenAtMs: null,
    missingSinceMs: null,
    createdAtMs: 1000,
    updatedAtMs: 1000,
};
async function setup(locale: "en" | "zh-CN" = "en", blocked = false) {
    const i18n = await createAppI18n(locale);
    const saved = vi.fn();
    const busy = vi.fn();
    const view = render(
        <StrictMode>
            <I18nextProvider i18n={i18n}>
                <ManualModelForm
                    providerId={ID}
                    blocked={blocked}
                    onSaved={saved}
                    onBusyChange={busy}
                />
            </I18nextProvider>
        </StrictMode>,
    );
    return { ...view, saved, busy };
}
function enter(id = " vendor/model ", alias = "") {
    fireEvent.change(screen.getByLabelText("Model ID"), {
        target: { value: id },
    });
    fireEvent.change(screen.getByLabelText("Alias (optional)"), {
        target: { value: alias },
    });
}
function submit() {
    fireEvent.click(screen.getByRole("button", { name: "Add and save" }));
}
function page(items: ProviderModel[]) {
    return {
        kind: "desktop" as const,
        page: { items, nextCursor: null, totalMatches: items.length },
    };
}
beforeEach(() => {
    add.mockReset().mockResolvedValue(row);
    list.mockReset();
});
afterEach(() => {
    cleanup();
});

test("submits through Rust, clears fields and reports the saved manual row", async () => {
    const { saved } = await setup();
    enter(" vendor/model ", "Work");
    submit();
    await waitFor(() => {
        expect(saved).toHaveBeenCalledWith(row);
    });
    expect(add).toHaveBeenCalledWith({
        providerId: ID,
        modelId: " vendor/model ",
        alias: "Work",
    });
    expect(screen.getByLabelText<HTMLInputElement>("Model ID").value).toBe("");
    expect(
        screen.getByLabelText<HTMLInputElement>("Alias (optional)").value,
    ).toBe("");
});

test("normalizes an empty optional alias without guessing model validation", async () => {
    await setup();
    enter("vendor/model", "  ");
    submit();
    await waitFor(() => {
        expect(add).toHaveBeenCalledWith({
            providerId: ID,
            modelId: "vendor/model",
            alias: null,
        });
    });
});

test.each([
    "model_id_invalid",
    "model_alias_invalid",
    "model_already_exists",
] as const)(
    "marks the correct field for %s and preserves input for retry",
    async (code) => {
        add.mockRejectedValueOnce(new ModelRequestError(code));
        const { saved } = await setup();
        enter();
        submit();
        await screen.findByRole("alert");
        const field = screen.getByLabelText(
            code === "model_alias_invalid" ? "Alias (optional)" : "Model ID",
        );
        expect(field.getAttribute("aria-invalid")).toBe("true");
        expect(saved).not.toHaveBeenCalled();
        submit();
        await waitFor(() => {
            expect(saved).toHaveBeenCalledOnce();
        });
    },
);

test("blocks duplicate submits and ignores a write reply after unmount", async () => {
    let resolve: (model: ProviderModel) => void = () => {};
    add.mockImplementation(
        () =>
            new Promise((done) => {
                resolve = done;
            }),
    );
    const { saved, busy, unmount } = await setup();
    enter();
    submit();
    submit();
    expect(add).toHaveBeenCalledOnce();
    expect(busy).toHaveBeenCalledWith(true);
    unmount();
    await act(async () => {
        resolve(row);
        await Promise.resolve();
    });
    expect(saved).not.toHaveBeenCalled();
    expect(busy).toHaveBeenLastCalledWith(false);
});

test("unknown write results require a successful exact-ID check before another creation", async () => {
    add.mockRejectedValueOnce(new ModelRequestError("invalid_response"));
    list.mockRejectedValueOnce(new Error("raw internal diagnostic"));
    const { saved } = await setup();
    enter();
    submit();
    await screen.findByText(/save result is unknown/i);
    submit();
    expect(add).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Check saved models" }));
    await waitFor(() => {
        expect(
            screen
                .getByRole("button", { name: "Check saved models" })
                .getAttribute("aria-disabled"),
        ).toBe("false");
    });
    expect(screen.queryByText("raw internal diagnostic")).toBeNull();
    list.mockResolvedValue(page([row]));
    fireEvent.click(screen.getByRole("button", { name: "Check saved models" }));
    await waitFor(() => {
        expect(saved).toHaveBeenCalledWith(row);
    });
    expect(list).toHaveBeenLastCalledWith({
        providerId: ID,
        after: null,
        limit: 200,
        filter: "all",
    });
    expect(add).toHaveBeenCalledOnce();
});

test("a fuzzy match does not confirm the write and a completed read permits retry", async () => {
    add.mockRejectedValueOnce(new ModelRequestError("operation_failed"));
    list.mockResolvedValue(page([{ ...row, modelId: "vendor/model-other" }]));
    const { saved } = await setup();
    enter();
    submit();
    await screen.findByText(/save result is unknown/i);
    fireEvent.click(screen.getByRole("button", { name: "Check saved models" }));
    await waitFor(() => {
        expect(screen.queryByText(/save result is unknown/i)).toBeNull();
    });
    expect(saved).not.toHaveBeenCalled();
    submit();
    await waitFor(() => {
        expect(saved).toHaveBeenCalledOnce();
    });
});

test("blocked state makes no write even on form submission", async () => {
    await setup("en", true);
    fireEvent.submit(
        screen.getByRole("form", { name: "Add a model manually" }),
    );
    expect(add).not.toHaveBeenCalled();
});

test("Chinese labels and field errors remain translated", async () => {
    add.mockRejectedValue(new ModelRequestError("model_id_invalid"));
    await setup("zh-CN");
    fireEvent.change(screen.getByLabelText("模型 ID"), {
        target: { value: "bad id" },
    });
    fireEvent.click(screen.getByRole("button", { name: "添加并保存" }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).not.toContain("model_id_invalid");
    expect(screen.getByLabelText("模型 ID").getAttribute("aria-invalid")).toBe(
        "true",
    );
});

test("unknown results check every cursor page, including long model IDs", async () => {
    const modelId = "m".repeat(256);
    const longRow = { ...row, modelId };
    add.mockRejectedValueOnce(new ModelRequestError("operation_failed"));
    list.mockResolvedValueOnce({
        kind: "desktop",
        page: { items: [], nextCursor: "next", totalMatches: null },
    }).mockResolvedValueOnce(page([longRow]));
    const { saved } = await setup();
    enter(modelId);
    submit();
    await screen.findByText(/save result is unknown/i);
    fireEvent.click(screen.getByRole("button", { name: "Check saved models" }));
    await waitFor(() => {
        expect(saved).toHaveBeenCalledWith(longRow);
    });
    expect(list).toHaveBeenLastCalledWith({
        providerId: ID,
        after: "next",
        limit: 200,
        filter: "all",
    });
});
