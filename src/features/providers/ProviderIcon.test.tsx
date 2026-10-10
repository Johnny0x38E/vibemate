import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { ProviderIcon } from "./ProviderIcon";

afterEach(cleanup);

function images(container: HTMLElement): HTMLImageElement[] {
    return Array.from(container.querySelectorAll("img"));
}

test.each([
    ["command-code", "command-code.svg"],
    ["deepseek", "deepseek.svg"],
] as const)(
    "%s shows its single official icon, hidden from assistive technology",
    (kind, file) => {
        const { container } = render(<ProviderIcon kind={kind} />);
        const [icon, ...rest] = images(container);
        expect(rest).toHaveLength(0);
        expect(icon?.getAttribute("src")).toMatch(new RegExp(`/${file}$`));
        // Decorative: the name next to it identifies the provider.
        expect(icon?.getAttribute("alt")).toBe("");
        expect(screen.queryAllByRole("img")).toHaveLength(0);
    },
);

test("OpenRouter ships the grape file for light and the volt file for dark", () => {
    const { container } = render(<ProviderIcon kind="openrouter" />);
    const source = (scheme: string): string | null | undefined =>
        container
            .querySelector(`img[data-scheme="${scheme}"]`)
            ?.getAttribute("src");
    expect(images(container)).toHaveLength(2);
    expect(source("light")).toMatch(/\/openrouter\.svg$/);
    expect(source("dark")).toMatch(/\/openrouter-volt\.svg$/);
    expect(images(container).every((image) => image.alt === "")).toBe(true);
    expect(screen.queryAllByRole("img")).toHaveLength(0);
});

test("an unknown kind renders no icon and does not throw", () => {
    const { container } = render(<ProviderIcon kind="future-provider" />);
    expect(container.innerHTML).toBe("");
    const proto = render(<ProviderIcon kind="toString" />);
    expect(proto.container.innerHTML).toBe("");
});
