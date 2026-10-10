import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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
];

afterEach(() => {
    cleanup();
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
        </>,
    );
}

test("opens the list and reports the chosen value", () => {
    const onChange = vi.fn();
    renderSelect({ onChange });
    const combobox = screen.getByRole("combobox", { name: "Choice" });
    expect(fieldSelectValue(combobox)).toBe("a");
    setFieldSelectValue(combobox, "b");
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
