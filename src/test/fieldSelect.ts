import { fireEvent, within } from "@testing-library/react";

/** Current value of a native select or {@link FieldSelect} trigger. */
export function fieldSelectValue(element: Element): string {
    if (element instanceof HTMLSelectElement) return element.value;
    if (element instanceof HTMLElement) return element.dataset["value"] ?? "";
    return "";
}

/** Whether the control rejects interaction (native `disabled` or `aria-disabled`). */
export function fieldSelectDisabled(element: Element): boolean {
    if (element instanceof HTMLSelectElement) return element.disabled;
    if (element instanceof HTMLButtonElement)
        return (
            element.disabled || element.getAttribute("aria-disabled") === "true"
        );
    return false;
}

/**
 * Choose an option by value. Works with native `<select>` and {@link FieldSelect}.
 */
export function setFieldSelectValue(
    combobox: HTMLElement,
    value: string,
): void {
    if (combobox instanceof HTMLSelectElement) {
        fireEvent.change(combobox, { target: { value } });
        return;
    }
    if (
        combobox instanceof HTMLButtonElement &&
        (combobox.disabled || combobox.getAttribute("aria-disabled") === "true")
    ) {
        return;
    }
    const root = combobox.closest("[data-field-select]");
    if (root === null) {
        throw new Error("FieldSelect root not found");
    }
    fireEvent.click(combobox);
    const option = (root as HTMLElement).querySelector(
        `[role="option"][data-value="${value}"]`,
    );
    if (option === null) {
        throw new Error(`FieldSelect option not found: ${value}`);
    }
    fireEvent.click(option);
}

/** Visible labels for each option (opens the list briefly). */
export function fieldSelectOptionLabels(combobox: HTMLElement): string[] {
    if (combobox instanceof HTMLSelectElement) {
        return Array.from(combobox.querySelectorAll("option"), (option) =>
            option.textContent.trim(),
        );
    }
    fireEvent.click(combobox);
    const root = combobox.closest("[data-field-select]");
    if (root === null) return [];
    const labels = within(root as HTMLElement)
        .getAllByRole("option")
        .map((option) => option.textContent.trim());
    fireEvent.click(combobox);
    return labels;
}

export function fieldSelectOptionValues(combobox: HTMLElement): string[] {
    if (combobox instanceof HTMLSelectElement) {
        return Array.from(
            combobox.querySelectorAll("option"),
            (option) => option.value,
        );
    }
    fireEvent.click(combobox);
    const root = combobox.closest("[data-field-select]");
    if (root === null) return [];
    return Array.from(
        within(root as HTMLElement).getAllByRole("option"),
        (option) => option.getAttribute("data-value") ?? "",
    );
}
