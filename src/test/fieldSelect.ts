import { waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

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

/** Choose by value using real pointer/focus sequences; wait for popup cleanup. */
export async function setFieldSelectValue(
    combobox: HTMLElement,
    value: string,
): Promise<void> {
    if (fieldSelectDisabled(combobox)) return;
    const user = userEvent.setup();
    if (combobox instanceof HTMLSelectElement) {
        await user.selectOptions(combobox, value);
        return;
    }
    await user.click(combobox);
    const list = await waitFor(() => selectList(combobox));
    const option = within(list)
        .getAllByRole("option")
        .find((entry) => entry.getAttribute("data-value") === value);
    if (option === undefined)
        throw new Error(`FieldSelect option not found: ${value}`);
    await user.click(option);
    await waitFor(() => {
        if (combobox.getAttribute("aria-expanded") === "true")
            throw new Error("FieldSelect did not close");
    });
}

/** Visible labels for each option, dismissing the popup before returning. */
export async function fieldSelectOptionLabels(
    combobox: HTMLElement,
): Promise<string[]> {
    if (combobox instanceof HTMLSelectElement) {
        return Array.from(combobox.options, (option) =>
            option.textContent.trim(),
        );
    }
    const user = userEvent.setup();
    await user.click(combobox);
    const list = await waitFor(() => selectList(combobox));
    const labels = within(list)
        .getAllByRole("option")
        .map((option) => option.textContent.trim());
    await user.keyboard("{Escape}");
    return labels;
}

/** Option identities, independent of translated labels or DOM ancestry. */
export async function fieldSelectOptionValues(
    combobox: HTMLElement,
): Promise<string[]> {
    if (combobox instanceof HTMLSelectElement) {
        return Array.from(combobox.options, (option) => option.value);
    }
    const user = userEvent.setup();
    await user.click(combobox);
    const list = await waitFor(() => selectList(combobox));
    const values = within(list)
        .getAllByRole("option")
        .map((option) => option.getAttribute("data-value") ?? "");
    await user.keyboard("{Escape}");
    return values;
}

/** Follow the accessible popup relationship; a portal is not a DOM descendant. */
function selectList(combobox: HTMLElement): HTMLElement {
    const listId = combobox.getAttribute("aria-controls");
    const list = listId === null ? null : document.getElementById(listId);
    if (list === null) throw new Error("FieldSelect list not found");
    return list;
}
