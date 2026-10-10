import {
    cleanup,
    fireEvent,
    render,
    screen,
    waitFor,
    within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode, useState } from "react";
import { afterEach, expect, test, vi } from "vitest";
import {
    fieldSelectDisabled,
    fieldSelectValue,
    setFieldSelectValue,
} from "../test/fieldSelect";
import { FieldSelect } from "./FieldSelect";

const OPTIONS = [
    { value: "a", label: "Alpha" },
    { value: "b", label: "Beta" },
    { value: "g", label: "Gamma" },
];

afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
});

function renderSelect(
    props: Partial<React.ComponentProps<typeof FieldSelect>> = {},
): void {
    render(
        <>
            <label htmlFor="demo">Choice</label>
            <FieldSelect
                id="demo"
                value="a"
                options={OPTIONS}
                onChange={() => {}}
                {...props}
            />
            <button type="button">Next action</button>
        </>,
    );
}

test("opens the list and reports the chosen value", async () => {
    const onChange = vi.fn();
    renderSelect({ onChange });
    const combobox = screen.getByRole("combobox", { name: "Choice" });
    expect(fieldSelectValue(combobox)).toBe("a");
    await setFieldSelectValue(combobox, "b");
    expect(onChange).toHaveBeenCalledWith("b");
    expect(fieldSelectValue(combobox)).toBe("a");
});

test("does not open when disabled", () => {
    renderSelect({ disabled: true });
    const combobox = screen.getByRole("combobox", { name: "Choice" });
    expect(fieldSelectDisabled(combobox)).toBe(true);
    fireEvent.click(combobox);
    expect(screen.queryByRole("option")).toBeNull();
});

test("does not open when blocked with aria-disabled", () => {
    renderSelect({ blocked: true });
    const combobox = screen.getByRole("combobox", { name: "Choice" });
    expect(fieldSelectDisabled(combobox)).toBe(true);
    fireEvent.click(combobox);
    expect(screen.queryByRole("option")).toBeNull();
});

test("keyboard navigation moves real focus and Enter reports the highlighted value", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    renderSelect({ onChange });
    await user.tab();
    const trigger = screen.getByRole("combobox", { name: "Choice" });
    expect(document.activeElement).toBe(trigger);
    await user.keyboard("{ArrowDown}");
    const alpha = await screen.findByRole("option", { name: "Alpha" });
    await waitFor(() => {
        expect(document.activeElement).toBe(alpha);
    });
    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(
        screen.getByRole("option", { name: "Beta" }),
    );
    await user.keyboard("{Enter}");
    expect(onChange).toHaveBeenCalledExactlyOnceWith("b");
    await waitFor(() => {
        expect(document.activeElement).toBe(trigger);
    });
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    // The parent has not acknowledged a saved change, so display stays controlled.
    expect(fieldSelectValue(trigger)).toBe("a");
});

test("Home, End and ArrowUp navigate without committing until Space", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    renderSelect({ onChange });
    await user.tab();
    await user.keyboard("{Enter}");
    await screen.findByRole("option", { name: "Alpha" });
    await user.keyboard("{End}");
    expect(document.activeElement).toBe(
        screen.getByRole("option", { name: "Gamma" }),
    );
    await user.keyboard("{Home}");
    expect(document.activeElement).toBe(
        screen.getByRole("option", { name: "Alpha" }),
    );
    await user.keyboard("{End}{ArrowUp}");
    expect(document.activeElement).toBe(
        screen.getByRole("option", { name: "Beta" }),
    );
    expect(onChange).not.toHaveBeenCalled();
    await user.keyboard(" ");
    expect(onChange).toHaveBeenCalledExactlyOnceWith("b");
});

test("typeahead highlights the matching text instead of the raw value", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    renderSelect({ onChange });
    await user.tab();
    await user.keyboard("{ArrowDown}");
    await screen.findByRole("option", { name: "Alpha" });
    await user.keyboard("gam");
    expect(document.activeElement).toBe(
        screen.getByRole("option", { name: "Gamma" }),
    );
    await user.keyboard("{Enter}");
    expect(onChange).toHaveBeenCalledExactlyOnceWith("g");
});

test("Escape dismisses without saving and returns focus to the trigger", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    renderSelect({ onChange });
    const trigger = screen.getByRole("combobox", { name: "Choice" });
    await user.click(trigger);
    await screen.findByRole("option", { name: "Alpha" });
    await user.keyboard("{ArrowDown}{Escape}");
    await waitFor(() => {
        expect(document.activeElement).toBe(trigger);
    });
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    expect(onChange).not.toHaveBeenCalled();
});

test("Tab dismisses the popup and advances to the next action", async () => {
    const user = userEvent.setup();
    renderSelect();
    const trigger = screen.getByRole("combobox", { name: "Choice" });
    const next = screen.getByRole("button", { name: "Next action" });
    // jsdom has no layout. Base UI checks real client rectangles to find the
    // next tabbable control; supply geometry only for these two visible buttons.
    const rectangles = Object.assign([new DOMRect(0, 0, 176, 36)], {
        item: (index: number) =>
            index === 0 ? new DOMRect(0, 0, 176, 36) : null,
    });
    vi.spyOn(trigger, "getClientRects").mockReturnValue(rectangles);
    vi.spyOn(next, "getClientRects").mockReturnValue(rectangles);
    await user.click(trigger);
    const alpha = await screen.findByRole("option", { name: "Alpha" });
    await waitFor(() => {
        expect(document.activeElement).toBe(alpha);
    });
    await user.tab();
    await waitFor(() => {
        expect(trigger.getAttribute("aria-expanded")).toBe("false");
    });
    expect(document.activeElement).toBe(
        screen.getByRole("button", { name: "Next action" }),
    );
});

