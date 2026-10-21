import { cleanup, render, screen } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { afterEach, expect, test } from "vitest";
import { createAppI18n } from "../../i18n";
import type { PiModelFact } from "../../lib/desktop/pi";
import { piModelFactsFixture } from "../../test/piFacts";
import { PiModelFacts } from "./PiModelFacts";

afterEach(cleanup);

/** Render the facts section with the English resources the assertions read. */
async function renderFacts(
    facts: PiModelFact[] = piModelFactsFixture,
): Promise<void> {
    const i18n = await createAppI18n("en");
    render(
        <I18nextProvider i18n={i18n}>
            <PiModelFacts facts={facts} />
        </I18nextProvider>,
    );
}

test("shows each setting's value, where it came from and what this plan does with it", async () => {
    await renderFacts();
    // Token counts are formatted for the active locale.
    expect(screen.getByText("128,000 tokens")).toBeTruthy();
    expect(screen.getByText("64,000 tokens")).toBeTruthy();
    expect(screen.getByText("Written to contextWindow.")).toBeTruthy();
    expect(screen.getByText("Stated by the provider catalog.")).toBeTruthy();
    expect(screen.getByText("Supported")).toBeTruthy();
    expect(
        screen.getAllByText(
            "Saved in the model library; the origin is not recorded.",
        ),
    ).toHaveLength(2);
    // An unstated item is said so, and nothing is written for it.
    expect(screen.getByText("Not stated")).toBeTruthy();
    expect(
        screen.getByText("Not written: Pi's own default applies."),
    ).toBeTruthy();
    // A kept thinking level shows the value already in the file.
    expect(screen.getByText("low")).toBeTruthy();
    expect(
        screen.getByText("Not written: the value already in the file stays."),
    ).toBeTruthy();
});

test("translates the image-input tokens it knows and shows any other value as received", async () => {
    const facts = piModelFactsFixture.map((fact): PiModelFact => {
        if (fact.item === "imageInput")
            return {
                ...fact,
                source: "catalog",
                state: "written",
                value: "text",
            };
        if (fact.item === "thinkingLevel")
            return { ...fact, value: "custom-level" };
        return fact;
    });
    await renderFacts(facts);
    expect(screen.getByText("Text only")).toBeTruthy();
    expect(screen.getByText("custom-level")).toBeTruthy();
});

test("says which settings vibemate leaves alone and where overrides come from", async () => {
    await renderFacts();
    expect(
        screen.getByText(/Existing request parameters stay unchanged/u),
    ).toBeTruthy();
    expect(
        screen.getByText(/Pi reads the maximum output as the model's limit/u),
    ).toBeTruthy();
    expect(
        screen.getByText(
            /Project settings, command-line options and resumed sessions/u,
        ),
    ).toBeTruthy();
});
