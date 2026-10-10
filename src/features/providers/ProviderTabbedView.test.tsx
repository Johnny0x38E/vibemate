import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import type { ProviderRecord } from "../../lib/desktop/providers";
import { ProviderTabbedView } from "./ProviderTabbedView";

vi.mock(import("./ProviderForm"), () => ({
    ProviderForm: () => <div data-testid="provider-form">form</div>,
}));
vi.mock(import("./ProviderModels"), () => ({
    ProviderModels: () => <div data-testid="provider-models">models</div>,
}));
vi.mock(import("./ProviderModelsCreateGate"), () => ({
    ProviderModelsCreateGate: () => (
        <div data-testid="provider-models-gate">gate</div>
    ),
}));

const record: ProviderRecord = {
    id: "0123456789abcdef0123456789abcdef",
    kind: "deepseek",
    displayName: "Work",
    baseUrl: "https://api.deepseek.com",
    protocol: "chat_completions",
    extensions: {},
    revision: 1,
    createdAtMs: 1000,
    updatedAtMs: 1000,
    selectedModelCount: 0,
};

afterEach(cleanup);

test("edit: switches between API and Models tabs with keyboard and keeps both panels mounted", async () => {
    const i18n = await createAppI18n("en");
    render(
        <StrictMode>
            <I18nextProvider i18n={i18n}>
                <ProviderTabbedView
                    mode={{ kind: "edit", record }}
                    templates={[]}
                    onSaved={vi.fn()}
                    onRecordLoaded={vi.fn()}
                    onRefresh={() => Promise.resolve(true)}
                    onFormBusyChange={vi.fn()}
                    onModelsBusyChange={vi.fn()}
                />
            </I18nextProvider>
        </StrictMode>,
    );

    const api = screen.getByRole("tab", { name: "API" });
    const models = screen.getByRole("tab", { name: "Models" });
    expect(api.getAttribute("aria-selected")).toBe("true");
    expect(screen.getByTestId("provider-form")).toBeDefined();
    expect(screen.getByTestId("provider-models")).toBeDefined();
    const modelsPanel = document.getElementById(
        `provider-${record.id}-panel-models`,
    );
    expect(modelsPanel?.hasAttribute("hidden")).toBe(true);
    expect(modelsPanel?.hasAttribute("inert")).toBe(true);

    fireEvent.click(models);
    expect(models.getAttribute("aria-selected")).toBe("true");
    expect(api.getAttribute("tabIndex")).toBe("-1");

    fireEvent.keyDown(models, { key: "ArrowLeft" });
    expect(api.getAttribute("aria-selected")).toBe("true");
    expect(document.activeElement).toBe(api);
});

test("create: shows Models gate instead of the models list", async () => {
    const i18n = await createAppI18n("en");
    render(
        <StrictMode>
            <I18nextProvider i18n={i18n}>
                <ProviderTabbedView
                    mode={{ kind: "create" }}
                    templates={[]}
                    onSaved={vi.fn()}
                    onRecordLoaded={vi.fn()}
                    onRefresh={() => Promise.resolve(true)}
                    onFormBusyChange={vi.fn()}
                    onModelsBusyChange={vi.fn()}
                />
            </I18nextProvider>
        </StrictMode>,
    );

    fireEvent.click(screen.getByRole("tab", { name: "Models" }));
    expect(screen.getByTestId("provider-models-gate")).toBeDefined();
    expect(screen.queryByTestId("provider-models")).toBeNull();
});