test("outside click dismisses without stealing the destination's focus", async () => {
    const user = userEvent.setup();
    renderSelect();
    const trigger = screen.getByRole("combobox", { name: "Choice" });
    await user.click(trigger);
    await screen.findByRole("option", { name: "Alpha" });
    const next = screen.getByRole("button", { name: "Next action" });
    await user.click(next);
    await waitFor(() => {
        expect(trigger.getAttribute("aria-expanded")).toBe("false");
    });
    expect(document.activeElement).toBe(next);
});

for (const lock of ["disabled", "blocked"] as const) {
    test(`${lock} closes an open menu and does not reopen it when unlocked`, async () => {
        const user = userEvent.setup();
        const onChange = vi.fn();
        function Fixture({ locked }: { locked: boolean }) {
            return (
                <>
                    <label htmlFor="locked">Choice</label>
                    <FieldSelect
                        id="locked"
                        value="a"
                        options={OPTIONS}
                        onChange={onChange}
                        disabled={lock === "disabled" && locked}
                        blocked={lock === "blocked" && locked}
                    />
                </>
            );
        }
        const view = render(<Fixture locked={false} />);
        const trigger = screen.getByRole("combobox", { name: "Choice" });
        await user.click(trigger);
        await screen.findByRole("option", { name: "Alpha" });
        view.rerender(<Fixture locked />);
        expect(trigger.getAttribute("aria-expanded")).toBe("false");
        expect(trigger.hasAttribute("disabled")).toBe(lock === "disabled");
        if (lock === "blocked") {
            trigger.focus();
            expect(document.activeElement).toBe(trigger);
            await user.keyboard("b{ArrowDown}{Enter}");
        }
        expect(onChange).not.toHaveBeenCalled();
        view.rerender(<Fixture locked={false} />);
        expect(trigger.getAttribute("aria-expanded")).toBe("false");
        await user.click(trigger);
        await screen.findByRole("option", { name: "Beta" });
        await user.click(screen.getByRole("option", { name: "Beta" }));
        expect(onChange).toHaveBeenCalledExactlyOnceWith("b");
    });
}

test("portaled options retain labels, icons, language and validation associations", async () => {
    const user = userEvent.setup();
    renderSelect({
        "aria-describedby": "choice-help",
        "aria-invalid": true,
        options: [
            {
                value: "a",
                label: "Alpha",
                leading: <img src="/synthetic-icon.svg" alt="" />,
                optionLang: "en",
            },
        ],
    });
    const trigger = screen.getByRole("combobox", { name: "Choice" });
    expect(trigger.getAttribute("aria-describedby")).toBe("choice-help");
    expect(trigger.getAttribute("aria-invalid")).toBe("true");
    expect(trigger.querySelector("img")).not.toBeNull();
    await user.click(trigger);
    const option = await screen.findByRole("option", { name: "Alpha" });
    expect(trigger.closest("[data-field-select]")?.contains(option)).toBe(
        false,
    );
    expect(option.getAttribute("lang")).toBe("en");
    expect(option.querySelector("img")).not.toBeNull();
    const listId = trigger.getAttribute("aria-controls");
    expect(document.getElementById(listId ?? "")?.contains(option)).toBe(true);
});

test("multiple controls keep their values and popup relationships separate", async () => {
    const first = vi.fn();
    const second = vi.fn();
    render(
        <>
            <label htmlFor="first">First</label>
            <FieldSelect
                id="first"
                value="a"
                options={OPTIONS}
                onChange={first}
            />
            <label htmlFor="second">Second</label>
            <FieldSelect
                id="second"
                value="g"
                options={OPTIONS}
                onChange={second}
            />
        </>,
    );
    await setFieldSelectValue(
        screen.getByRole("combobox", { name: "Second" }),
        "b",
    );
    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledExactlyOnceWith("b");
    expect(
        fieldSelectValue(screen.getByRole("combobox", { name: "First" })),
    ).toBe("a");
});

test("StrictMode preserves controlled updates and unmount removes the portaled menu", async () => {
    const user = userEvent.setup();
    function Fixture() {
        const [value, setValue] = useState("a");
        return (
            <>
                <label htmlFor="strict">Choice</label>
                <FieldSelect
                    id="strict"
                    value={value}
                    options={OPTIONS}
                    onChange={setValue}
                />
            </>
        );
    }
    const view = render(
        <StrictMode>
            <Fixture />
        </StrictMode>,
    );
    const trigger = screen.getByRole("combobox", { name: "Choice" });
    await setFieldSelectValue(trigger, "g");
    expect(fieldSelectValue(trigger)).toBe("g");
    expect(within(trigger).getByText("Gamma")).toBeDefined();
    await user.click(trigger);
    await screen.findByRole("option", { name: "Gamma" });
    view.unmount();
    expect(screen.queryByRole("listbox")).toBeNull();
});
