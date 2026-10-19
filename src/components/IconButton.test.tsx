import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState, type JSX } from "react";
import { afterEach, expect, test, vi } from "vitest";
import { IconButton } from "./IconButton";

afterEach(cleanup);

test("mouse hover exposes a hint and clicking performs only the auxiliary action", async () => {
    const user = userEvent.setup();
    const action = vi.fn();
    const submit = vi.fn();
    render(
        <form
            onSubmit={(event) => {
                event.preventDefault();
                submit();
            }}
        >
            <IconButton
                icon="trash"
                label="Remove argument 1"
                destructive
                onClick={action}
            />
        </form>,
    );
    const button = screen.getByRole("button", { name: "Remove argument 1" });
    await user.hover(button);
    expect((await screen.findByText("Remove argument 1")).textContent).toBe(
        "Remove argument 1",
    );
    await user.click(button);
    expect(action).toHaveBeenCalledTimes(1);
    expect(submit).not.toHaveBeenCalled();
});

test("keyboard focus exposes the hint, Escape dismisses it and Enter activates", async () => {
    const user = userEvent.setup();
    const action = vi.fn();
    render(
        <IconButton
            icon="trash"
            label="Remove field 1"
            destructive
            onClick={action}
        />,
    );
    await user.tab();
    expect(document.activeElement).toBe(
        screen.getByRole("button", { name: "Remove field 1" }),
    );
    // The hint repeats the accessible name. Check its rendered text without
    // assuming Base UI exposes a separate tooltip accessibility node.
    await screen.findByText("Remove field 1");
    await user.keyboard("{Escape}");
    await waitFor(() => {
        expect(screen.queryByText("Remove field 1")).toBeNull();
    });
    await user.keyboard("{Enter}");
    expect(action).toHaveBeenCalledTimes(1);
});

test("auxiliary submit icons submit with Enter and suppress submission while blocked", async () => {
    const user = userEvent.setup();
    const submit = vi.fn();
    const form = (blocked: boolean) => (
        <form
            onSubmit={(event) => {
                event.preventDefault();
                submit();
            }}
        >
            <IconButton
                type="submit"
                icon="refresh"
                label="Rediscover"
                blocked={blocked}
            />
        </form>
    );
    const view = render(form(false));
    await user.tab();
    await user.keyboard("{Enter}");
    expect(submit).toHaveBeenCalledTimes(1);
    view.rerender(form(true));
    await user.keyboard("{Enter}");
    await user.click(screen.getByRole("button", { name: "Rediscover" }));
    expect(submit).toHaveBeenCalledTimes(1);
});

test("blocked auxiliary actions remain keyboard focusable without running callbacks", async () => {
    const user = userEvent.setup();
    const action = vi.fn();
    render(
        <IconButton
            icon="trash"
            label="Remove field 1"
            blocked
            onClick={action}
        />,
    );
    const button = screen.getByRole("button", { name: "Remove field 1" });
    await user.tab();
    expect(document.activeElement).toBe(button);
    await user.keyboard("{Enter}");
    await user.click(button);
    expect(action).not.toHaveBeenCalled();
});

test("a click closes the hint even when its action replaces the page", async () => {
    const user = userEvent.setup();

    function Harness(): JSX.Element {
        const [open, setOpen] = useState(true);
        return open ? (
            <IconButton
                icon="info"
                label="Open details"
                onClick={() => {
                    setOpen(false);
                }}
            />
        ) : (
            <p>Detail page</p>
        );
    }

    render(<Harness />);
    const button = screen.getByRole("button", { name: "Open details" });
    await user.hover(button);
    await screen.findByText("Open details");
    await user.click(button);
    expect(await screen.findByText("Detail page")).toBeTruthy();
    await waitFor(() => {
        expect(screen.queryByText("Open details")).toBeNull();
    });
});
